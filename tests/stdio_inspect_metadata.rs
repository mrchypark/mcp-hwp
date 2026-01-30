use hwpers::HwpWriter;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::tempdir;

#[test]
fn inspect_metadata_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("sample.hwp");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("Hello")?;
    writer.save_to_file(&file_path)?;

    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "hwp.inspect_metadata",
            "arguments": {
                "path": file_path.to_string_lossy()
            }
        }
    });
    let serialized = serde_json::to_string(&request)?;
    writeln!(stdin, "{serialized}")?;
    stdin.flush()?;

    let mut line = String::new();
    stdout.read_line(&mut line)?;

    let response: serde_json::Value = serde_json::from_str(line.trim())?;
    let result = response.get("result").expect("result present");
    assert_eq!(result.get("isError").and_then(|v| v.as_bool()), Some(false));

    let structured = result
        .get("structuredContent")
        .and_then(|value| value.as_object())
        .expect("structured content present");

    let format = structured
        .get("format")
        .and_then(|value| value.as_str())
        .expect("format present");
    assert_eq!(format, "hwp");

    let sections = structured
        .get("sections")
        .and_then(|value| value.as_u64())
        .expect("sections present");
    let paragraphs = structured
        .get("paragraphs")
        .and_then(|value| value.as_u64())
        .expect("paragraphs present");

    assert!(sections >= 1);
    assert!(paragraphs >= 1);

    let _ = child.kill();
    Ok(())
}
