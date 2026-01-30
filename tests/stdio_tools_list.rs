use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn tools_list_includes_expected_tools() -> Result<(), Box<dyn std::error::Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin available");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout available"));

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    let serialized = serde_json::to_string(&request)?;
    writeln!(stdin, "{serialized}")?;
    stdin.flush()?;

    let mut line = String::new();
    stdout.read_line(&mut line)?;

    let response: serde_json::Value = serde_json::from_str(line.trim())?;
    let tools = response
        .get("result")
        .and_then(|value| value.get("tools"))
        .and_then(|value| value.as_array())
        .expect("tools array present");

    let names: HashSet<&str> = tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(|value| value.as_str()))
        .collect();

    let expected: HashSet<&str> = [
        "hwp.extract_text",
        "hwp.inspect_metadata",
        "hwp.summarize_structure",
        "hwp.render_svg",
        "hwp.convert",
        "hwp.create_document",
    ]
    .into_iter()
    .collect();

    assert_eq!(names, expected);

    let _ = child.kill();
    Ok(())
}
