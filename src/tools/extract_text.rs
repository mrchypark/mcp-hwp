use crate::input::{InputFormat, load_input};
use crate::mcp::errors;
use crate::tools::error_result;
use hwpers::{HwpError, HwpReader, HwpxReader};
use serde_json::{Value, json};

pub fn call(args: &Value) -> Value {
    let payload = match load_input(args) {
        Ok(payload) => payload,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let include_newlines = args
        .get("include_newlines")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    let normalize_whitespace = args
        .get("normalize_whitespace")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let max_chars = args.get("max_chars").and_then(|value| value.as_u64());

    let document = match parse_document(&payload.bytes, payload.format) {
        Ok(document) => document,
        Err(err) => {
            return error_result(err.kind, err.message, Some(payload.source.as_str()));
        }
    };

    let text = document.extract_text();
    let normalized = normalize_text(&text, include_newlines, normalize_whitespace);
    let truncated = apply_max_chars(normalized, max_chars);

    json!({
        "content": [{"type": "text", "text": truncated}],
        "structuredContent": {"text": truncated},
        "isError": false
    })
}

struct ToolError {
    kind: &'static str,
    message: String,
}

fn parse_document(bytes: &[u8], format: InputFormat) -> Result<hwpers::HwpDocument, ToolError> {
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
