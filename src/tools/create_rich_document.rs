use crate::mcp::contracts::MAX_OUTPUT_BYTES;
use crate::mcp::errors;
use crate::tools::error_result;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hwpers::{HwpError, HwpWriter, HwpxWriter};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

pub fn call(args: &Value) -> Value {
    let to_format = match OutputFormat::parse(args.get("to")) {
        Ok(value) => value,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let output_path = match parse_output_path(args.get("output_path")) {
        Ok(path) => path,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let document = match parse_document_spec(args.get("document")) {
        Ok(doc) => doc,
        Err(err) => return error_result(err.kind, err.message, None),
    };

    let mut warnings: Vec<String> = Vec::new();

    let output_bytes = match to_format {
        OutputFormat::Hwp => match build_hwp(&document, &mut warnings) {
            Ok(bytes) => bytes,
            Err(err) => return error_result(err.kind, err.message, None),
        },
        OutputFormat::Hwpx => match build_hwpx(&document, &mut warnings) {
            Ok(bytes) => bytes,
            Err(err) => return error_result(err.kind, err.message, None),
        },
    };

    let bytes_len = output_bytes.len() as u64;

    match output_path {
        Some(path) => match write_output(&path, &output_bytes) {
            Ok(output) => json!({
                "content": output.content,
                "structuredContent": {
                    "to": to_format.as_str(),
                    "path": output.path,
                    "uri": output.uri,
                    "bytes_len": bytes_len,
                    "warnings": warnings
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
                    "text": format!("created rich document ({}) ({bytes_len} bytes)", to_format.as_str())
                }],
                "structuredContent": {
                    "to": to_format.as_str(),
                    "base64": base64,
                    "bytes_len": bytes_len,
                    "warnings": warnings
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

enum OutputFormat {
    Hwp,
    Hwpx,
}

impl OutputFormat {
    fn parse(value: Option<&Value>) -> Result<Self, ToolError> {
        let Some(value) = value else {
            return Ok(OutputFormat::Hwp);
        };
        let Some(value) = value.as_str() else {
            return Err(ToolError {
                kind: errors::INVALID_INPUT,
                message: "to must be a string".to_string(),
            });
        };
        match value {
            "hwp" => Ok(OutputFormat::Hwp),
            "hwpx" => Ok(OutputFormat::Hwpx),
            _ => Err(ToolError {
                kind: errors::INVALID_INPUT,
                message: "to must be hwp or hwpx".to_string(),
            }),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Hwp => "hwp",
            OutputFormat::Hwpx => "hwpx",
        }
    }
}

#[derive(Clone, Debug)]
struct DocumentSpec {
    title: Option<String>,
    author: Option<String>,
    header: Option<String>,
    footer: Option<String>,
    blocks: Vec<BlockSpec>,
}

#[derive(Clone, Debug)]
enum BlockSpec {
    Paragraph {
        text: String,
        style: Option<TextStyleSpec>,
    },
    Heading {
        level: u8,
        text: String,
    },
    Table {
        rows: Vec<Vec<TableCellSpec>>,
        header_row: bool,
        column_widths: Option<Vec<u32>>,
        border_style: Option<TableBorderStyle>,
    },
    Image {
        source: ImageSource,
        mime_type: Option<String>,
        width_mm: Option<u32>,
        height_mm: Option<u32>,
        caption: Option<String>,
        align: Option<ImageAlign>,
        wrap_text: Option<bool>,
    },
    PageBreak,
    List {
        items: Vec<String>,
        list_type: ListTypeSpec,
    },
}

#[derive(Clone, Debug)]
enum ListTypeSpec {
    Bullet,
    Numbered,
    Alphabetic,
    Roman,
    Korean,
}

#[derive(Clone, Debug)]
enum ImageSource {
    Base64 { data: Vec<u8>, mime_type: String },
    File { path: String },
}

#[derive(Clone, Debug)]
enum ImageAlign {
    Left,
    Center,
    Right,
    Inline,
}

#[derive(Clone, Debug)]
struct TableCellSpec {
    content: String,
    row_span: Option<u32>,
    col_span: Option<u32>,
    background_color: Option<u32>,
    text_align: Option<TextAlign>,
    style: Option<TextStyleSpec>,
}

#[derive(Clone, Debug)]
enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
enum TableBorderStyle {
    None,
    Basic,
    Full,
}

#[derive(Clone, Debug, Default)]
struct TextStyleSpec {
    font_name: Option<String>,
    font_size: Option<u32>,
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<u32>,
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

fn parse_document_spec(value: Option<&Value>) -> Result<DocumentSpec, ToolError> {
    let Some(value) = value else {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "document is required".to_string(),
        });
    };
    let Some(obj) = value.as_object() else {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "document must be an object".to_string(),
        });
    };

    let title = obj
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let author = obj
        .get("author")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let header = obj
        .get("header")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let footer = obj
        .get("footer")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let blocks_value = obj.get("blocks").ok_or_else(|| ToolError {
        kind: errors::INVALID_INPUT,
        message: "document.blocks is required".to_string(),
    })?;
    let Some(blocks_array) = blocks_value.as_array() else {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "document.blocks must be an array".to_string(),
        });
    };

    let mut blocks = Vec::with_capacity(blocks_array.len());
    for (idx, item) in blocks_array.iter().enumerate() {
        let block = parse_block(item).map_err(|mut err| {
            err.message = format!("document.blocks[{idx}]: {}", err.message);
            err
        })?;
        blocks.push(block);
    }

    Ok(DocumentSpec {
        title,
        author,
        header,
        footer,
        blocks,
    })
}

fn parse_block(value: &Value) -> Result<BlockSpec, ToolError> {
    let Some(obj) = value.as_object() else {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "block must be an object".to_string(),
        });
    };

    let block_type = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError {
            kind: errors::INVALID_INPUT,
            message: "block.type is required".to_string(),
        })?;

    match block_type {
        "paragraph" => {
            let text = obj
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "paragraph.text is required".to_string(),
                })?
                .to_string();
            let style = match obj.get("style") {
                None => None,
                Some(v) => Some(parse_text_style(v)?),
            };
            Ok(BlockSpec::Paragraph { text, style })
        }
        "heading" => {
            let level = obj
                .get("level")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "heading.level is required".to_string(),
                })?;
            let level_u8 = u8::try_from(level).unwrap_or(1).clamp(1, 6);
            let text = obj
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "heading.text is required".to_string(),
                })?
                .to_string();
            Ok(BlockSpec::Heading {
                level: level_u8,
                text,
            })
        }
        "table" => {
            let rows_value = obj.get("rows").ok_or_else(|| ToolError {
                kind: errors::INVALID_INPUT,
                message: "table.rows is required".to_string(),
            })?;
            let Some(rows_array) = rows_value.as_array() else {
                return Err(ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "table.rows must be an array".to_string(),
                });
            };

            let mut rows: Vec<Vec<TableCellSpec>> = Vec::with_capacity(rows_array.len());
            for row_value in rows_array {
                let Some(cols_array) = row_value.as_array() else {
                    return Err(ToolError {
                        kind: errors::INVALID_INPUT,
                        message: "table.rows items must be arrays".to_string(),
                    });
                };
                let mut row: Vec<TableCellSpec> = Vec::with_capacity(cols_array.len());
                for cell in cols_array {
                    let cell_spec = parse_table_cell(cell)?;
                    row.push(cell_spec);
                }
                rows.push(row);
            }

            if rows.is_empty() {
                return Err(ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "table.rows must not be empty".to_string(),
                });
            }
            let cols = rows[0].len();
            if cols == 0 {
                return Err(ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "table must have at least 1 column".to_string(),
                });
            }
            if rows.iter().any(|r| r.len() != cols) {
                return Err(ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "all table rows must have the same column count".to_string(),
                });
            }

            let header_row = obj
                .get("header_row")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let column_widths = obj
                .get("column_widths")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()))
                        .collect()
                });

            let border_style = obj
                .get("border_style")
                .and_then(|v| v.as_str())
                .map(|s| match s {
                    "none" => TableBorderStyle::None,
                    "basic" => TableBorderStyle::Basic,
                    "full" => TableBorderStyle::Full,
                    _ => TableBorderStyle::Basic,
                });

            Ok(BlockSpec::Table {
                rows,
                header_row,
                column_widths,
                border_style,
            })
        }
        "image" => {
            let width_mm = obj
                .get("width_mm")
                .and_then(|v| v.as_u64())
                .and_then(|v| u32::try_from(v).ok());
            let height_mm = obj
                .get("height_mm")
                .and_then(|v| v.as_u64())
                .and_then(|v| u32::try_from(v).ok());
            let caption = obj
                .get("caption")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let align = obj.get("align").and_then(|v| v.as_str()).map(|s| match s {
                "left" => ImageAlign::Left,
                "center" => ImageAlign::Center,
                "right" => ImageAlign::Right,
                "inline" => ImageAlign::Inline,
                _ => ImageAlign::Center,
            });
            let wrap_text = obj.get("wrap_text").and_then(|v| v.as_bool());

            if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                Ok(BlockSpec::Image {
                    source: ImageSource::File {
                        path: path.to_string(),
                    },
                    mime_type: None,
                    width_mm,
                    height_mm,
                    caption,
                    align,
                    wrap_text,
                })
            } else if let Some(data_base64) = obj.get("data_base64").and_then(|v| v.as_str()) {
                let mime_type = obj
                    .get("mimeType")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError {
                        kind: errors::INVALID_INPUT,
                        message: "image.mimeType is required when using data_base64".to_string(),
                    })?
                    .to_string();

                let data = STANDARD
                    .decode(data_base64.as_bytes())
                    .map_err(|_| ToolError {
                        kind: errors::INVALID_INPUT,
                        message: "image.data_base64 must be valid base64".to_string(),
                    })?;

                Ok(BlockSpec::Image {
                    source: ImageSource::Base64 { data, mime_type },
                    mime_type: None,
                    width_mm,
                    height_mm,
                    caption,
                    align,
                    wrap_text,
                })
            } else {
                Err(ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "image requires either 'path' (file path) or 'data_base64' (base64 encoded data)".to_string(),
                })
            }
        }
        "page_break" => Ok(BlockSpec::PageBreak),
        "list" => {
            let items_value = obj.get("items").ok_or_else(|| ToolError {
                kind: errors::INVALID_INPUT,
                message: "list.items is required".to_string(),
            })?;
            let Some(items_array) = items_value.as_array() else {
                return Err(ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "list.items must be an array".to_string(),
                });
            };
            let items: Vec<String> = items_array
                .iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect();

            let list_type = if let Some(type_str) = obj.get("list_type").and_then(|v| v.as_str()) {
                match type_str {
                    "bullet" => ListTypeSpec::Bullet,
                    "numbered" => ListTypeSpec::Numbered,
                    "alphabetic" => ListTypeSpec::Alphabetic,
                    "roman" => ListTypeSpec::Roman,
                    "korean" => ListTypeSpec::Korean,
                    _ => ListTypeSpec::Bullet,
                }
            } else {
                let ordered = obj
                    .get("ordered")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if ordered {
                    ListTypeSpec::Numbered
                } else {
                    ListTypeSpec::Bullet
                }
            };
            Ok(BlockSpec::List { items, list_type })
        }
        _ => Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: format!("unsupported block.type: {block_type}"),
        }),
    }
}

