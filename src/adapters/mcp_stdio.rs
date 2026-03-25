use super::mcp_schema;
use anyhow::{Context, Result};
use mcp_hwp::tools;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

pub fn run_stdio_server() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let reader = stdin.lock().lines();
    let mut writer = io::BufWriter::new(stdout.lock());

    for line in reader {
        let line = line.context("failed to read stdin")?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let method = request.get("method").and_then(|value| value.as_str());
        let id = request.get("id").cloned();
        let response = match (method, id) {
            (Some("initialize"), Some(id)) => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": env!("CARGO_PKG_NAME"),
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            })),
            (Some("tools/list"), Some(id)) => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": mcp_schema::tool_definitions()
                }
            })),
            (Some("tools/call"), Some(id)) => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": handle_tool_call(&request)
            })),
            _ => None,
        };

        if let Some(response) = response {
            let serialized =
                serde_json::to_string(&response).context("failed to serialize response")?;
            writeln!(writer, "{serialized}").context("failed to write response")?;
            writer.flush().context("failed to flush response")?;
        }
    }

    Ok(())
}

fn handle_tool_call(request: &Value) -> Value {
    let params = request.get("params");
    let Some(params) = params.and_then(|value| value.as_object()) else {
        return tools::error_result("invalid_input", "params must be an object", None);
    };

    let name = params.get("name").and_then(|value| value.as_str());
    let Some(name) = name else {
        return tools::error_result("invalid_input", "params.name must be a string", None);
    };

    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match name {
        mcp_schema::TOOL_EXTRACT_TEXT => tools::extract_text::call(&args),
        mcp_schema::TOOL_INSPECT_METADATA => tools::inspect_metadata::call(&args),
        mcp_schema::TOOL_SUMMARIZE_STRUCTURE => tools::summarize_structure::call(&args),
        mcp_schema::TOOL_RENDER_SVG => tools::render_svg::call(&args),
        mcp_schema::TOOL_CONVERT => tools::convert::call(&args),
        mcp_schema::TOOL_CREATE_DOCUMENT => tools::create_document::call(&args),
        mcp_schema::TOOL_CREATE_RICH_DOCUMENT => tools::create_rich_document::call(&args),
        mcp_schema::TOOL_EXTRACT_RICH => tools::extract_rich::call(&args),
        _ => tools::error_result(
            "invalid_input",
            format!("tool not implemented: {name}"),
            Some(name),
        ),
    }
}
