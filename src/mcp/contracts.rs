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

pub const MAX_INPUT_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_SVG_OUTPUT_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_PARSE_MS: u64 = 10_000;

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
                    "page": {
                        "type": "object",
                        "properties": {
                            "size": { "type": "string", "enum": ["a4"] },
                            "orientation": { "type": "string", "enum": ["portrait", "landscape"] }
                        },
                        "additionalProperties": false
                    },
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
                                        "style": {
                                            "type": "object",
                                            "properties": {
                                                "font_name": { "type": "string" },
                                                "font_size": { "type": "integer", "minimum": 1 },
                                                "bold": { "type": "boolean" },
                                                "italic": { "type": "boolean" },
                                                "underline": { "type": "boolean" },
                                                "color": { "type": "string", "description": "0xRRGGBB (hex), e.g. 0xFF0000" }
                                            },
                                            "additionalProperties": false
                                        }
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
                                                "items": { "type": "string" }
                                            }
                                        },
                                        "header_row": { "type": "boolean" }
                                    },
                                    "required": ["type", "rows"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "type": { "const": "image" },
                                        "data_base64": { "type": "string" },
                                        "mimeType": { "type": "string", "enum": ["image/png", "image/jpeg", "image/gif", "image/bmp"] },
                                        "width_mm": { "type": "integer", "minimum": 1 },
                                        "height_mm": { "type": "integer", "minimum": 1 },
                                        "caption": { "type": "string" }
                                    },
                                    "required": ["type", "data_base64", "mimeType"],
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

pub fn extract_rich_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "base64": { "type": "string" },
            "format": { "type": "string", "enum": ["auto", "hwp", "hwpx"] },
            "images": { "type": "string", "enum": ["none", "metadata", "inline", "resource"], "default": "metadata" },
            "max_image_bytes": { "type": "integer", "minimum": 0 }
        },
        "oneOf": [
            { "required": ["path"] },
            { "required": ["base64"] }
        ],
        "additionalProperties": false
    })
}
