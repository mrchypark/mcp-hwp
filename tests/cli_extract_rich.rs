use hwpers::HwpWriter;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn cli_extract_rich_outputs_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let input_path = dir.path().join("sample.hwp");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("Hello rich")?;
    writer.save_to_file(&input_path)?;

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args([
            "extract-rich",
            "--path",
            input_path.to_string_lossy().as_ref(),
            "--json",
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value.get("format").and_then(|v| v.as_str()), Some("hwp"));
    assert!(value.get("blocks").and_then(|v| v.as_array()).is_some());
    Ok(())
}

#[test]
fn cli_extract_rich_help_lists_image_modes_and_output_dir() -> Result<(), Box<dyn std::error::Error>>
{
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args(["extract-rich", "--help"])
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("--images <IMAGES>"));
    assert!(stdout.contains("--output-dir <OUTPUT_DIR>"));
    assert!(stdout.contains("none"));
    assert!(stdout.contains("metadata"));
    assert!(stdout.contains("inline"));
    assert!(stdout.contains("resource"));
    Ok(())
}

#[test]
fn cli_extract_rich_rejects_invalid_image_mode() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let input_path = dir.path().join("sample.hwp");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("Hello rich")?;
    writer.save_to_file(&input_path)?;

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args([
            "extract-rich",
            "--path",
            input_path.to_string_lossy().as_ref(),
            "--images",
            "bogus",
        ])
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("possible values"));
    Ok(())
}