fn parse_text_style(value: &Value) -> Result<TextStyleSpec, ToolError> {
    let Some(obj) = value.as_object() else {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "style must be an object".to_string(),
        });
    };
    let font_name = obj
        .get("font_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let font_size = obj
        .get("font_size")
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok());
    let bold = obj.get("bold").and_then(|v| v.as_bool()).unwrap_or(false);
    let italic = obj.get("italic").and_then(|v| v.as_bool()).unwrap_or(false);
    let underline = obj
        .get("underline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let color = match obj.get("color") {
        None => None,
        Some(v) => {
            let Some(s) = v.as_str() else {
                return Err(ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "style.color must be a string".to_string(),
                });
            };
            Some(parse_color(s).map_err(|message| ToolError {
                kind: errors::INVALID_INPUT,
                message,
            })?)
        }
    };

    Ok(TextStyleSpec {
        font_name,
        font_size,
        bold,
        italic,
        underline,
        color,
    })
}

fn parse_color(value: &str) -> Result<u32, String> {
    let trimmed = value.trim();
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .or_else(|| trimmed.strip_prefix('#'))
        .unwrap_or(trimmed);
    if hex.len() != 6 {
        return Err("style.color must be 0xRRGGBB".to_string());
    }
    u32::from_str_radix(hex, 16).map_err(|_| "style.color must be valid hex".to_string())
}

