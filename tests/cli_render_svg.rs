use hwpers::HwpWriter;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn cli_render_svg_writes_svg_files() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let input_path = dir.path().join("sample.hwp");
    let output_dir = dir.path().join("svg-output");

    let mut writer = HwpWriter::new();
    writer.add_paragraph("Hello svg")?;
    writer.set_a4_portrait()?;
    writer.save_to_file(&input_path)?;

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-hwp"))
        .args([
            "render-svg",
            "--path",
            input_path.to_string_lossy().as_ref(),
            "--page",
            "1",
            "--output-dir",
            output_dir.to_string_lossy().as_ref(),
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");

    let svg_path = output_dir.join("page-1.svg");
    let svg = fs::read_to_string(svg_path)?;
    assert!(svg.starts_with("<svg"));
    Ok(())
}
