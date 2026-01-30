use std::fs;
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
fn create_document_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let output_path = dir.path().join("created.hwp");

    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let create_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 30,
        "method": "tools/call",
        "params": {
            "name": "hwp.create_document",
            "arguments": {
                "text": "Hello\n안녕"
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
    let base64 = structured
        .get("base64")
        .and_then(|value| value.as_str())
        .expect("base64 present");
    let bytes_len = structured
        .get("bytes_len")
        .and_then(|value| value.as_u64())
        .expect("bytes_len present");
    assert!(bytes_len > 0);

    let extract_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 31,
        "method": "tools/call",
        "params": {
            "name": "hwp.extract_text",
            "arguments": {
                "base64": base64,
                "format": "hwp"
            }
        }
    });
    let extract_response = send_request(&mut stdin, &mut stdout, extract_request)?;
    let extract_result = extract_response.get("result").expect("result present");
    assert_eq!(
        extract_result.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );
    let text = extract_result
        .get("structuredContent")
        .and_then(|value| value.get("text"))
        .and_then(|value| value.as_str())
        .expect("text present");
    assert!(text.contains("Hello"));
    assert!(text.contains("안녕"));

    let output_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 32,
        "method": "tools/call",
        "params": {
            "name": "hwp.create_document",
            "arguments": {
                "text": "Hello\n안녕",
                "output_path": output_path.to_string_lossy()
            }
        }
    });
    let output_response = send_request(&mut stdin, &mut stdout, output_request)?;
    let output_result = output_response.get("result").expect("result present");
    assert_eq!(
        output_result.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );
    let output_structured = output_result
        .get("structuredContent")
        .and_then(|value| value.as_object())
        .expect("structured content present");
    let output_bytes_len = output_structured
        .get("bytes_len")
        .and_then(|value| value.as_u64())
        .expect("bytes_len present");
    assert!(output_bytes_len > 0);
    fs::metadata(&output_path)?;

    let _ = child.kill();
    Ok(())
}
