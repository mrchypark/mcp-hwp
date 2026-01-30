use serde_json::json;

pub mod convert;
pub mod create_document;
pub mod create_rich_document;
pub mod extract_rich;
pub mod extract_text;
pub mod inspect_metadata;
pub mod render_svg;
pub mod summarize_structure;

pub fn error_result(
    kind: &'static str,
    message: impl Into<String>,
    source: Option<&str>,
) -> serde_json::Value {
    let message = message.into();
    let mut error = json!({
        "kind": kind,
        "message": message,
    });

    if let Some(source) = source
        && let Some(obj) = error.as_object_mut()
    {
        obj.insert("source".to_string(), json!(source));
    }

    json!({
        "content": [{"type": "text", "text": format!("Error: {message}")}],
        "structuredContent": {"error": error},
        "isError": true
    })
}