fn parse_table_cell(value: &Value) -> Result<TableCellSpec, ToolError> {
    if let Some(text) = value.as_str() {
        return Ok(TableCellSpec {
            content: text.to_string(),
            row_span: None,
            col_span: None,
            background_color: None,
            text_align: None,
            style: None,
        });
    }

    let Some(obj) = value.as_object() else {
        return Err(ToolError {
            kind: errors::INVALID_INPUT,
            message: "table cell must be a string or an object".to_string(),
        });
    };

    let content = obj
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let row_span = obj
        .get("row_span")
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok());
    let col_span = obj
        .get("col_span")
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok());
    let background_color = match obj.get("background_color") {
        None => None,
        Some(v) => {
            let Some(s) = v.as_str() else {
                return Err(ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "cell.background_color must be a string".to_string(),
                });
            };
            Some(parse_color(s).map_err(|message| ToolError {
                kind: errors::INVALID_INPUT,
                message,
            })?)
        }
    };
    let text_align = obj
        .get("text_align")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "left" => TextAlign::Left,
            "center" => TextAlign::Center,
            "right" => TextAlign::Right,
            _ => TextAlign::Left,
        });
    let style = match obj.get("style") {
        None => None,
        Some(v) => Some(parse_text_style(v)?),
    };

    Ok(TableCellSpec {
        content,
        row_span,
        col_span,
        background_color,
        text_align,
        style,
    })
}

