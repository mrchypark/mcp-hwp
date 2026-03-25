use hwpers::HwpReader;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn cli_create_rich_document_reads_json_file_and_writes_output()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let input_path = dir.path().join("doc.json");
    let output_path = dir.path().join("rich.hwp");

    fs::write(
        &input_path,
        serde_json::json!({
            "title": "Rich",
            "blocks": [
                {"type": "heading", "level": 1, "text": "Title"},
                {"type": "paragraph", "text": "Hello rich"}
            ]
        })
        .to_string(),
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args([
            "create-rich-document",
            "--input",
            input_path.to_string_lossy().as_ref(),
            "--output",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");

    let bytes = fs::read(&output_path)?;
    let document = HwpReader::from_bytes(&bytes)?;
    let text = document.extract_text();
    assert!(text.contains("Title"));
    assert!(text.contains("Hello rich"));
    Ok(())
}
