use hwpers::HwpWriter;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn cli_extract_text_outputs_text() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("sample.hwp");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("Hello CLI")?;
    writer.save_to_file(&file_path)?;

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args([
            "extract-text",
            "--path",
            file_path.to_string_lossy().as_ref(),
        ])
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("Hello CLI"));
    Ok(())
}
