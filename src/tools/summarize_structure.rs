use crate::input::{InputFormat, load_input};
use crate::mcp::errors;
use crate::tools::error_result;
use hwpers::{HwpError, HwpReader, HwpxReader};
use serde_json::{Value, json};

const DEFAULT_PREVIEW_CHARS: usize = 120;

pub fn call(args: &Value) -> Value {
    let payload = match load_input(args) {
        Ok(payload) => payload,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let max_sections = limit_from_args(args.get("max_sections"));
    let max_paragraphs = limit_from_args(args.get("max_paragraphs_per_section"));
    let preview_chars = preview_chars_from_args(args.get("preview_chars"));

    let parsed = match parse_document(&payload.bytes, payload.format) {
        Ok(parsed) => parsed,
        Err(err) => {
            return error_result(err.kind, err.message, Some(payload.source.as_str()));
        }
    };

    let mut sections_out = Vec::new();
    let mut paragraph_count: u64 = 0;

    for (section_index, section) in parsed.document.sections().enumerate() {
        if section_index >= max_sections {
            break;
        }

        let mut paragraphs_out = Vec::new();
        for (paragraph_index, paragraph) in section.paragraphs.iter().enumerate() {
            if paragraph_index >= max_paragraphs {
                break;
            }

            let text = paragraph
                .text
                .as_ref()
                .map(|para_text| para_text.content.as_str())
                .unwrap_or("");

            let char_count = text.chars().count() as u64;
            let preview = text.chars().take(preview_chars).collect::<String>();

            paragraphs_out.push(json!({
                "index": paragraph_index as u64,
                "char_count": char_count,
                "preview": preview
            }));

            paragraph_count += 1;
        }

        sections_out.push(json!({
            "index": section_index as u64,
            "paragraphs": paragraphs_out
        }));
    }

    let section_count = sections_out.len() as u64;
    let summary = format!(
        "sections: {section_count}, paragraphs: {paragraph_count} (preview_chars={preview_chars})"
    );

    json!({
        "content": [{"type": "text", "text": summary}],
        "structuredContent": {
            "format": parsed.format.as_str(),
            "sections": sections_out,
            "warnings": parsed.warnings
        },
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

fn limit_from_args(value: Option<&Value>) -> usize {
    let Some(value) = value else {
        return usize::MAX;
    };
    let Some(value) = value.as_u64() else {
        return usize::MAX;
    };
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn preview_chars_from_args(value: Option<&Value>) -> usize {
    let Some(value) = value else {
        return DEFAULT_PREVIEW_CHARS;
    };
    let Some(value) = value.as_u64() else {
        return DEFAULT_PREVIEW_CHARS;
    };
    usize::try_from(value).unwrap_or(usize::MAX)
}
