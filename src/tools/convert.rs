use crate::input::{InputFormat, load_input};
use crate::mcp::contracts::MAX_OUTPUT_BYTES;
use crate::mcp::errors;
use crate::tools::error_result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hwpers::{HwpError, HwpReader, HwpWriter, HwpxReader, HwpxWriter};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub fn call(args: &Value) -> Value {
    let payload = match load_input(args) {
        Ok(payload) => payload,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let to_format = match OutputFormat::parse(args.get("to")) {
        Ok(to_format) => to_format,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let output_path = match parse_output_path(args.get("output_path")) {
        Ok(path) => path,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let parsed = match parse_document(&payload.bytes, payload.format) {
        Ok(parsed) => parsed,
        Err(err) => return error_result(err.kind, err.message, Some(payload.source.as_str())),
    };

    let output_bytes = match to_format {
        OutputFormat::Hwp => HwpWriter::from_document(parsed.document)
            .to_bytes()
            .map_err(|error| map_hwp_error_with_stage(error, "convert to hwp")),
        OutputFormat::Hwpx => HwpxWriter::from_document(parsed.document)
            .to_bytes()
            .map_err(|error| map_hwp_error_with_stage(error, "convert to hwpx")),
    };

    let output_bytes = match output_bytes {
        Ok(bytes) => bytes,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let bytes_len = output_bytes.len() as u64;
    let warnings = parsed.warnings;

    match output_path {
        Some(path) => match write_output(&path, &output_bytes) {
            Ok(output) => json!({
                "content": output.content,
                "structuredContent": {
                    "to": to_format.as_str(),
                    "path": output.path,
                    "uri": output.uri,
                    "bytes_len": bytes_len,
                    "warnings": warnings
                },
                "isError": false
            }),
            Err(err) => error_result(err.kind, err.message, None),
        },
        None => {
            if bytes_len > MAX_OUTPUT_BYTES {
                return error_result(
                    errors::TOO_LARGE,
                    format!("output exceeds limit: {bytes_len} bytes (max {MAX_OUTPUT_BYTES})"),
                    None,
                );
            }
            let base64 = STANDARD.encode(&output_bytes);
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("converted to {} ({bytes_len} bytes)", to_format.as_str())
                }],
                "structuredContent": {
                    "to": to_format.as_str(),
                    "base64": base64,
                    "bytes_len": bytes_len,
                    "warnings": warnings
                },
                "isError": false
            })
        }
    }
}

struct ToolError {
    kind: &'static str,
    message: String,
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

enum OutputFormat {
    Hwp,
    Hwpx,
}

impl OutputFormat {
    fn parse(value: Option<&Value>) -> Result<Self, ToolError> {
        let Some(value) = value else {
            return Err(ToolError {
                kind: errors::INVALID_INPUT,
                message: "to is required".to_string(),
            });
        };
        let Some(value) = value.as_str() else {
            return Err(ToolError {
                kind: errors::INVALID_INPUT,
                message: "to must be a string".to_string(),
            });
        };
        match value {
            "hwp" => Ok(OutputFormat::Hwp),
            "hwpx" => Ok(OutputFormat::Hwpx),
            _ => Err(ToolError {
                kind: errors::INVALID_INPUT,
                message: "to must be hwp or hwpx".to_string(),
            }),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Hwp => "hwp",
            OutputFormat::Hwpx => "hwpx",
        }
    }
}

fn parse_output_path(value: Option<&Value>) -> Result<Option<String>, ToolError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(path) = value.as_str() else {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "output_path must be a string".to_string(),
        });
    };
    if path.trim().is_empty() {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "output_path must not be empty".to_string(),
        });
    }
    Ok(Some(path.to_string()))
}

fn parse_document(bytes: &[u8], format: InputFormat) -> Result<ParsedDocument, ToolError> {
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
                    Err(hwpx_err) => Err(ToolError {
                        kind: errors::PARSE_FAILED,
                        message: format!(
                            "auto format parse failed (hwp: {}; hwpx: {})",
                            hwp_err, hwpx_err
                        ),
                    }),
                },
            }
        }
    }
}

fn write_output(path: &str, bytes: &[u8]) -> Result<OutputResource, ToolError> {
    fs::write(path, bytes).map_err(|err| ToolError {
        kind: errors::INTERNAL_ERROR,
        message: format!("failed to write output: {err}"),
    })?;

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

fn map_hwp_error(error: HwpError) -> ToolError {
    match error {
        HwpError::UnsupportedVersion(message) => {
            if message.contains("Password-encrypted") {
                ToolError {
                    kind: errors::ENCRYPTED,
                    message,
                }
            } else {
                ToolError {
                    kind: errors::PARSE_FAILED,
                    message,
                }
            }
        }
        HwpError::InvalidInput(message) => ToolError {
            kind: errors::INVALID_INPUT,
            message,
        },
        HwpError::Io(err) => ToolError {
            kind: errors::INVALID_INPUT,
            message: err.to_string(),
        },
        HwpError::InvalidFormat(message)
        | HwpError::Cfb(message)
        | HwpError::CompressionError(message)
        | HwpError::ParseError(message)
        | HwpError::EncodingError(message)
        | HwpError::NotFound(message) => ToolError {
            kind: errors::PARSE_FAILED,
            message,
        },
    }
}

fn map_hwp_error_with_format(error: HwpError, format: &str) -> ToolError {
    let mut mapped = map_hwp_error(error);
    mapped.message = format!("{format} parse failed: {}", mapped.message);
    mapped
}

fn map_hwp_error_with_stage(error: HwpError, stage: &str) -> ToolError {
    let mut mapped = map_hwp_error(error);
    mapped.message = format!("{stage} failed: {}", mapped.message);
    mapped
}
