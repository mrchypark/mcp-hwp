use crate::errors::AppError;
use crate::input::{InputFormat, load_input};
use crate::tools::error_result;
use hwpers::{HwpError, HwpReader, HwpxReader};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DEFAULT_PREVIEW_CHARS: usize = 120;

#[derive(Debug, Clone)]
pub struct Request {
    pub payload: crate::input::InputPayload,
    pub max_sections: Option<u64>,
    pub max_paragraphs_per_section: Option<u64>,
    pub preview_chars: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParagraphResponse {
    pub index: u64,
    pub char_count: u64,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionResponse {
    pub index: u64,
    pub paragraphs: Vec<ParagraphResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub format: String,
    pub sections: Vec<SectionResponse>,
    pub warnings: Vec<String>,
}

pub fn request_from_value(args: &Value) -> Result<Request, AppError> {
    Ok(Request {
        payload: load_input(args)?,
        max_sections: parse_optional_u64(args, "max_sections")?,
        max_paragraphs_per_section: parse_optional_u64(args, "max_paragraphs_per_section")?,
        preview_chars: parse_optional_u64(args, "preview_chars")?,
    })
}

pub fn run(req: Request) -> Result<Response, AppError> {
    let parsed = parse_document(&req.payload.bytes, req.payload.format)?;
    let max_sections = limit_from_request(req.max_sections);
    let max_paragraphs = limit_from_request(req.max_paragraphs_per_section);
    let preview_chars = preview_chars_from_request(req.preview_chars);

    let mut sections_out = Vec::new();

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

            paragraphs_out.push(ParagraphResponse {
                index: paragraph_index as u64,
                char_count,
                preview,
            });
        }

        sections_out.push(SectionResponse {
            index: section_index as u64,
            paragraphs: paragraphs_out,
        });
    }

    Ok(Response {
        format: parsed.format.as_str().to_string(),
        sections: sections_out,
        warnings: parsed.warnings,
    })
}

pub fn call(args: &Value) -> Value {
    let req = match request_from_value(args) {
        Ok(req) => req,
        Err(err) => return error_result(err.kind, err.message, None),
    };
    let source = req.payload.source.clone();
    let preview_chars = preview_chars_from_request(req.preview_chars);
    let response = match run(req) {
        Ok(response) => response,
        Err(err) => return error_result(err.kind, err.message, Some(source.as_str())),
    };

    let section_count = response.sections.len() as u64;
    let paragraph_count = response
        .sections
        .iter()
        .map(|section| section.paragraphs.len() as u64)
        .sum::<u64>();
    let summary = format!(
        "sections: {section_count}, paragraphs: {paragraph_count} (preview_chars={preview_chars})"
    );

    json!({
        "content": [{"type": "text", "text": summary}],
        "structuredContent": response,
        "isError": false
    })
}

struct ParsedDocument {
    document: hwpers::HwpDocument,
    format: InputFormat,
    warnings: Vec<String>,
}

fn parse_document(bytes: &[u8], format: InputFormat) -> Result<ParsedDocument, AppError> {
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

fn limit_from_request(value: Option<u64>) -> usize {
    let Some(value) = value else {
        return usize::MAX;
    };
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn preview_chars_from_request(value: Option<u64>) -> usize {
    let Some(value) = value else {
        return DEFAULT_PREVIEW_CHARS;
    };
    usize::try_from(value).unwrap_or(usize::MAX)
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
