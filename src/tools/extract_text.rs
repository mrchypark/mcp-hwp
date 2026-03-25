use crate::errors::AppError;
use crate::input::{InputFormat, load_input};
use crate::results::TextResult;
use crate::tools::error_result;
use hwpers::{HwpError, HwpReader, HwpxReader};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct Request {
    pub payload: crate::input::InputPayload,
    pub include_newlines: bool,
    pub normalize_whitespace: bool,
    pub max_chars: Option<u64>,
}

pub type Response = TextResult;

pub fn request_from_value(args: &Value) -> Result<Request, AppError> {
    let payload = load_input(args)?;
    Ok(Request {
        payload,
        include_newlines: parse_optional_bool(args, "include_newlines")?.unwrap_or(true),
        normalize_whitespace: parse_optional_bool(args, "normalize_whitespace")?.unwrap_or(false),
        max_chars: parse_optional_u64(args, "max_chars")?,
    })
}

pub fn run(req: Request) -> Result<Response, AppError> {
    let document = parse_document(&req.payload.bytes, req.payload.format)?;

    let text = document.extract_text();
    let normalized = normalize_text(&text, req.include_newlines, req.normalize_whitespace);
    let truncated = apply_max_chars(normalized, req.max_chars);

    Ok(Response { text: truncated })
}

pub fn call(args: &Value) -> Value {
    let req = match request_from_value(args) {
        Ok(req) => req,
        Err(err) => return error_result(err.kind, err.message, None),
    };
    let response = match run(req.clone()) {
        Ok(response) => response,
        Err(err) => return error_result(err.kind, err.message, Some(req.payload.source.as_str())),
    };

    json!({
        "content": [{"type": "text", "text": response.text}],
        "structuredContent": {"text": response.text},
        "isError": false
    })
}

fn parse_document(bytes: &[u8], format: InputFormat) -> Result<hwpers::HwpDocument, AppError> {
    match format {
        InputFormat::Hwp => HwpReader::from_bytes(bytes)
            .map_err(|error| map_hwp_error_with_format(error, format.as_str())),
        InputFormat::Hwpx => HwpxReader::from_bytes(bytes)
            .map_err(|error| map_hwp_error_with_format(error, format.as_str())),
        InputFormat::Auto => {
            let hwp_result = HwpReader::from_bytes(bytes);
            match hwp_result {
                Ok(doc) => Ok(doc),
                Err(hwp_err) => match HwpxReader::from_bytes(bytes) {
                    Ok(doc) => Ok(doc),
                    Err(hwpx_err) => Err(AppError::parse_failed(format!(
                        "auto format parse failed (hwp: {}; hwpx: {})",
                        hwp_err, hwpx_err
                    ))),
                },
            }
        }
    }
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

fn map_hwp_error_with_format(error: HwpError, format: &str) -> AppError {
    let mut mapped = map_hwp_error(error);
    mapped.message = format!("{format} parse failed: {}", mapped.message);
    mapped
}

fn normalize_text(text: &str, include_newlines: bool, normalize_whitespace: bool) -> String {
    let mut output = text.replace("\r\n", "\n").replace('\r', "\n");

    if !include_newlines {
        output = output.replace('\n', " ");
    }

    if normalize_whitespace {
        if include_newlines {
            let lines: Vec<String> = output
                .lines()
                .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect();
            output = lines.join("\n");
        } else {
            output = output.split_whitespace().collect::<Vec<_>>().join(" ");
        }
    }

    output
}

fn apply_max_chars(text: String, max_chars: Option<u64>) -> String {
    let Some(max_chars) = max_chars else {
        return text;
    };
    let limit = usize::try_from(max_chars).unwrap_or(usize::MAX);
    text.chars().take(limit).collect()
}

fn parse_optional_bool(args: &Value, key: &str) -> Result<Option<bool>, AppError> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_bool() else {
        return Err(AppError::invalid_input(format!("{key} must be a boolean")));
    };
    Ok(Some(value))
}

fn parse_optional_u64(args: &Value, key: &str) -> Result<Option<u64>, AppError> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_u64() else {
        return Err(AppError::invalid_input(format!("{key} must be an integer")));
    };
    Ok(Some(value))
}
