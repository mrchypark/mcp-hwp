#![allow(dead_code)]

use serde_json::json;

pub const TOOL_EXTRACT_TEXT: &str = "hwp.extract_text";
pub const TOOL_INSPECT_METADATA: &str = "hwp.inspect_metadata";
pub const TOOL_SUMMARIZE_STRUCTURE: &str = "hwp.summarize_structure";
pub const TOOL_RENDER_SVG: &str = "hwp.render_svg";
pub const TOOL_CONVERT: &str = "hwp.convert";
pub const TOOL_CREATE_DOCUMENT: &str = "hwp.create_document";
pub const TOOL_CREATE_RICH_DOCUMENT: &str = "hwp.create_rich_document";
pub const TOOL_EXTRACT_RICH: &str = "hwp.extract_rich";

pub fn tool_definitions() -> Vec<serde_json::Value> {
    vec![
        json!({
            "name": TOOL_EXTRACT_TEXT,
            "description": "Extract plain text from HWP documents.",
            "inputSchema": extract_text_schema()
        }),
        json!({
            "name": TOOL_INSPECT_METADATA,
            "description": "Inspect metadata from HWP documents.",
            "inputSchema": inspect_metadata_schema()
        }),
        json!({
            "name": TOOL_SUMMARIZE_STRUCTURE,
            "description": "Summarize document structure for HWP documents.",
            "inputSchema": summarize_structure_schema()
        }),
        json!({
            "name": TOOL_RENDER_SVG,
            "description": "Render HWP pages or elements into SVG.",
            "inputSchema": render_svg_schema()
        }),
        json!({
            "name": TOOL_CONVERT,
            "description": "Convert HWP documents between formats.",
            "inputSchema": convert_schema()
        }),
        json!({
            "name": TOOL_CREATE_DOCUMENT,
            "description": "Create new HWP documents from text.",
            "inputSchema": create_document_schema()
        }),
        json!({
            "name": TOOL_CREATE_RICH_DOCUMENT,
            "description": "Create a rich HWP/HWPX document from a block-based JSON spec.",
            "inputSchema": create_rich_document_schema()
        }),
        json!({
            "name": TOOL_EXTRACT_RICH,
            "description": "Extract a rich block structure from HWP/HWPX documents.",
            "inputSchema": extract_rich_schema()
        }),
    ]
}

pub fn extract_text_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "base64": { "type": "string" },
            "format": { "type": "string", "enum": ["auto", "hwp", "hwpx"] },
            "max_chars": { "type": "integer", "minimum": 0 },
            "include_newlines": { "type": "boolean" },
            "normalize_whitespace": { "type": "boolean" }
        },
        "oneOf": [
            { "required": ["path"] },
            { "required": ["base64"] }
        ],
        "additionalProperties": false
    })
}

pub fn inspect_metadata_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "base64": { "type": "string" },
            "format": { "type": "string", "enum": ["auto", "hwp", "hwpx"] }
        },
        "oneOf": [
            { "required": ["path"] },
            { "required": ["base64"] }
        ],
        "additionalProperties": false
    })
}

pub fn summarize_structure_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "base64": { "type": "string" },
            "format": { "type": "string", "enum": ["auto", "hwp", "hwpx"] },
            "max_sections": { "type": "integer", "minimum": 0 },
            "max_paragraphs_per_section": { "type": "integer", "minimum": 0 },
            "preview_chars": { "type": "integer", "minimum": 0 }
        },
        "oneOf": [
            { "required": ["path"] },
            { "required": ["base64"] }
        ],
        "additionalProperties": false
    })
}

pub fn render_svg_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "base64": { "type": "string" },
            "format": { "type": "string", "enum": ["auto", "hwp", "hwpx"] },
            "page": { "type": "integer", "minimum": 1 },
            "pages": {
                "type": "array",
                "items": { "type": "integer", "minimum": 1 }
            },
            "output": { "type": "string", "enum": ["inline", "resource"] }
        },
        "oneOf": [
            { "required": ["path"] },
            { "required": ["base64"] }
        ],
        "additionalProperties": false
    })
}

pub fn convert_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "base64": { "type": "string" },
            "format": { "type": "string", "enum": ["auto", "hwp", "hwpx"] },
            "to": { "type": "string", "enum": ["hwp", "hwpx"] },
            "output_path": { "type": "string" }
        },
        "required": ["to"],
        "oneOf": [
            { "required": ["path"] },
            { "required": ["base64"] }
        ],
        "additionalProperties": false
    })
}

