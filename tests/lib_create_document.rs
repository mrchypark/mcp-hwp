use hwpers::HwpReader;
use std::fs;
use tempfile::tempdir;

#[test]
fn create_document_run_returns_binary_artifact() -> Result<(), Box<dyn std::error::Error>> {
    let response =
        mcp_hwp::tools::create_document::run(mcp_hwp::tools::create_document::Request {
            text: "Hello\n안녕".to_string(),
            output_path: None,
        })?;

    let document = HwpReader::from_bytes(&response.output.bytes)?;
    let text = document.extract_text();

    assert!(text.contains("Hello"));
    assert!(text.contains("안녕"));
    assert!(response.output.bytes_len > 0);
    Ok(())
}

#[test]
fn create_document_run_allows_large_output_when_writing_to_file()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let output_path = dir.path().join("large.hwp");
    let text = "A".repeat(mcp_hwp::constants::MAX_OUTPUT_BYTES as usize + 2_000_000);

    let response =
        mcp_hwp::tools::create_document::run(mcp_hwp::tools::create_document::Request {
            text,
            output_path: Some(output_path.to_string_lossy().to_string()),
        })?;

    let file = response.file.expect("file output");
    assert!(file.bytes_len > mcp_hwp::constants::MAX_OUTPUT_BYTES);
    assert!(fs::metadata(output_path).is_ok());
    Ok(())
}
