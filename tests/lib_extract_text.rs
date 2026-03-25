use hwpers::HwpWriter;

#[test]
fn extract_text_run_returns_typed_response() -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = HwpWriter::new();
    writer.add_paragraph("Hello")?;
    writer.add_paragraph("안녕")?;
    let bytes = writer.to_bytes()?;

    let response = mcp_hwp::tools::extract_text::run(mcp_hwp::tools::extract_text::Request {
        payload: mcp_hwp::input::InputPayload {
            bytes,
            format: mcp_hwp::input::InputFormat::Hwp,
            source: "test".to_string(),
        },
        include_newlines: true,
        normalize_whitespace: false,
        max_chars: None,
    })?;

    assert!(response.text.contains("Hello"));
    assert!(response.text.contains("안녕"));
    Ok(())
}
