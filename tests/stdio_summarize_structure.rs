use hwpers::HwpWriter;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::tempdir;

fn send_request(
    stdin: &mut impl Write,
    stdout: &mut impl BufRead,
    request: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let serialized = serde_json::to_string(&request)?;
    writeln!(stdin, "{serialized}")?;
    stdin.flush()?;

    let mut line = String::new();
    stdout.read_line(&mut line)?;
    Ok(serde_json::from_str(line.trim())?)
}

#[test]
fn summarize_structure_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("sample.hwp");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("First paragraph text")?;
    writer.add_paragraph("Second paragraph text")?;
    writer.save_to_file(&file_path)?;

    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let request_full = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "hwp.summarize_structure",
            "arguments": {
                "path": file_path.to_string_lossy()
            }
        }
    });
    let response_full = send_request(&mut stdin, &mut stdout, request_full)?;
    let result_full = response_full.get("result").expect("result present");
    assert_eq!(
        result_full.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );

    let sections_full = result_full
        .get("structuredContent")
        .and_then(|value| value.get("sections"))
        .and_then(|value| value.as_array())
        .expect("sections present");
    assert!(!sections_full.is_empty());

    let paragraphs_full = sections_full[0]
        .get("paragraphs")
        .and_then(|value| value.as_array())
        .expect("paragraphs present");
    assert!(paragraphs_full.len() >= 2);

    let request_limited = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "hwp.summarize_structure",
            "arguments": {
                "path": file_path.to_string_lossy(),
                "max_paragraphs_per_section": 1,
                "preview_chars": 5
            }
        }
    });
    let response_limited = send_request(&mut stdin, &mut stdout, request_limited)?;
    let result_limited = response_limited.get("result").expect("result present");
    assert_eq!(
        result_limited.get("isError").and_then(|v| v.as_bool()),
        Some(false)
    );

    let sections_limited = result_limited
        .get("structuredContent")
        .and_then(|value| value.get("sections"))
        .and_then(|value| value.as_array())
        .expect("sections present");
    assert!(!sections_limited.is_empty());

    let paragraphs_limited = sections_limited[0]
        .get("paragraphs")
        .and_then(|value| value.as_array())
        .expect("paragraphs present");
    assert_eq!(paragraphs_limited.len(), 1);

    let preview = paragraphs_limited[0]
        .get("preview")
        .and_then(|value| value.as_str())
        .expect("preview present");
    assert!(preview.contains("First"));
    assert!(preview.len() <= 5);

    let _ = child.kill();
    Ok(())
}
