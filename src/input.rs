use crate::mcp::contracts::MAX_INPUT_BYTES;
use crate::mcp::errors;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    Auto,
    Hwp,
    Hwpx,
}

impl InputFormat {
    fn parse(value: Option<&Value>) -> Result<Self, InputError> {
        let Some(value) = value else {
            return Ok(InputFormat::Auto);
        };
        let Some(value) = value.as_str() else {
            return Err(InputError::invalid_input("format must be a string"));
        };
        match value {
            "auto" => Ok(InputFormat::Auto),
            "hwp" => Ok(InputFormat::Hwp),
            "hwpx" => Ok(InputFormat::Hwpx),
            _ => Err(InputError::unsupported_format(
                "format must be auto, hwp, or hwpx",
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            InputFormat::Auto => "auto",
            InputFormat::Hwp => "hwp",
            InputFormat::Hwpx => "hwpx",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputPayload {
    pub bytes: Vec<u8>,
    pub format: InputFormat,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct InputError {
    pub kind: &'static str,
    pub message: String,
}

impl InputError {
    fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(errors::INVALID_INPUT, message)
    }

    fn too_large(message: impl Into<String>) -> Self {
        Self::new(errors::TOO_LARGE, message)
    }

    fn unsupported_format(message: impl Into<String>) -> Self {
        Self::new(errors::UNSUPPORTED_FORMAT, message)
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for InputError {}

pub fn load_input(args: &Value) -> Result<InputPayload, InputError> {
    let obj = args
        .as_object()
        .ok_or_else(|| InputError::invalid_input("arguments must be an object"))?;

    let path_value = obj.get("path");
    let base64_value = obj.get("base64");

    match (path_value, base64_value) {
        (None, None) => {
            return Err(InputError::invalid_input(
                "either path or base64 is required",
            ));
        }
        (Some(_), Some(_)) => {
            return Err(InputError::invalid_input(
                "path and base64 cannot both be set",
            ));
        }
        _ => {}
    }

    let format = InputFormat::parse(obj.get("format"))?;

    if let Some(value) = path_value {
        let path = value
            .as_str()
            .ok_or_else(|| InputError::invalid_input("path must be a string"))?;
        let path_ref = Path::new(path);
        let metadata = fs::metadata(path_ref)
            .map_err(|_| InputError::invalid_input("path must exist and be a file"))?;
        if !metadata.is_file() {
            return Err(InputError::invalid_input("path must be a file"));
        }
        let len = metadata.len();
        if len > MAX_INPUT_BYTES {
            return Err(InputError::too_large(format!(
                "input exceeds limit: {len} bytes (max {MAX_INPUT_BYTES})"
            )));
        }
        let bytes = fs::read(path_ref)
            .map_err(|_| InputError::invalid_input("failed to read path contents"))?;
        return Ok(InputPayload {
            bytes,
            format,
            source: format!("path:{path}"),
        });
    }

    let value = base64_value.expect("base64 must be present here");
    let base64_str = value
        .as_str()
        .ok_or_else(|| InputError::invalid_input("base64 must be a string"))?;
    let bytes = STANDARD
        .decode(base64_str.as_bytes())
        .map_err(|_| InputError::invalid_input("base64 must be valid"))?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(InputError::too_large(format!(
            "input exceeds limit: {} bytes (max {MAX_INPUT_BYTES})",
            bytes.len()
        )));
    }
    Ok(InputPayload {
        bytes,
        format,
        source: "base64".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn base64_ok() {
        let encoded = STANDARD.encode(b"hello");
        let args = json!({"base64": encoded});
        let payload = load_input(&args).expect("payload");
        assert_eq!(payload.bytes, b"hello");
        assert_eq!(payload.format, InputFormat::Auto);
        assert_eq!(payload.source, "base64");
    }

    #[test]
    fn base64_invalid() {
        let args = json!({"base64": "not@@@"});
        let err = load_input(&args).expect_err("error");
        assert_eq!(err.kind, errors::INVALID_INPUT);
    }

    #[test]
    fn missing_input() {
        let args = json!({});
        let err = load_input(&args).expect_err("error");
        assert_eq!(err.kind, errors::INVALID_INPUT);
    }

    #[test]
    fn both_present() {
        let encoded = STANDARD.encode(b"hello");
        let args = json!({"path": "./example.hwp", "base64": encoded});
        let err = load_input(&args).expect_err("error");
        assert_eq!(err.kind, errors::INVALID_INPUT);
    }

    #[test]
    fn path_not_found() {
        let args = json!({"path": "/tmp/definitely-missing-hwp-file.hwp"});
        let err = load_input(&args).expect_err("error");
        assert_eq!(err.kind, errors::INVALID_INPUT);
    }

    #[test]
    fn path_is_dir() {
        let dir = tempdir().expect("tempdir");
        let args = json!({"path": dir.path().to_string_lossy()});
        let err = load_input(&args).expect_err("error");
        assert_eq!(err.kind, errors::INVALID_INPUT);
    }

    #[test]
    fn too_large() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("large.hwp");
        let file = File::create(&file_path).expect("file");
        file.set_len(MAX_INPUT_BYTES + 1).expect("set_len");
        let args = json!({"path": file_path.to_string_lossy()});
        let err = load_input(&args).expect_err("error");
        assert_eq!(err.kind, errors::TOO_LARGE);
    }
}
