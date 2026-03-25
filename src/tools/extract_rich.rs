use crate::constants::MAX_OUTPUT_BYTES;
use crate::errors::AppError;
use crate::input::{InputFormat, InputPayload, load_input};
use crate::tools::error_result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hwpers::model::bin_data::BinData;
use hwpers::{HwpError, HwpReader, HwpxReader};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct Request {
    pub payload: InputPayload,
    pub images: String,
    pub max_image_bytes: u64,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub format: String,
    pub blocks: Vec<Value>,
    pub warnings: Vec<String>,
}

pub fn request_from_value(args: &Value) -> Result<Request, AppError> {
    let payload = load_input(args)?;
    Ok(Request {
        payload,
        images: args
            .get("images")
            .and_then(|v| v.as_str())
            .unwrap_or("metadata")
            .to_string(),
        max_image_bytes: args
            .get("max_image_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        output_path: args
            .get("output_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

pub fn run(req: Request) -> Result<Response, AppError> {
    let parsed = parse_document(&req.payload.bytes, req.payload.format)?;

    let mut warnings = parsed.warnings;
    let mut blocks: Vec<Value> = Vec::new();
    let mut total_inline_image_bytes: u64 = 0;
    let images = parsed.document.get_images();
    let mut image_cursor: usize = 0;

    for (section_index, section) in parsed.document.sections().enumerate() {
        let paragraphs = &section.paragraphs;
        let mut i: usize = 0;
        while i < paragraphs.len() {
            let paragraph = &paragraphs[i];

            // Prefer structured control data when available.
            if let Some(table) = paragraph.table_data.as_ref() {
                let rows = usize::from(table.rows);
                let cols = usize::from(table.cols);

                let mut cells = table.cells.iter().collect::<Vec<_>>();
                cells.sort_by_key(|cell| (cell.cell_address.0, cell.cell_address.1));

                let cell_para_start = i.saturating_add(1);
                let mut cell_texts: Vec<String> = Vec::with_capacity(cells.len());
                for cell_idx in 0..cells.len() {
                    let para_idx = cell_para_start + cell_idx;
                    let text = paragraphs
                        .get(para_idx)
                        .map(paragraph_text)
                        .unwrap_or_default();
                    cell_texts.push(text);
                }
                if cell_para_start + cells.len() > paragraphs.len() {
                    warnings.push(format!(
                        "table at section {section_index} paragraph {i}: expected {} cell paragraphs but only {} remain",
                        cells.len(),
                        paragraphs.len().saturating_sub(cell_para_start)
                    ));
                }

                let mut grid: Vec<Vec<String>> = Vec::with_capacity(rows);
                for _ in 0..rows {
                    grid.push(vec![String::new(); cols]);
                }

                let mut spans: Vec<Value> = Vec::new();
                for (idx, cell) in cells.iter().enumerate() {
                    let r = usize::from(cell.cell_address.0);
                    let c = usize::from(cell.cell_address.1);
                    if r < rows && c < cols {
                        grid[r][c] = cell_texts.get(idx).cloned().unwrap_or_default();
                    }
                    if cell.row_span > 1 || cell.col_span > 1 {
                        spans.push(json!({
                            "row": cell.cell_address.0,
                            "col": cell.cell_address.1,
                            "row_span": cell.row_span,
                            "col_span": cell.col_span
                        }));
                    }
                }

                blocks.push(json!({
                    "type": "table",
                    "section_index": section_index,
                    "paragraph_index": i,
                    "rows": grid,
                    "spans": spans,
                    "inferred": false,
                    "cells_count": cells.len()
                }));

                // Skip over the following cell paragraphs that belong to this table.
                i = cell_para_start.saturating_add(cells.len());
                continue;
            }

            let current_text = paragraph_text(paragraph);
            let current_trim = current_text.trim();

            if current_trim.is_empty() {
                // Heuristic: empty paragraph followed by a caption paragraph -> image
                if i + 1 < paragraphs.len() {
                    let next_text = paragraph_text(&paragraphs[i + 1]);
                    if next_text.trim_start().starts_with("그림:") {
                        let caption_line = next_text.trim().to_string();
                        let caption = caption_line
                            .strip_prefix("그림:")
                            .map(|s| s.trim().to_string());

                        let mut image_ctx = ImageRenderContext {
                            images_mode: req.images.as_str(),
                            max_image_bytes: req.max_image_bytes,
                            total_inline_image_bytes: &mut total_inline_image_bytes,
                            warnings: &mut warnings,
                            output_path: &req.output_path,
                        };

                        if image_cursor < images.len() {
                            let bin = images[image_cursor];
                            image_cursor += 1;
                            let block = image_block_from_bin(
                                section_index,
                                i,
                                bin,
                                caption,
                                &mut image_ctx,
                            )?;
                            blocks.push(block);
                        } else {
                            warnings.push(
                                "image bytes are not available from parser; returning caption-only image block"
                                    .to_string(),
                            );
                            blocks.push(json!({
                                "type": "image",
                                "section_index": section_index,
                                "paragraph_index": i,
                                "caption": caption,
                                "bytes_len": null,
                                "mimeType": null,
                                "note": "image data not available"
                            }));
                        }

                        i += 2;
                        continue;
                    }
                }

                // Fallback: empty paragraph followed by multiple non-empty paragraphs -> infer a table.
                let mut j = i + 1;
                while j < paragraphs.len() {
                    let t = paragraph_text(&paragraphs[j]);
                    if t.trim().is_empty() {
                        break;
                    }
                    j += 1;
                }
                let cell_count = j.saturating_sub(i + 1);
                if cell_count >= 2 {
                    let mut cells: Vec<String> = Vec::with_capacity(cell_count);
                    for paragraph in paragraphs.iter().take(j).skip(i + 1) {
                        cells.push(paragraph_text(paragraph).trim().to_string());
                    }

                    let (rows, cols) = infer_table_dims(cells.len());
                    let mut rows_out: Vec<Vec<String>> = Vec::with_capacity(rows);
                    for r in 0..rows {
                        let mut row: Vec<String> = Vec::with_capacity(cols);
                        for c in 0..cols {
                            let idx = r * cols + c;
                            row.push(cells.get(idx).cloned().unwrap_or_default());
                        }
                        rows_out.push(row);
                    }

                    blocks.push(json!({
                        "type": "table",
                        "section_index": section_index,
                        "paragraph_index": i,
                        "rows": rows_out,
                        "inferred": true,
                        "cells_count": cells.len()
                    }));

                    // If the next paragraph is an empty anchor for an image caption, don't consume it.
                    if j < paragraphs.len()
                        && paragraph_text(&paragraphs[j]).trim().is_empty()
                        && j + 1 < paragraphs.len()
                        && paragraph_text(&paragraphs[j + 1])
                            .trim_start()
                            .starts_with("그림:")
                    {
                        i = j;
                    } else {
                        i = j;
                        if i < paragraphs.len() && paragraph_text(&paragraphs[i]).trim().is_empty()
                        {
                            i += 1;
                        }
                    }
                    continue;
                }

                blocks.push(json!({
                    "type": "paragraph",
                    "section_index": section_index,
                    "paragraph_index": i,
                    "text": ""
                }));
                i += 1;
                continue;
            }

            // Heuristic: treat caption paragraphs as the anchor for the next image.
            if current_trim.starts_with("그림:") {
                let caption = current_trim
                    .strip_prefix("그림:")
                    .map(|s| s.trim().to_string());

                let mut image_ctx = ImageRenderContext {
                    images_mode: req.images.as_str(),
                    max_image_bytes: req.max_image_bytes,
                    total_inline_image_bytes: &mut total_inline_image_bytes,
                    warnings: &mut warnings,
                    output_path: &req.output_path,
                };

                if image_cursor < images.len() {
                    let bin = images[image_cursor];
                    image_cursor += 1;
                    let block =
                        image_block_from_bin(section_index, i, bin, caption, &mut image_ctx)?;
                    blocks.push(block);
                    i += 1;
                    continue;
                }
            }

            blocks.push(json!({
                "type": "paragraph",
                "section_index": section_index,
                "paragraph_index": i,
                "text": current_text
            }));
            i += 1;
        }
    }

    // Any remaining embedded images without obvious anchors
    while image_cursor < images.len() {
        let bin = images[image_cursor];
        image_cursor += 1;

        let mut image_ctx = ImageRenderContext {
            images_mode: req.images.as_str(),
            max_image_bytes: req.max_image_bytes,
            total_inline_image_bytes: &mut total_inline_image_bytes,
            warnings: &mut warnings,
            output_path: &req.output_path,
        };
        let block = image_block_from_bin(0, 0, bin, None, &mut image_ctx)?;
        let mut block = block;
        if let Some(obj) = block.as_object_mut() {
            obj.insert("placement".to_string(), json!("unanchored"));
        }
        blocks.push(block);
    }

    Ok(Response {
        format: parsed.format.as_str().to_string(),
        blocks,
        warnings,
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

    json!({
        "content": [{
            "type": "text",
            "text": format!("extracted {} blocks", response.blocks.len())
        }],
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

fn paragraph_text(paragraph: &hwpers::model::paragraph::Paragraph) -> String {
    match &paragraph.text {
        Some(text) => text.content.clone(),
        None => String::new(),
    }
}

fn infer_table_dims(cell_count: usize) -> (usize, usize) {
    if cell_count == 0 {
        return (0, 0);
    }

    let mut best_rows = 1usize;
    let mut best_cols = cell_count;
    let mut best_diff = best_cols.saturating_sub(best_rows);

    let mut r = 1usize;
    while r * r <= cell_count {
        if cell_count.is_multiple_of(r) {
            let c = cell_count / r;
            let (rows, cols) = if r <= c { (r, c) } else { (c, r) };
            let diff = cols.saturating_sub(rows);
            if diff < best_diff {
                best_rows = rows;
                best_cols = cols;
                best_diff = diff;
            }
        }
        r += 1;
    }

    (best_rows, best_cols)
}

struct ImageRenderContext<'a> {
    images_mode: &'a str,
    max_image_bytes: u64,
    total_inline_image_bytes: &'a mut u64,
    warnings: &'a mut Vec<String>,
    output_path: &'a Option<String>,
}

fn image_block_from_bin(
    section_index: usize,
    paragraph_index: usize,
    bin: &BinData,
    caption: Option<String>,
    ctx: &mut ImageRenderContext<'_>,
) -> Result<Value, AppError> {
    let bin_id = bin.bin_id;
    let bytes = match bin.get_data() {
        Ok(bytes) => bytes,
        Err(err) => {
            ctx.warnings
                .push(format!("failed to load image data bin_id={bin_id}: {err}"));
            Vec::new()
        }
    };
    let bytes_len = bytes.len() as u64;

    let mut block = json!({
        "type": "image",
        "section_index": section_index,
        "paragraph_index": paragraph_index,
        "bin_id": bin_id,
        "bytes_len": bytes_len,
        "extension": bin.extension,
        "mimeType": mime_from_extension(&bin.extension),
    });
    if let (Some(obj), Some(caption)) = (block.as_object_mut(), caption) {
        obj.insert("caption".to_string(), json!(caption));
    }

    match ctx.images_mode {
        "none" => {}
        "metadata" => {}
        "inline" => {
            if ctx.max_image_bytes > 0 && bytes_len > ctx.max_image_bytes {
                ctx.warnings.push(format!(
                    "image bin_id={bin_id} exceeds max_image_bytes ({bytes_len} > {}); returning metadata",
                    ctx.max_image_bytes
                ));
            } else {
                *ctx.total_inline_image_bytes += bytes_len;
                if *ctx.total_inline_image_bytes > MAX_OUTPUT_BYTES {
                    return Err(AppError::too_large(format!(
                        "inline images exceed output limit: {} bytes (max {MAX_OUTPUT_BYTES})",
                        *ctx.total_inline_image_bytes
                    )));
                }
                if let Some(obj) = block.as_object_mut() {
                    obj.insert("base64".to_string(), json!(STANDARD.encode(&bytes)));
                }
            }
        }
        "resource" => {
            let ext = if bin.extension.trim().is_empty() {
                "bin"
            } else {
                bin.extension.as_str()
            };
            let path = write_image_file(bin_id, ext, &bytes, ctx.output_path).map_err(|err| {
                AppError::internal(format!("failed to write image bin_id={bin_id}: {err}"))
            })?;
            let uri = format!("file://{}", path.to_string_lossy());
            if let Some(obj) = block.as_object_mut() {
                obj.insert("path".to_string(), json!(path.to_string_lossy()));
                obj.insert("uri".to_string(), json!(uri));
            }
        }
        _ => {
            return Err(AppError::invalid_input(
                "images must be none, metadata, inline, or resource",
            ));
        }
    }

    Ok(block)
}

fn mime_from_extension(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

fn write_image_file(
    bin_id: u16,
    ext: &str,
    bytes: &[u8],
    output_path: &Option<String>,
) -> Result<PathBuf, String> {
    let mut path = if let Some(custom_path) = output_path {
        let custom = PathBuf::from(custom_path);
        fs::create_dir_all(&custom).map_err(|e| e.to_string())?;
        custom
    } else {
        let mut temp = std::env::temp_dir();
        temp.push("mcp-hwp");
        fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
        temp
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let pid = std::process::id();
    let filename = format!("image-{pid}-{now}-{bin_id}.{ext}");
    path.push(filename);
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
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
