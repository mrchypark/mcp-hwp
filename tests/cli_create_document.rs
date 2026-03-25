use hwpers::HwpReader;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn cli_create_document_writes_output() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let output_path = dir.path().join("created.hwp");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args([
            "create-document",
            "--text",
            "Hello\n안녕",
            "--output",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");

    let bytes = fs::read(&output_path)?;
    let document = HwpReader::from_bytes(&bytes)?;
    let text = document.extract_text();
    assert!(text.contains("Hello"));
    assert!(text.contains("안녕"));
    Ok(())
}
