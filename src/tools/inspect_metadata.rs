use crate::errors::AppError;
use crate::input::{InputFormat, load_input};
use crate::tools::error_result;
use hwpers::{HwpError, HwpReader, HwpxReader};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct Request {
    pub payload: crate::input::InputPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub format: String,
    pub sections: u64,
    pub paragraphs: u64,
    pub warnings: Vec<String>,
    pub encrypted: bool,
    pub compressed: bool,
    pub version: String,
}

pub fn request_from_value(args: &Value) -> Result<Request, AppError> {
    Ok(Request {
        payload: load_input(args)?,
    })
}

pub fn run(req: Request) -> Result<Response, AppError> {
    let parsed = parse_document(&req.payload.bytes, req.payload.format)?;

    let sections = parsed.document.sections().count() as u64;
    let paragraphs = parsed
        .document
        .sections()
        .map(|section| section.paragraphs.len() as u64)
        .sum::<u64>();

    Ok(Response {
        format: parsed.format.as_str().to_string(),
        sections,
        paragraphs,
        warnings: parsed.warnings,
        encrypted: parsed.document.is_encrypted(),
        compressed: parsed.document.header.is_compressed(),
        version: parsed.document.header.version_string().to_string(),
    })
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

    let summary = format!(
        "sections: {}, paragraphs: {}",
        response.sections, response.paragraphs
    );
    let structured = json!({
        "format": response.format,
        "sections": response.sections,
        "paragraphs": response.paragraphs,
        "warnings": response.warnings,
        "encrypted": response.encrypted,
        "compressed": response.compressed,
        "version": response.version,
    });

    json!({
        "content": [{"type": "text", "text": summary}],
        "structuredContent": structured,
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
