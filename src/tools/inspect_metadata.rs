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

    let parsed = match parse_document(&payload.bytes, payload.format) {
        Ok(parsed) => parsed,
        Err(err) => {
            return error_result(err.kind, err.message, Some(payload.source.as_str()));
        }
    };

    let sections = parsed.document.sections().count() as u64;
    let paragraphs = parsed
        .document
        .sections()
        .map(|section| section.paragraphs.len() as u64)
        .sum::<u64>();

    let mut structured = json!({
        "format": parsed.format.as_str(),
        "sections": sections,
        "paragraphs": paragraphs,
        "warnings": parsed.warnings,
    });

    if let Some(obj) = structured.as_object_mut() {
        obj.insert(
            "encrypted".to_string(),
            json!(parsed.document.is_encrypted()),
        );
        obj.insert(
            "compressed".to_string(),
            json!(parsed.document.header.is_compressed()),
        );
        obj.insert(
            "version".to_string(),
            json!(parsed.document.header.version_string()),
        );
    }

    let summary = format!("sections: {sections}, paragraphs: {paragraphs}");

    json!({
        "content": [{"type": "text", "text": summary}],
        "structuredContent": structured,
        "isError": false
    })
}

struct ToolError {
    kind: &'static str,
    message: String,
}

struct ParsedDocument {
    document: hwpers::HwpDocument,
    format: InputFormat,
    warnings: Vec<String>,
}

fn parse_document(bytes: &[u8], format: InputFormat) -> Result<ParsedDocument, ToolError> {
    match format {
        InputFormat::Hwp => HwpReader::from_bytes(bytes)
            .map(|document| ParsedDocument {
                document,
                format,
                warnings: Vec::new(),
            })
            .map_err(|error| map_hwp_error_with_format(error, format.as_str())),
        InputFormat::Hwpx => HwpxReader::from_bytes(bytes)
            .map(|document| ParsedDocument {
                document,
                format,
                warnings: Vec::new(),
            })
            .map_err(|error| map_hwp_error_with_format(error, format.as_str())),
        InputFormat::Auto => {
            let hwp_result = HwpReader::from_bytes(bytes);
            match hwp_result {
                Ok(document) => Ok(ParsedDocument {
                    document,
                    format: InputFormat::Hwp,
                    warnings: Vec::new(),
                }),
                Err(hwp_err) => match HwpxReader::from_bytes(bytes) {
                    Ok(document) => Ok(ParsedDocument {
                        document,
                        format: InputFormat::Hwpx,
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
