use hwpers::HwpWriter;
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
fn convert_round_trip_hwp_hwpx() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("sample.hwp");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("Hello 안녕")?;
    writer.save_to_file(&file_path)?;

    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let convert_hwpx_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 20,
        "method": "tools/call",
        "params": {
            "name": "hwp.convert",
            "arguments": {
                "path": file_path.to_string_lossy(),
                "to": "hwpx"
            }
        }
    });
    let convert_hwpx_response = send_request(&mut stdin, &mut stdout, convert_hwpx_request)?;
    let convert_hwpx_result = convert_hwpx_response.get("result").expect("result present");
    assert_eq!(
        convert_hwpx_result.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );
    let structured = convert_hwpx_result
        .get("structuredContent")
        .and_then(|value| value.as_object())
        .expect("structured content present");
    let hwpx_base64 = structured
        .get("base64")
        .and_then(|value| value.as_str())
        .expect("base64 present")
        .to_string();
    let bytes_len = structured
        .get("bytes_len")
        .and_then(|value| value.as_u64())
        .expect("bytes_len present");
    assert!(bytes_len > 0);

    let extract_hwpx_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 21,
        "method": "tools/call",
        "params": {
            "name": "hwp.extract_text",
            "arguments": {
                "base64": hwpx_base64,
                "format": "hwpx"
            }
        }
    });
    let extract_hwpx_response = send_request(&mut stdin, &mut stdout, extract_hwpx_request)?;
    let extract_hwpx_result = extract_hwpx_response.get("result").expect("result present");
    assert_eq!(
        extract_hwpx_result.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );
    let hwpx_text = extract_hwpx_result
        .get("structuredContent")
        .and_then(|value| value.get("text"))
        .and_then(|value| value.as_str())
        .expect("text present");
    assert!(hwpx_text.contains("Hello"));
    assert!(hwpx_text.contains("안녕"));

    let convert_hwp_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 22,
        "method": "tools/call",
        "params": {
            "name": "hwp.convert",
            "arguments": {
                "base64": structured
                    .get("base64")
                    .and_then(|value| value.as_str())
                    .expect("base64 present"),
                "format": "hwpx",
                "to": "hwp"
            }
        }
    });
    let convert_hwp_response = send_request(&mut stdin, &mut stdout, convert_hwp_request)?;
    let convert_hwp_result = convert_hwp_response.get("result").expect("result present");
    assert_eq!(
        convert_hwp_result.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );
    let hwp_base64 = convert_hwp_result
        .get("structuredContent")
        .and_then(|value| value.get("base64"))
        .and_then(|value| value.as_str())
        .expect("base64 present");

    let extract_hwp_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 23,
        "method": "tools/call",
        "params": {
            "name": "hwp.extract_text",
            "arguments": {
                "base64": hwp_base64,
                "format": "hwp"
            }
        }
    });
    let extract_hwp_response = send_request(&mut stdin, &mut stdout, extract_hwp_request)?;
    let extract_hwp_result = extract_hwp_response.get("result").expect("result present");
    assert_eq!(
        extract_hwp_result.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );
    let hwp_text = extract_hwp_result
        .get("structuredContent")
        .and_then(|value| value.get("text"))
        .and_then(|value| value.as_str())
        .expect("text present");
    assert!(hwp_text.contains("Hello"));
    assert!(hwp_text.contains("안녕"));

    let _ = child.kill();
    Ok(())
}
