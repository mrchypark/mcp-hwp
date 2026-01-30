use crate::input::{InputFormat, load_input};
use crate::mcp::contracts::MAX_SVG_OUTPUT_BYTES;
use crate::mcp::errors;
use crate::tools::error_result;
use hwpers::render::renderer::{HwpRenderer, RenderOptions};
use hwpers::{HwpError, HwpReader, HwpxReader};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

pub fn call(args: &Value) -> Value {
    let payload = match load_input(args) {
        Ok(payload) => payload,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let pages = match parse_pages(args) {
        Ok(pages) => pages,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let output = match OutputMode::parse(args.get("output")) {
        Ok(output) => output,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let mut parsed = match parse_document(&payload.bytes, payload.format) {
        Ok(parsed) => parsed,
        Err(err) => {
            return error_result(err.kind, err.message, Some(payload.source.as_str()));
        }
    };

    if ensure_page_defs(&mut parsed.document) {
        parsed
            .warnings
            .push("missing page definition; default layout applied".to_string());
    }

    let renderer = HwpRenderer::new(&parsed.document, RenderOptions::default());
    let render_result = renderer.render();

    let mut rendered_pages = Vec::new();
    for page in pages {
        let page_index = match usize::try_from(page.saturating_sub(1)) {
            Ok(index) => index,
            Err(_) => return error_result(errors::INVALID_INPUT, "page index out of range", None),
        };
        let Some(svg) = render_result.to_svg(page_index) else {
            return error_result(
                errors::INVALID_INPUT,
                format!("page out of range: {page}"),
                None,
            );
        };
        rendered_pages.push(RenderedPage { page, svg });
    }

    if let Err(err) = enforce_size_limit(&rendered_pages) {
        return error_result(err.kind, err.message, None);
    }

    let structured_pages = match output {
        OutputMode::Inline => render_inline(&rendered_pages),
        OutputMode::Resource => match render_resource(&rendered_pages) {
            Ok(pages) => pages,
            Err(err) => return error_result(err.kind, err.message, None),
        },
    };

    let content = match output {
        OutputMode::Inline => vec![json!({
            "type": "text",
            "text": format!("rendered {} page(s) as svg", rendered_pages.len())
        })],
        OutputMode::Resource => build_resource_content(&structured_pages),
    };

    json!({
        "content": content,
        "structuredContent": {
            "format": parsed.format.as_str(),
            "pages": structured_pages,
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

struct RenderedPage {
    page: u64,
    svg: String,
}

enum OutputMode {
    Inline,
    Resource,
}

impl OutputMode {
    fn parse(value: Option<&Value>) -> Result<Self, ToolError> {
        let Some(value) = value else {
            return Ok(OutputMode::Inline);
        };
        let Some(value) = value.as_str() else {
            return Err(ToolError {
                kind: errors::INVALID_INPUT,
                message: "output must be a string".to_string(),
            });
        };
        match value {
            "inline" => Ok(OutputMode::Inline),
            "resource" => Ok(OutputMode::Resource),
            _ => Err(ToolError {
                kind: errors::INVALID_INPUT,
                message: "output must be inline or resource".to_string(),
            }),
        }
    }
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

fn parse_pages(args: &Value) -> Result<Vec<u64>, ToolError> {
    let mut pages = Vec::new();
    let mut seen = HashSet::new();

    if let Some(value) = args.get("page") {
        let page = value.as_u64().ok_or_else(|| ToolError {
            kind: errors::INVALID_INPUT,
            message: "page must be an integer".to_string(),
        })?;
        if page == 0 {
            return Err(ToolError {
                kind: errors::INVALID_INPUT,
                message: "page must be >= 1".to_string(),
            });
        }
        if seen.insert(page) {
            pages.push(page);
        }
    }

    if let Some(value) = args.get("pages") {
        let Some(array) = value.as_array() else {
            return Err(ToolError {
                kind: errors::INVALID_INPUT,
                message: "pages must be an array of integers".to_string(),
            });
        };
        for entry in array {
            let page = entry.as_u64().ok_or_else(|| ToolError {
                kind: errors::INVALID_INPUT,
                message: "pages must be an array of integers".to_string(),
            })?;
            if page == 0 {
                return Err(ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "pages must be >= 1".to_string(),
                });
            }
            if seen.insert(page) {
                pages.push(page);
            }
        }
    }

    if pages.is_empty() {
        pages.push(1);
    }

    Ok(pages)
}

fn enforce_size_limit(pages: &[RenderedPage]) -> Result<(), ToolError> {
    let size: u64 = pages.iter().map(|page| page.svg.len() as u64).sum();
    if size > MAX_SVG_OUTPUT_BYTES {
        return Err(ToolError {
            kind: errors::TOO_LARGE,
            message: format!("svg output exceeds limit: {size} bytes (max {MAX_SVG_OUTPUT_BYTES})"),
        });
    }
    Ok(())
}

fn render_inline(pages: &[RenderedPage]) -> Vec<Value> {
    pages
        .iter()
        .map(|page| json!({"page": page.page, "svg": page.svg}))
        .collect()
}

fn render_resource(pages: &[RenderedPage]) -> Result<Vec<Value>, ToolError> {
    let mut output = Vec::new();
    for page in pages {
        let path = svg_path_for_page(page.page);
        fs::write(&path, page.svg.as_bytes()).map_err(|err| ToolError {
            kind: errors::INTERNAL_ERROR,
            message: format!("failed to write svg output: {err}"),
        })?;
        let path_string = path.to_string_lossy().to_string();
        let uri = format!("file://{path_string}");
        output.push(json!({
            "page": page.page,
            "path": path_string,
            "uri": uri
        }));
    }
    Ok(output)
}

fn build_resource_content(pages: &[Value]) -> Vec<Value> {
    let mut content = Vec::new();
    content.push(json!({
        "type": "text",
        "text": format!("rendered {} page(s) as svg resources", pages.len())
    }));
    for page in pages {
        let uri = page
            .get("uri")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let page_number = page
            .get("page")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        content.push(json!({
            "type": "resource_link",
            "uri": uri,
            "name": format!("page-{page_number}"),
            "mimeType": "image/svg+xml"
        }));
    }
    content
}

fn svg_path_for_page(page: u64) -> PathBuf {
    let pid = std::process::id();
    let filename = format!("hwp-render-{pid}-page-{page}.svg");
    std::env::temp_dir().join(filename)
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

fn ensure_page_defs(document: &mut hwpers::HwpDocument) -> bool {
    let mut updated = false;
    for body_text in &mut document.body_texts {
        for section in &mut body_text.sections {
            if section.page_def.is_none() {
                section.page_def = Some(hwpers::model::page_def::PageDef::new_default());
                updated = true;
            }
        }
    }
    updated
}
