use hwpers::HwpWriter;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::tempdir;

#[test]
fn render_svg_inline_and_resource() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("sample.hwp");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("Hello")?;
    writer.set_a4_portrait()?;
    writer.add_header("Header");
    writer.save_to_file(&file_path)?;

    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let inline_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "tools/call",
        "params": {
            "name": "hwp.render_svg",
            "arguments": {
                "path": file_path.to_string_lossy(),
                "page": 1,
                "output": "inline"
            }
        }
    });
    let inline_serialized = serde_json::to_string(&inline_request)?;
    writeln!(stdin, "{inline_serialized}")?;
    stdin.flush()?;

    let mut inline_line = String::new();
    stdout.read_line(&mut inline_line)?;
    let inline_response: serde_json::Value = serde_json::from_str(inline_line.trim())?;
    let inline_result = inline_response.get("result").expect("result present");
    assert_eq!(
        inline_result.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );
    let svg = inline_result
        .get("structuredContent")
        .and_then(|value| value.get("pages"))
        .and_then(|value| value.as_array())
        .and_then(|value| value.first())
        .and_then(|value| value.get("svg"))
        .and_then(|value| value.as_str())
        .expect("svg present");
    assert!(svg.starts_with("<svg"));

    let resource_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "tools/call",
        "params": {
            "name": "hwp.render_svg",
            "arguments": {
                "path": file_path.to_string_lossy(),
                "page": 1,
                "output": "resource"
            }
        }
    });
    let resource_serialized = serde_json::to_string(&resource_request)?;
    writeln!(stdin, "{resource_serialized}")?;
    stdin.flush()?;

    let mut resource_line = String::new();
    stdout.read_line(&mut resource_line)?;
    let resource_response: serde_json::Value = serde_json::from_str(resource_line.trim())?;
    let resource_result = resource_response.get("result").expect("result present");
    assert_eq!(
        resource_result.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );
    let page_entry = resource_result
        .get("structuredContent")
        .and_then(|value| value.get("pages"))
        .and_then(|value| value.as_array())
        .and_then(|value| value.first())
        .expect("page entry present");
    let uri = page_entry
        .get("uri")
        .and_then(|value| value.as_str())
        .expect("uri present");
    assert!(uri.starts_with("file:"));
    let path = page_entry
        .get("path")
        .and_then(|value| value.as_str())
        .expect("path present");
    assert!(fs::metadata(path).is_ok());

    let _ = fs::remove_file(path);
    let _ = child.kill();
    Ok(())
}
