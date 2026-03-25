use crate::constants::MAX_OUTPUT_BYTES;
use crate::errors::AppError;
use crate::results::{BinaryArtifact, FileArtifact};
use crate::tools::error_result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hwpers::{HwpError, HwpWriter};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Request {
    pub text: String,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub output: BinaryArtifact,
    pub file: Option<FileArtifact>,
}

pub fn request_from_value(args: &Value) -> Result<Request, AppError> {
    Ok(Request {
        text: parse_text(args.get("text"))?,
        output_path: parse_output_path(args.get("output_path"))?,
    })
}

pub fn run(req: Request) -> Result<Response, AppError> {
    let mut writer = HwpWriter::new();
    let normalized = req.text.replace("\r\n", "\n").replace('\r', "\n");
    for paragraph in normalized.split('\n') {
        writer
            .add_paragraph(paragraph)
            .map_err(|error| map_hwp_error_with_stage(error, "add paragraph"))?;
    }

    let output_bytes = writer
        .to_bytes()
        .map_err(|error| map_hwp_error_with_stage(error, "write document"))?;

    let bytes_len = output_bytes.len() as u64;
    let file = match req.output_path {
        Some(path) => {
            write_output(&path, &output_bytes)?;
            Some(FileArtifact { path, bytes_len })
        }
        None => {
            if bytes_len > MAX_OUTPUT_BYTES {
                return Err(AppError::too_large(format!(
                    "output exceeds limit: {bytes_len} bytes (max {MAX_OUTPUT_BYTES})"
                )));
            }
            None
        }
    };

    Ok(Response {
        output: BinaryArtifact {
            bytes: output_bytes,
            bytes_len,
        },
        file,
    })
}

pub fn call(args: &Value) -> Value {
    let req = match request_from_value(args) {
        Ok(req) => req,
        Err(err) => return error_result(err.kind, err.message, None),
    };
    let response = match run(req) {
        Ok(response) => response,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    if let Some(file) = response.file {
        let uri = format!("file://{}", file.path);
        let name = Path::new(&file.path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("document");

        return json!({
            "content": [
                {
                    "type": "text",
                    "text": format!("document written to {}", file.path)
                },
                {
                    "type": "resource_link",
                    "uri": uri,
                    "name": name,
                    "mimeType": "application/octet-stream"
                }
            ],
            "structuredContent": {
                "path": file.path,
                "uri": uri,
                "bytes_len": file.bytes_len
            },
            "isError": false
        });
    }

    let base64 = STANDARD.encode(&response.output.bytes);
    json!({
        "content": [{
            "type": "text",
            "text": format!("created document ({} bytes)", response.output.bytes_len)
        }],
        "structuredContent": {
            "base64": base64,
            "bytes_len": response.output.bytes_len
        },
        "isError": false
    })
}

fn parse_text(value: Option<&Value>) -> Result<String, AppError> {
    let Some(value) = value else {
        return Err(AppError::invalid_input("text is required"));
    };
    let Some(text) = value.as_str() else {
        return Err(AppError::invalid_input("text must be a string"));
    };
    if text.trim().is_empty() {
        return Err(AppError::invalid_input("text must not be empty"));
    }
    Ok(text.to_string())
}

fn parse_output_path(value: Option<&Value>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(path) = value.as_str() else {
        return Err(AppError::invalid_input("output_path must be a string"));
    };
    if path.trim().is_empty() {
        return Err(AppError::invalid_input("output_path must not be empty"));
    }
    Ok(Some(path.to_string()))
}

fn write_output(path: &str, bytes: &[u8]) -> Result<(), AppError> {
    fs::write(path, bytes)
        .map_err(|err| AppError::internal(format!("failed to write output: {err}")))?;
    Ok(())
}

fn map_hwp_error(error: HwpError) -> AppError {
    match error {
        HwpError::UnsupportedVersion(message) => {
            if message.contains("Password-encrypted") {
                AppError::encrypted(message)
            } else {
                AppError::parse_failed(message)
            }
        }
        HwpError::InvalidInput(message) => AppError::invalid_input(message),
        HwpError::Io(err) => AppError::invalid_input(err.to_string()),
        HwpError::InvalidFormat(message)
        | HwpError::Cfb(message)
        | HwpError::CompressionError(message)
        | HwpError::ParseError(message)
        | HwpError::EncodingError(message)
        | HwpError::NotFound(message) => AppError::parse_failed(message),
    }
}

fn map_hwp_error_with_stage(error: HwpError, stage: &str) -> AppError {
    let mut mapped = map_hwp_error(error);
    mapped.message = format!("{stage} failed: {}", mapped.message);
    mapped
}
