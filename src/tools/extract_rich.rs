use crate::input::{InputFormat, load_input};
use crate::mcp::contracts::MAX_OUTPUT_BYTES;
use crate::mcp::errors;
use crate::tools::error_result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hwpers::model::bin_data::BinData;
use hwpers::{HwpError, HwpReader, HwpxReader};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn call(args: &Value) -> Value {
    let payload = match load_input(args) {
        Ok(payload) => payload,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let images_mode = args
        .get("images")
        .and_then(|v| v.as_str())
        .unwrap_or("metadata");
    let max_image_bytes = args
        .get("max_image_bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output_path = args
        .get("output_path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let parsed = match parse_document(&payload.bytes, payload.format) {
        Ok(parsed) => parsed,
        Err(err) => return error_result(err.kind, err.message, Some(payload.source.as_str())),
    };

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
                            images_mode,
                            max_image_bytes,
                            total_inline_image_bytes: &mut total_inline_image_bytes,
                            source: &payload.source,
                            warnings: &mut warnings,
                            output_path: &output_path,
                        };

                        if image_cursor < images.len() {
                            let bin = images[image_cursor];
                            image_cursor += 1;
                            let block = match image_block_from_bin(
                                section_index,
                                i,
                                bin,
                                caption,
                                &mut image_ctx,
                            ) {
                                Ok(block) => block,
                                Err(tool_result) => return tool_result,
                            };
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
                    for idx in (i + 1)..j {
                        cells.push(paragraph_text(&paragraphs[idx]).trim().to_string());
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
                    images_mode,
                    max_image_bytes,
                    total_inline_image_bytes: &mut total_inline_image_bytes,
                    source: &payload.source,
                    warnings: &mut warnings,
                    output_path: &output_path,
                };

                if image_cursor < images.len() {
                    let bin = images[image_cursor];
                    image_cursor += 1;
                    let block = match image_block_from_bin(
                        section_index,
                        i,
                        bin,
                        caption,
                        &mut image_ctx,
                    ) {
                        Ok(block) => block,
                        Err(tool_result) => return tool_result,
                    };
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
            images_mode,
            max_image_bytes,
            total_inline_image_bytes: &mut total_inline_image_bytes,
            source: &payload.source,
            warnings: &mut warnings,
            output_path: &output_path,
        };
        let block = match image_block_from_bin(0, 0, bin, None, &mut image_ctx) {
            Ok(block) => block,
            Err(tool_result) => return tool_result,
        };
        let mut block = block;
        if let Some(obj) = block.as_object_mut() {
            obj.insert("placement".to_string(), json!("unanchored"));
        }
        blocks.push(block);
    }

    json!({
        "content": [{
            "type": "text",
            "text": format!("extracted {} blocks", blocks.len())
        }],
        "structuredContent": {
            "format": parsed.format.as_str(),
            "blocks": blocks,
            "warnings": warnings
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

fn mime_from_extension(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

fn normalize_paragraph_text(value: &str) -> String {
    value.trim_end_matches(&['\r', '\n'][..]).to_string()
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
        if cell_count % r == 0 {
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
    source: &'a str,
    warnings: &'a mut Vec<String>,
    output_path: &'a Option<String>,
}

fn image_block_from_bin(
    section_index: usize,
    paragraph_index: usize,
    bin: &BinData,
    caption: Option<String>,
    ctx: &mut ImageRenderContext<'_>,
) -> Result<Value, Value> {
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
                    return Err(error_result(
                        errors::TOO_LARGE,
                        format!(
                            "inline images exceed output limit: {} bytes (max {MAX_OUTPUT_BYTES})",
                            *ctx.total_inline_image_bytes
                        ),
                        Some(ctx.source),
                    ));
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
                error_result(
                    errors::INTERNAL_ERROR,
                    format!("failed to write image bin_id={bin_id}: {err}"),
                    Some(ctx.source),
                )
            })?;
            let uri = format!("file://{}", path.to_string_lossy());
            if let Some(obj) = block.as_object_mut() {
                obj.insert("path".to_string(), json!(path.to_string_lossy()));
                obj.insert("uri".to_string(), json!(uri));
            }
        }
        _ => {
            return Err(error_result(
                errors::INVALID_INPUT,
                "images must be none, metadata, inline, or resource",
                Some(ctx.source),
            ));
        }
    }

    Ok(block)
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
