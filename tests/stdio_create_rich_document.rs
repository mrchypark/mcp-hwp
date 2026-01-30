use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::tempdir;

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
fn create_rich_document_with_list_and_page_break() -> Result<(), Box<dyn std::error::Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let create_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 40,
        "method": "tools/call",
        "params": {
            "name": "hwp.create_rich_document",
            "arguments": {
                "to": "hwp",
                "document": {
                    "title": "Rich Document Test",
                    "blocks": [
                        { "type": "heading", "level": 1, "text": "Introduction" },
                        { "type": "paragraph", "text": "This is a test document with rich features." },
                        { "type": "list", "items": ["First item", "Second item", "Third item"], "ordered": false },
                        { "type": "page_break" },
                        { "type": "heading", "level": 2, "text": "Numbered List" },
                        { "type": "list", "items": ["Step 1", "Step 2", "Step 3"], "ordered": true }
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

    let structured = create_result
        .get("structuredContent")
        .and_then(|value| value.as_object())
        .expect("structured content present");
    let bytes_len = structured
        .get("bytes_len")
        .and_then(|value| value.as_u64())
        .expect("bytes_len present");
    assert!(bytes_len > 0);

    let _ = child.kill();
    Ok(())
}

#[test]
fn create_rich_document_with_styled_paragraph() -> Result<(), Box<dyn std::error::Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let create_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 41,
        "method": "tools/call",
        "params": {
            "name": "hwp.create_rich_document",
            "arguments": {
                "to": "hwp",
                "document": {
                    "title": "Styled Paragraph Test",
                    "blocks": [
                        {
                            "type": "paragraph",
                            "text": "Bold and red text",
                            "style": {
                                "bold": true,
                                "color": "0xFF0000"
                            }
                        },
                        {
                            "type": "paragraph",
                            "text": "Italic text",
                            "style": {
                                "italic": true
                            }
                        }
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

    let _ = child.kill();
    Ok(())
}

#[test]
fn create_rich_document_with_table() -> Result<(), Box<dyn std::error::Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let create_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/call",
        "params": {
            "name": "hwp.create_rich_document",
            "arguments": {
                "to": "hwp",
                "document": {
                    "title": "Table Test",
                    "blocks": [
                        { "type": "heading", "level": 1, "text": "Data Table" },
                        {
                            "type": "table",
                            "rows": [
                                ["Name", "Age", "City"],
                                ["Alice", "30", "Seoul"],
                                ["Bob", "25", "Busan"]
                            ],
                            "header_row": true
                        }
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

    let _ = child.kill();
    Ok(())
}