fn build_hwp(document: &DocumentSpec, warnings: &mut Vec<String>) -> Result<Vec<u8>, ToolError> {
    use hwpers::writer::style as hwp_style;

    let mut writer = HwpWriter::new();

    if let Some(title) = &document.title {
        writer
            .add_heading(title, 1)
            .map_err(|error| map_hwp_error_with_stage(error, "add title heading"))?;
    }
    if let Some(author) = &document.author {
        let text = format!("Author: {author}");
        let len = text.chars().count();
        let styled = hwp_style::StyledText::new(text).add_range(
            0,
            len,
            hwp_style::TextStyle::new().italic(),
        );
        writer
            .add_styled_paragraph(&styled)
            .map_err(|error| map_hwp_error_with_stage(error, "add author"))?;
    }
    if document.header.is_some() {
        warnings.push("hwp: document.header is not supported; ignoring".to_string());
    }
    if document.footer.is_some() {
        warnings.push("hwp: document.footer is not supported; ignoring".to_string());
    }

    for block in &document.blocks {
        match block {
            BlockSpec::Paragraph { text, style } => {
                if let Some(style) = style {
                    let mut ts = hwp_style::TextStyle::new();
                    if let Some(font) = &style.font_name {
                        ts = ts.font(font);
                    }
                    if let Some(size) = style.font_size {
                        ts = ts.size(size);
                    }
                    if style.bold {
                        ts = ts.bold();
                    }
                    if style.italic {
                        ts = ts.italic();
                    }
                    if style.underline {
                        ts = ts.underline();
                    }
                    if let Some(color) = style.color {
                        ts = ts.color(color);
                    }
                    let len = text.chars().count();
                    let styled = hwp_style::StyledText::new(text.clone()).add_range(0, len, ts);
                    writer
                        .add_styled_paragraph(&styled)
                        .map_err(|error| map_hwp_error_with_stage(error, "add styled paragraph"))?;
                } else {
                    writer
                        .add_paragraph(text)
                        .map_err(|error| map_hwp_error_with_stage(error, "add paragraph"))?;
                }
            }
            BlockSpec::Heading { level, text } => {
                writer
                    .add_heading(text, *level)
                    .map_err(|error| map_hwp_error_with_stage(error, "add heading"))?;
            }
            BlockSpec::Table {
                rows,
                header_row,
                column_widths,
                border_style,
            } => {
                let row_count = rows.len() as u32;
                let col_count = rows
                    .first()
                    .map(|r| r.len())
                    .unwrap_or(0)
                    .try_into()
                    .unwrap_or(0u32);

                let mut builder = writer
                    .add_table(row_count, col_count)
                    .set_header_row(*header_row);

                if column_widths.is_some() {
                    warnings.push(
                        "hwp: column_widths is not supported by hwpers 0.5.0; ignoring".to_string(),
                    );
                }

                if let Some(style) = border_style {
                    let border_line = match style {
                        TableBorderStyle::None => hwp_style::BorderLineStyle::none(),
                        TableBorderStyle::Basic => hwp_style::BorderLineStyle::solid(1),
                        TableBorderStyle::Full => hwp_style::BorderLineStyle::solid(2),
                    };
                    builder = builder.set_all_borders(border_line);
                }

                for (r, row) in rows.iter().enumerate() {
                    for (c, cell) in row.iter().enumerate() {
                        if let (Some(row_span), Some(col_span)) = (cell.row_span, cell.col_span) {
                            builder = builder.merge_cells(
                                r as u32,
                                c as u32,
                                row_span as u16,
                                col_span as u16,
                            );
                        } else if cell.row_span.is_some() {
                            let row_span = cell.row_span.unwrap();
                            builder = builder.merge_cells(r as u32, c as u32, row_span as u16, 1);
                        } else if cell.col_span.is_some() {
                            let col_span = cell.col_span.unwrap();
                            builder = builder.merge_cells(r as u32, c as u32, 1, col_span as u16);
                        }

                        if cell.background_color.is_some() {
                            warnings.push(format!(
                                "hwp: cell background_color at ({}, {}) is not supported; use table-level style instead",
                                r, c
                            ));
                        }
                        if cell.text_align.is_some() {
                            warnings.push(format!(
                                "hwp: cell text_align at ({}, {}) is not supported by hwpers 0.5.0",
                                r, c
                            ));
                        }
                        if cell.style.is_some() {
                            warnings.push(format!(
                                "hwp: cell style at ({}, {}) is not supported; use table-level header_style instead",
                                r, c
                            ));
                        }
                        builder = builder.set_cell(r as u32, c as u32, &cell.content);
                    }
                }
                builder
                    .finish()
                    .map_err(|error| map_hwp_error_with_stage(error, "add table"))?;
            }
            BlockSpec::Image {
                source,
                mime_type: _,
                width_mm,
                height_mm,
                caption,
                align,
                wrap_text,
            } => {
                let (data, format) = match source {
                    ImageSource::Base64 { data, mime_type } => {
                        let format = match mime_type.as_str() {
                            "image/png" => hwp_style::ImageFormat::Png,
                            "image/jpeg" => hwp_style::ImageFormat::Jpeg,
                            "image/gif" => hwp_style::ImageFormat::Gif,
                            "image/bmp" => hwp_style::ImageFormat::Bmp,
                            _ => {
                                return Err(ToolError {
                                    kind: errors::INVALID_INPUT,
                                    message: format!("unsupported image mimeType: {mime_type}"),
                                });
                            }
                        };
                        (data.clone(), format)
                    }
                    ImageSource::File { path } => {
                        let data = fs::read(path).map_err(|err| ToolError {
                            kind: errors::INVALID_INPUT,
                            message: format!("failed to read image file: {err}"),
                        })?;
                        let format = if data.len() >= 8 {
                            if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                                Some(hwp_style::ImageFormat::Png)
                            } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
                                Some(hwp_style::ImageFormat::Jpeg)
                            } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
                                Some(hwp_style::ImageFormat::Gif)
                            } else if data.starts_with(b"BM") {
                                Some(hwp_style::ImageFormat::Bmp)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                        .ok_or_else(|| ToolError {
                            kind: errors::INVALID_INPUT,
                            message: "unable to detect image format from file".to_string(),
                        })?;
                        (data, format)
                    }
                };

                let mut options = hwp_style::ImageOptions::new();
                if let Some(w) = width_mm {
                    options = options.width(*w);
                }
                if let Some(h) = height_mm {
                    options = options.height(*h);
                }
                if let Some(caption) = caption {
                    options = options.caption(caption);
                }
                if let Some(align) = align {
                    let hwp_align = match align {
                        ImageAlign::Left => hwp_style::ImageAlign::Left,
                        ImageAlign::Center => hwp_style::ImageAlign::Center,
                        ImageAlign::Right => hwp_style::ImageAlign::Right,
                        ImageAlign::Inline => hwp_style::ImageAlign::InlineWithText,
                    };
                    options = options.align(hwp_align);
                }
                if let Some(true) = wrap_text {
                    options = options.wrap_text(true);
                }
                writer
                    .add_image_with_options(&data, format, &options)
                    .map_err(|error| map_hwp_error_with_stage(error, "add image"))?;
            }
            BlockSpec::PageBreak => {
                warnings.push("hwp: page_break is not fully supported".to_string());
                writer
                    .add_paragraph("")
                    .map_err(|error| map_hwp_error_with_stage(error, "add page break"))?;
            }
            BlockSpec::List { items, list_type } => {
                let hwp_list_type = match list_type {
                    ListTypeSpec::Bullet => hwp_style::ListType::Bullet,
                    ListTypeSpec::Numbered => hwp_style::ListType::Numbered,
                    ListTypeSpec::Alphabetic => hwp_style::ListType::Alphabetic,
                    ListTypeSpec::Roman => hwp_style::ListType::Roman,
                    ListTypeSpec::Korean => hwp_style::ListType::Korean,
                };

                let items_ref: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
                writer
                    .add_list(&items_ref, hwp_list_type)
                    .map_err(|error| map_hwp_error_with_stage(error, "add list"))?;
            }
        }
    }

    writer
        .to_bytes()
        .map_err(|error| map_hwp_error_with_stage(error, "write document"))
}

