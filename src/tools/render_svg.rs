use crate::constants::MAX_SVG_OUTPUT_BYTES;
use crate::errors::AppError;
use crate::input::{InputFormat, load_input};
use crate::tools::error_result;
use hwpers::render::renderer::{HwpRenderer, RenderOptions};
use hwpers::{HwpError, HwpReader, HwpxReader};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Request {
    pub payload: crate::input::InputPayload,
    pub pages: Vec<u64>,
    pub output: OutputMode,
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
        pages: parse_pages(args)?,
        output: OutputMode::parse(args.get("output"))?,
    })
}

pub fn run(req: Request) -> Result<Response, AppError> {
    let Request {
        payload,
        pages,
        output,
    } = req;

    let mut parsed = parse_document(&payload.bytes, payload.format)?;

    if ensure_page_defs(&mut parsed.document) {
        parsed
            .warnings
            .push("missing page definition; default layout applied".to_string());
    }

    let renderer = HwpRenderer::new(&parsed.document, RenderOptions::default());
    let render_result = renderer.render();

    let mut rendered_pages = Vec::new();
    for page in pages {
        let page_index = usize::try_from(page.saturating_sub(1))
            .map_err(|_| AppError::invalid_input("page index out of range"))?;
        let Some(svg) = render_result.to_svg(page_index) else {
            return Err(AppError::invalid_input(format!(
                "page out of range: {page}"
            )));
        };
        rendered_pages.push(RenderedPage { page, svg });
    }

    enforce_size_limit(&rendered_pages)?;

    let output_mode = output.clone();
    let structured_pages = match output_mode {
        OutputMode::Inline => render_inline(&rendered_pages),
        OutputMode::Resource => render_resource(&rendered_pages)?,
    };

    let content = match output {
        OutputMode::Inline => vec![json!({
            "type": "text",
            "text": format!("rendered {} page(s) as svg", rendered_pages.len())
        })],
        OutputMode::Resource => build_resource_content(&structured_pages),
    };

    Ok(Response {
        content,
        structured_content: json!({
            "format": parsed.format.as_str(),
            "pages": structured_pages,
            "warnings": parsed.warnings
        }),
    })
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

#[derive(Debug, Clone)]
pub enum OutputMode {
    Inline,
    Resource,
}

impl OutputMode {
    fn parse(value: Option<&Value>) -> Result<Self, AppError> {
        let Some(value) = value else {
            return Ok(OutputMode::Inline);
        };
        let Some(value) = value.as_str() else {
            return Err(AppError::invalid_input("output must be a string"));
        };
        match value {
            "inline" => Ok(OutputMode::Inline),
            "resource" => Ok(OutputMode::Resource),
            _ => Err(AppError::invalid_input("output must be inline or resource")),
        }
    }
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

fn parse_pages(args: &Value) -> Result<Vec<u64>, AppError> {
    let mut pages = Vec::new();
    let mut seen = HashSet::new();

    if let Some(value) = args.get("page") {
        let page = value
            .as_u64()
            .ok_or_else(|| AppError::invalid_input("page must be an integer"))?;
        if page == 0 {
            return Err(AppError::invalid_input("page must be >= 1"));
        }
        if seen.insert(page) {
            pages.push(page);
        }
    }

    if let Some(value) = args.get("pages") {
        let Some(array) = value.as_array() else {
            return Err(AppError::invalid_input(
                "pages must be an array of integers",
            ));
        };
        for entry in array {
            let page = entry
                .as_u64()
                .ok_or_else(|| AppError::invalid_input("pages must be an array of integers"))?;
            if page == 0 {
                return Err(AppError::invalid_input("pages must be >= 1"));
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

fn enforce_size_limit(pages: &[RenderedPage]) -> Result<(), AppError> {
    let size: u64 = pages.iter().map(|page| page.svg.len() as u64).sum();
    if size > MAX_SVG_OUTPUT_BYTES {
        return Err(AppError::too_large(format!(
            "svg output exceeds limit: {size} bytes (max {MAX_SVG_OUTPUT_BYTES})"
        )));
    }
    Ok(())
}

fn render_inline(pages: &[RenderedPage]) -> Vec<Value> {
    pages
        .iter()
        .map(|page| json!({"page": page.page, "svg": page.svg}))
        .collect()
}

fn render_resource(pages: &[RenderedPage]) -> Result<Vec<Value>, AppError> {
    let mut output = Vec::new();
    for page in pages {
        let path = svg_path_for_page(page.page);
        fs::write(&path, page.svg.as_bytes())
            .map_err(|err| AppError::internal(format!("failed to write svg output: {err}")))?;
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
