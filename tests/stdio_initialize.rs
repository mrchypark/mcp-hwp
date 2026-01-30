use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn initialize_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });
    let serialized = serde_json::to_string(&request)?;
    writeln!(stdin, "{serialized}")?;
    stdin.flush()?;

    let mut line = String::new();
    stdout.read_line(&mut line)?;

    let response: serde_json::Value = serde_json::from_str(line.trim())?;
    assert_eq!(
        response.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0")
    );
    assert_eq!(response.get("id").and_then(|v| v.as_i64()), Some(1));

    let result = response.get("result").expect("result present");
    assert_eq!(
        result.get("protocolVersion").and_then(|v| v.as_str()),
        Some("2025-11-25")
    );
    assert!(
        result
            .get("capabilities")
            .and_then(|v| v.get("tools"))
            .is_some()
    );

    let server_info = result.get("serverInfo").expect("serverInfo present");
    assert_eq!(
        server_info.get("name").and_then(|v| v.as_str()),
        Some("mcp-hwp")
    );
    assert_eq!(
        server_info.get("version").and_then(|v| v.as_str()),
        Some(env!("CARGO_PKG_VERSION"))
    );

    let _ = child.kill();
    Ok(())
}
