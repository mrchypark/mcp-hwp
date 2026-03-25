use crate::constants::MAX_OUTPUT_BYTES;
use crate::errors::AppError;
use crate::input::{InputFormat, load_input};
use crate::tools::error_result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hwpers::{HwpError, HwpReader, HwpWriter, HwpxReader, HwpxWriter};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Request {
    pub payload: crate::input::InputPayload,
    pub to: OutputFormat,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub content: Vec<Value>,
    pub structured_content: Value,
}

pub fn call(args: &Value) -> Value {
    let req = match request_from_value(args) {
        Ok(req) => req,
        Err(err) => return error_result(err.kind, err.message, None),
    };
    let source = req.payload.source.clone();
    let response = match run(req) {
        Ok(response) => response,
        Err(err) => return error_result(err.kind, err.message, Some(source.as_str())),
    };

    json!({
        "content": response.content,
        "structuredContent": response.structured_content,
        "isError": false
    })
}

pub fn request_from_value(args: &Value) -> Result<Request, AppError> {
    Ok(Request {
        payload: load_input(args)?,
        to: OutputFormat::parse(args.get("to"))?,
        output_path: parse_output_path(args.get("output_path"))?,
    })
}

pub fn run(req: Request) -> Result<Response, AppError> {
    let Request {
        payload,
        to,
        output_path,
    } = req;

    let parsed = parse_document(&payload.bytes, payload.format)?;

    let output_kind = to.clone();
    let output_bytes = match output_kind {
        OutputFormat::Hwp => HwpWriter::from_document(parsed.document)
            .to_bytes()
            .map_err(|error| map_hwp_error_with_stage(error, "convert to hwp")),
        OutputFormat::Hwpx => HwpxWriter::from_document(parsed.document)
            .to_bytes()
            .map_err(|error| map_hwp_error_with_stage(error, "convert to hwpx")),
    }?;

    let bytes_len = output_bytes.len() as u64;
    let warnings = parsed.warnings;

    match output_path {
        Some(path) => {
            let output = write_output(&path, &output_bytes)?;
            Ok(Response {
                content: output.content,
                structured_content: json!({
                    "to": to.as_str(),
                    "path": output.path,
                    "uri": output.uri,
                    "bytes_len": bytes_len,
                    "warnings": warnings
                }),
            })
        }
        None => {
            if bytes_len > MAX_OUTPUT_BYTES {
                return Err(AppError::too_large(format!(
                    "output exceeds limit: {bytes_len} bytes (max {MAX_OUTPUT_BYTES})"
                )));
            }
            let base64 = STANDARD.encode(&output_bytes);
            Ok(Response {
                content: vec![json!({
                    "type": "text",
                    "text": format!("converted to {} ({bytes_len} bytes)", to.as_str())
                })],
                structured_content: json!({
                    "to": to.as_str(),
                    "base64": base64,
                    "bytes_len": bytes_len,
                    "warnings": warnings
                }),
            })
        }
    }
}

struct ParsedDocument {
    document: hwpers::HwpDocument,
    warnings: Vec<String>,
}

struct OutputResource {
    path: String,
    uri: String,
    content: Vec<Value>,
}

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Hwp,
    Hwpx,
}

impl OutputFormat {
    fn parse(value: Option<&Value>) -> Result<Self, AppError> {
        let Some(value) = value else {
            return Err(AppError::invalid_input("to is required"));
        };
        let Some(value) = value.as_str() else {
            return Err(AppError::invalid_input("to must be a string"));
        };
        match value {
            "hwp" => Ok(OutputFormat::Hwp),
            "hwpx" => Ok(OutputFormat::Hwpx),
            _ => Err(AppError::invalid_input("to must be hwp or hwpx")),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Hwp => "hwp",
            OutputFormat::Hwpx => "hwpx",
        }
    }
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

fn parse_document(bytes: &[u8], format: InputFormat) -> Result<ParsedDocument, AppError> {
    match format {
        InputFormat::Hwp => HwpReader::from_bytes(bytes)
            .map(|document| ParsedDocument {
                document,
                warnings: Vec::new(),
            })
            .map_err(|error| map_hwp_error_with_format(error, format.as_str())),
        InputFormat::Hwpx => HwpxReader::from_bytes(bytes)
            .map(|document| ParsedDocument {
                document,
                warnings: Vec::new(),
            })
            .map_err(|error| map_hwp_error_with_format(error, format.as_str())),
        InputFormat::Auto => {
            let hwp_result = HwpReader::from_bytes(bytes);
            match hwp_result {
                Ok(document) => Ok(ParsedDocument {
                    document,
                    warnings: Vec::new(),
                }),
                Err(hwp_err) => match HwpxReader::from_bytes(bytes) {
                    Ok(document) => Ok(ParsedDocument {
                        document,
                        warnings: vec!["auto format: hwp parse failed; hwpx succeeded".to_string()],
                    }),
                    Err(hwpx_err) => Err(AppError::parse_failed(format!(
                        "auto format parse failed (hwp: {}; hwpx: {})",
                        hwp_err, hwpx_err
                    ))),
                },
            }
        }
    }
}

fn write_output(path: &str, bytes: &[u8]) -> Result<OutputResource, AppError> {
    fs::write(path, bytes)
        .map_err(|err| AppError::internal(format!("failed to write output: {err}")))?;

    let uri = format!("file://{path}");
    let name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("converted");

    let content = vec![
        json!({
            "type": "text",
            "text": format!("converted output written to {path}")
        }),
        json!({
            "type": "resource_link",
            "uri": uri,
            "name": name,
            "mimeType": "application/octet-stream"
        }),
    ];

    Ok(OutputResource {
        path: path.to_string(),
        uri: format!("file://{path}"),
        content,
    })
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

fn map_hwp_error_with_stage(error: HwpError, stage: &str) -> AppError {
    let mut mapped = map_hwp_error(error);
    mapped.message = format!("{stage} failed: {}", mapped.message);
    mapped
}
