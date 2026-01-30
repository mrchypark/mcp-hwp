use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn send_request(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    request: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let serialized = serde_json::to_string(&request)?;
    writeln!(stdin, "{serialized}")?;
    stdin.flush()?;

    let mut line = String::new();
    stdout.read_line(&mut line)?;
    let response: serde_json::Value = serde_json::from_str(line.trim())?;
    Ok(response)
}

#[test]
fn create_and_extract_rich_hwp() -> Result<(), Box<dyn std::error::Error>> {
    // 1x1 PNG
    let png_base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO6qVt0AAAAASUVORK5CYII=";

    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let create_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 80,
        "method": "tools/call",
        "params": {
            "name": "hwp.create_rich_document",
            "arguments": {
                "to": "hwp",
                "document": {
                    "title": "Rich Doc",
                    "blocks": [
                        {"type": "paragraph", "text": "Hello"},
                        {"type": "table", "header_row": true, "rows": [["A", "B"], ["1", "2"]]},
                        {"type": "image", "mimeType": "image/png", "data_base64": png_base64, "width_mm": 10, "height_mm": 10, "caption": "tiny"}
                    ]
                }
            }
        }
    });
    let create_response = send_request(&mut stdin, &mut stdout, create_request)?;
    let create_result = create_response.get("result").expect("result present");
    assert_eq!(
        create_result.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );
    let base64 = create_result
        .get("structuredContent")
        .and_then(|v| v.get("base64"))
        .and_then(|v| v.as_str())
        .expect("base64 present");

    let extract_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 81,
        "method": "tools/call",
        "params": {
            "name": "hwp.extract_rich",
            "arguments": {
                "base64": base64,
                "format": "hwp",
                "images": "metadata"
            }
        }
    });
    let extract_response = send_request(&mut stdin, &mut stdout, extract_request)?;
    let extract_result = extract_response.get("result").expect("result present");
    assert_eq!(
        extract_result.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );

    let blocks = extract_result
        .get("structuredContent")
        .and_then(|v| v.get("blocks"))
        .and_then(|v| v.as_array())
        .expect("blocks array");

    let mut saw_table = false;
    let mut saw_image = false;
    for b in blocks {
        let ty = b.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if ty == "table" {
            let rows = b.get("rows").and_then(|v| v.as_array()).expect("rows");
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0][0].as_str(), Some("A"));
            assert_eq!(rows[0][1].as_str(), Some("B"));
            assert_eq!(rows[1][0].as_str(), Some("1"));
            assert_eq!(rows[1][1].as_str(), Some("2"));
            saw_table = true;
        }
        if ty == "image" {
            saw_image = true;
        }
    }

    assert!(saw_table);
    assert!(saw_image);

    let _ = child.kill();
    Ok(())
}
