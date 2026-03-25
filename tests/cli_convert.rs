use hwpers::HwpWriter;
use hwpers::HwpxReader;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn cli_convert_writes_requested_output() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let input_path = dir.path().join("sample.hwp");
    let output_path = dir.path().join("converted.hwpx");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("Hello convert")?;
    writer.save_to_file(&input_path)?;

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args([
            "convert",
            "--path",
            input_path.to_string_lossy().as_ref(),
            "--to",
            "hwpx",
            "--output",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    assert!(fs::metadata(&output_path).is_ok());

    let bytes = fs::read(&output_path)?;
    let document = HwpxReader::from_bytes(&bytes)?;
    let text = document.extract_text();
    assert!(text.contains("Hello convert"));
    Ok(())
}
