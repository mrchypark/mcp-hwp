use hwpers::HwpWriter;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn cli_inspect_metadata_outputs_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("sample.hwp");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("Hello")?;
    writer.save_to_file(&file_path)?;

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args([
            "inspect-metadata",
            "--path",
            file_path.to_string_lossy().as_ref(),
            "--json",
        ])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert!(value.get("format").is_some());
    assert!(value.get("sections").is_some());
    assert!(value.get("paragraphs").is_some());
    Ok(())
}
