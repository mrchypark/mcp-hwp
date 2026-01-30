use serde_json::json;

pub mod contracts;
pub mod errors;

pub fn tool_definitions() -> Vec<serde_json::Value> {
    vec![
        json!({
            "name": contracts::TOOL_EXTRACT_TEXT,
            "description": "Extract plain text from HWP documents.",
            "inputSchema": contracts::extract_text_schema()
        }),
        json!({
            "name": contracts::TOOL_INSPECT_METADATA,
            "description": "Inspect metadata from HWP documents.",
            "inputSchema": contracts::inspect_metadata_schema()
        }),
        json!({
            "name": contracts::TOOL_SUMMARIZE_STRUCTURE,
            "description": "Summarize document structure for HWP documents.",
            "inputSchema": contracts::summarize_structure_schema()
        }),
        json!({
            "name": contracts::TOOL_RENDER_SVG,
            "description": "Render HWP pages or elements into SVG.",
            "inputSchema": contracts::render_svg_schema()
        }),
        json!({
            "name": contracts::TOOL_CONVERT,
            "description": "Convert HWP documents between formats.",
            "inputSchema": contracts::convert_schema()
        }),
        json!({
            "name": contracts::TOOL_CREATE_DOCUMENT,
            "description": "Create new HWP documents from text.",
            "inputSchema": contracts::create_document_schema()
        }),
    ]
}
