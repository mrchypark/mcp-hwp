#![allow(dead_code)]

use serde_json::json;

pub const TOOL_EXTRACT_TEXT: &str = "hwp.extract_text";
pub const TOOL_INSPECT_METADATA: &str = "hwp.inspect_metadata";
pub const TOOL_SUMMARIZE_STRUCTURE: &str = "hwp.summarize_structure";
pub const TOOL_RENDER_SVG: &str = "hwp.render_svg";
pub const TOOL_CONVERT: &str = "hwp.convert";
pub const TOOL_CREATE_DOCUMENT: &str = "hwp.create_document";

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