pub fn create_document_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "text": { "type": "string" },
            "output_path": { "type": "string" }
        },
        "required": ["text"],
        "additionalProperties": false
    })
}

pub fn create_rich_document_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "to": { "type": "string", "enum": ["hwp", "hwpx"], "default": "hwp" },
            "output_path": { "type": "string" },
            "document": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "author": { "type": "string" },
                    "header": { "type": "string" },
                    "footer": { "type": "string" },
                    "blocks": {
                        "type": "array",
                        "items": {
                            "oneOf": [
                                {
                                    "type": "object",
                                    "properties": {
                                        "type": { "const": "paragraph" },
                                        "text": { "type": "string" },
                                        "style": text_style_schema()
                                    },
                                    "required": ["type", "text"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "type": { "const": "heading" },
                                        "level": { "type": "integer", "minimum": 1, "maximum": 6 },
                                        "text": { "type": "string" }
                                    },
                                    "required": ["type", "level", "text"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "type": { "const": "table" },
                                        "rows": {
                                            "type": "array",
                                            "items": {
                                                "type": "array",
                                                "items": table_cell_schema()
                                            }
                                        },
                                        "header_row": { "type": "boolean" },
                                        "column_widths": {
                                            "type": "array",
                                            "items": { "type": "integer", "minimum": 0 }
                                        },
                                        "border_style": {
                                            "type": "string",
                                            "enum": ["none", "basic", "full"]
                                        }
                                    },
                                    "required": ["type", "rows"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "type": { "const": "image" },
                                        "path": { "type": "string" },
                                        "data_base64": { "type": "string" },
                                        "mimeType": {
                                            "type": "string",
                                            "enum": ["image/png", "image/jpeg", "image/gif", "image/bmp"]
                                        },
                                        "width_mm": { "type": "integer", "minimum": 1 },
                                        "height_mm": { "type": "integer", "minimum": 1 },
                                        "caption": { "type": "string" },
                                        "align": {
                                            "type": "string",
                                            "enum": ["left", "center", "right", "inline"]
                                        },
                                        "wrap_text": { "type": "boolean" }
                                    },
                                    "required": ["type"],
                                    "oneOf": [
                                        { "required": ["path"] },
                                        { "required": ["data_base64", "mimeType"] }
                                    ],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "type": { "const": "list" },
                                        "items": {
                                            "type": "array",
                                            "items": { "type": "string" }
                                        },
                                        "ordered": { "type": "boolean" },
                                        "list_type": {
                                            "type": "string",
                                            "enum": ["bullet", "numbered", "alphabetic", "roman", "korean"]
                                        }
                                    },
                                    "required": ["type", "items"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "type": { "const": "page_break" }
                                    },
                                    "required": ["type"],
                                    "additionalProperties": false
                                }
                            ]
                        }
                    }
                },
                "required": ["blocks"],
                "additionalProperties": false
            }
        },
        "required": ["document"],
        "additionalProperties": false
    })
}

fn text_style_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "font_name": { "type": "string" },
            "font_size": { "type": "integer", "minimum": 1 },
            "bold": { "type": "boolean" },
            "italic": { "type": "boolean" },
            "underline": { "type": "boolean" },
            "color": { "type": "string", "description": "0xRRGGBB or #RRGGBB" }
        },
        "additionalProperties": false
    })
}

fn table_cell_schema() -> serde_json::Value {
    json!({
        "oneOf": [
            { "type": "string" },
            {
                "type": "object",
                "properties": {
                    "content": { "type": "string" },
                    "row_span": { "type": "integer", "minimum": 1 },
                    "col_span": { "type": "integer", "minimum": 1 },
                    "background_color": { "type": "string", "description": "0xRRGGBB or #RRGGBB" },
                    "text_align": {
                        "type": "string",
                        "enum": ["left", "center", "right"]
                    },
                    "style": text_style_schema()
                },
                "additionalProperties": false
            }
        ]
    })
}

pub fn extract_rich_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "base64": { "type": "string" },
            "format": { "type": "string", "enum": ["auto", "hwp", "hwpx"] },
            "images": { "type": "string", "enum": ["none", "metadata", "inline", "resource"], "default": "metadata" },
            "max_image_bytes": { "type": "integer", "minimum": 0 },
            "output_path": { "type": "string" }
        },
        "oneOf": [
            { "required": ["path"] },
            { "required": ["base64"] }
        ],
        "additionalProperties": false
    })
}