fn build_hwpx(document: &DocumentSpec, warnings: &mut Vec<String>) -> Result<Vec<u8>, ToolError> {
    use hwpers::hwpx::{HwpxImage, HwpxTable, HwpxTextStyle};

    let mut writer = HwpxWriter::new();

    if let Some(header) = &document.header {
        writer.add_header(header);
    }
    if let Some(footer) = &document.footer {
        writer.add_footer(footer);
    }

    if let Some(title) = &document.title {
        let style = HwpxTextStyle::new().size(24).bold();
        writer
            .add_styled_paragraph(title, style)
            .map_err(|err| map_hwp_error_with_stage(err, "add title"))?;
    }
    if let Some(author) = &document.author {
        let style = HwpxTextStyle::new().italic();
        writer
            .add_styled_paragraph(&format!("Author: {author}"), style)
            .map_err(|err| map_hwp_error_with_stage(err, "add author"))?;
    }

    for block in &document.blocks {
        match block {
            BlockSpec::Paragraph { text, style } => {
                if let Some(style) = style {
                    let mut ts = HwpxTextStyle::new();
                    ts.font_name = style.font_name.clone();
                    if let Some(size) = style.font_size {
                        ts = ts.size(size);
                    }
                    if style.bold {
                        ts = ts.bold();
                    }
                    if style.italic {
                        ts = ts.italic();
                    }
                    if style.underline {
                        ts = ts.underline();
                    }
                    if let Some(color) = style.color {
                        ts = ts.color(color);
                    }
                    writer
                        .add_styled_paragraph(text, ts)
                        .map_err(|err| map_hwp_error_with_stage(err, "add styled paragraph"))?;
                } else {
                    writer
                        .add_paragraph(text)
                        .map_err(|err| map_hwp_error_with_stage(err, "add paragraph"))?;
                }
            }
            BlockSpec::Heading { level, text } => {
                let size = match level {
                    1 => 24,
                    2 => 18,
                    3 => 14,
                    4 => 12,
                    _ => 11,
                };
                let style = HwpxTextStyle::new().size(size).bold();
                writer
                    .add_styled_paragraph(text, style)
                    .map_err(|err| map_hwp_error_with_stage(err, "add heading"))?;
            }
            BlockSpec::Table {
                rows,
                header_row: _,
                column_widths,
                border_style,
            } => {
                let row_count = rows.len();
                let col_count = rows.first().map(|r| r.len()).unwrap_or(0);
                let mut table = HwpxTable::new(row_count, col_count);

                if column_widths.is_some() {
                    warnings.push("hwpx: column_widths is not supported; ignoring".to_string());
                }
                if border_style.is_some() {
                    warnings.push("hwpx: border_style is not supported; ignoring".to_string());
                }

                for (r, row) in rows.iter().enumerate() {
                    for (c, cell) in row.iter().enumerate() {
                        if cell.row_span.is_some() || cell.col_span.is_some() {
                            warnings.push(format!(
                                "hwpx: cell merging (row_span/col_span) at ({}, {}) is not supported",
                                r, c
                            ));
                        }
                        if cell.background_color.is_some() {
                            warnings.push(format!(
                                "hwpx: cell background_color at ({}, {}) is not supported",
                                r, c
                            ));
                        }
                        if cell.text_align.is_some() {
                            warnings.push(format!(
                                "hwpx: cell text_align at ({}, {}) is not supported",
                                r, c
                            ));
                        }
                        if cell.style.is_some() {
                            warnings.push(format!(
                                "hwpx: cell style at ({}, {}) is not supported",
                                r, c
                            ));
                        }
                        table.set_cell(r, c, &cell.content);
                    }
                }
                writer
                    .add_table(table)
                    .map_err(|err| map_hwp_error_with_stage(err, "add table"))?;
            }
            BlockSpec::Image {
                source,
                mime_type: _,
                width_mm,
                height_mm,
                caption,
                align: _,
                wrap_text: _,
            } => {
                let data = match source {
                    ImageSource::Base64 { data, .. } => data.clone(),
                    ImageSource::File { path } => fs::read(path).map_err(|err| ToolError {
                        kind: errors::INVALID_INPUT,
                        message: format!("failed to read image file: {err}"),
                    })?,
                };

                let mut image = HwpxImage::from_bytes(data).ok_or_else(|| ToolError {
                    kind: errors::INVALID_INPUT,
                    message: "unsupported image format bytes".to_string(),
                })?;

                if let (Some(w), Some(h)) = (width_mm, height_mm) {
                    image = image.with_size(*w, *h);
                }

                writer
                    .add_image(image)
                    .map_err(|err| map_hwp_error_with_stage(err, "add image"))?;
                if let Some(caption) = caption {
                    writer
                        .add_paragraph(&format!("그림: {caption}"))
                        .map_err(|err| map_hwp_error_with_stage(err, "add image caption"))?;
                }
            }
            BlockSpec::PageBreak => {
                warnings.push(
                    "hwpx: page_break is not fully supported; adding empty paragraph".to_string(),
                );
                writer
                    .add_paragraph("")
                    .map_err(|err| map_hwp_error_with_stage(err, "add page break"))?;
            }
            BlockSpec::List { items, list_type } => {
                let list_type_name = match list_type {
                    ListTypeSpec::Bullet => "bullet",
                    ListTypeSpec::Numbered => "numbered",
                    ListTypeSpec::Alphabetic => "alphabetic",
                    ListTypeSpec::Roman => "roman",
                    ListTypeSpec::Korean => "korean",
                };
                warnings.push(format!(
                    "hwpx: list type '{}' is not fully supported; using basic formatting",
                    list_type_name
                ));
                for (idx, item) in items.iter().enumerate() {
                    let prefix = match list_type {
                        ListTypeSpec::Bullet => "•".to_string(),
                        ListTypeSpec::Numbered => format!("{}.", idx + 1),
                        ListTypeSpec::Alphabetic => format!("{}.", (b'a' + idx as u8) as char),
                        ListTypeSpec::Roman => format!("{}.", idx + 1),
                        ListTypeSpec::Korean => format!("{}.", idx + 1),
                    };
                    writer
                        .add_paragraph(&format!("{} {}", prefix, item))
                        .map_err(|err| map_hwp_error_with_stage(err, "add list item"))?;
                }
            }
        }
    }

    writer
        .to_bytes()
        .map_err(|error| map_hwp_error_with_stage(error, "write document"))
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
