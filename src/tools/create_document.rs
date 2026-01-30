use crate::mcp::contracts::MAX_OUTPUT_BYTES;
use crate::mcp::errors;
use crate::tools::error_result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hwpers::{HwpError, HwpWriter};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub fn call(args: &Value) -> Value {
    let text = match parse_text(args.get("text")) {
        Ok(text) => text,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let output_path = match parse_output_path(args.get("output_path")) {
        Ok(path) => path,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let mut writer = HwpWriter::new();
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    for paragraph in normalized.split('\n') {
        if let Err(error) = writer.add_paragraph(paragraph) {
            let err = map_hwp_error_with_stage(error, "add paragraph");
            return error_result(err.kind, err.message, None);
        }
    }

    let output_bytes = match writer.to_bytes() {
        Ok(bytes) => bytes,
        Err(error) => {
            let err = map_hwp_error_with_stage(error, "write document");
            return error_result(err.kind, err.message, None);
        }
    };

    let bytes_len = output_bytes.len() as u64;

    match output_path {
        Some(path) => match write_output(&path, &output_bytes) {
            Ok(output) => json!({
                "content": output.content,
                "structuredContent": {
                    "path": output.path,
                    "uri": output.uri,
                    "bytes_len": bytes_len
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
                    "text": format!("created document ({bytes_len} bytes)")
                }],
                "structuredContent": {
                    "base64": base64,
                    "bytes_len": bytes_len
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

struct OutputResource {
    path: String,
    uri: String,
    content: Vec<Value>,
}

fn parse_text(value: Option<&Value>) -> Result<String, ToolError> {
    let Some(value) = value else {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "text is required".to_string(),
        });
    };
    let Some(text) = value.as_str() else {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "text must be a string".to_string(),
        });
    };
    if text.trim().is_empty() {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "text must not be empty".to_string(),
        });
    }
    Ok(text.to_string())
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

fn write_output(path: &str, bytes: &[u8]) -> Result<OutputResource, ToolError> {
    fs::write(path, bytes).map_err(|err| ToolError {
        kind: errors::INTERNAL_ERROR,
        message: format!("failed to write output: {err}"),
    })?;

    let uri = format!("file://{path}");
    let name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("document");

    let content = vec![
        json!({
            "type": "text",
            "text": format!("document written to {path}")
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

fn map_hwp_error_with_stage(error: HwpError, stage: &str) -> ToolError {
    let mut mapped = map_hwp_error(error);
    mapped.message = format!("{stage} failed: {}", mapped.message);
    mapped
}
