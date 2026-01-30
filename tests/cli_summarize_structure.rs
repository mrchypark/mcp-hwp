use hwpers::HwpWriter;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn cli_summarize_structure_outputs_limited_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("sample.hwp");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("First paragraph text")?;
    writer.add_paragraph("Second paragraph text")?;
    writer.save_to_file(&file_path)?;

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args([
            "summarize-structure",
            "--path",
            file_path.to_string_lossy().as_ref(),
            "--json",
            "--max-paragraphs-per-section",
            "1",
            "--preview-chars",
            "5",
        ])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let sections = value
        .get("sections")
        .and_then(|v| v.as_array())
        .expect("sections array present");
    assert!(!sections.is_empty());

    let paragraphs = sections[0]
        .get("paragraphs")
        .and_then(|v| v.as_array())
        .expect("paragraphs present");
    assert_eq!(paragraphs.len(), 1);

    let preview = paragraphs[0]
        .get("preview")
        .and_then(|v| v.as_str())
        .expect("preview present");
    assert!(preview.len() <= 5);
    Ok(())
}
