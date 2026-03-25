#[path = "../src/adapters/mcp_schema.rs"]
mod mcp_schema;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

fn branch_required_strings(schema: &Value) -> Vec<Vec<String>> {
    schema
        .get("oneOf")
        .and_then(|value| value.as_array())
        .expect("oneOf array")
        .iter()
        .map(|branch| {
            branch
                .get("required")
                .and_then(|value| value.as_array())
                .expect("required array")
                .iter()
                .map(|value| value.as_str().expect("required string").to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn extract_text_schema_matches_parser_contract() {
    let schema = mcp_schema::extract_text_schema();
    assert_eq!(
        branch_required_strings(&schema),
        vec![vec!["path".to_string()], vec!["base64".to_string()]]
    );

    let invalid = serde_json::json!({});
    let error = mcp_hwp::tools::extract_text::request_from_value(&invalid).unwrap_err();
    assert_eq!(error.kind, mcp_hwp::errors::INVALID_INPUT);

    let valid = serde_json::json!({
        "base64": STANDARD.encode([0_u8]),
        "format": "hwp"
    });
    assert!(mcp_hwp::tools::extract_text::request_from_value(&valid).is_ok());
}

#[test]
fn convert_schema_matches_parser_contract() {
    let schema = mcp_schema::convert_schema();
    let required = schema
        .get("required")
        .and_then(|value| value.as_array())
        .expect("required array")
        .iter()
        .map(|value| value.as_str().expect("required string"))
        .collect::<Vec<_>>();
    assert!(required.contains(&"to"));

    let branches = branch_required_strings(&schema);
    assert!(branches.contains(&vec!["path".to_string()]));
    assert!(branches.contains(&vec!["base64".to_string()]));

    let invalid = serde_json::json!({
        "base64": STANDARD.encode([0_u8]),
        "format": "hwp"
    });
    let error = mcp_hwp::tools::convert::request_from_value(&invalid).unwrap_err();
    assert_eq!(error.kind, mcp_hwp::errors::INVALID_INPUT);

    let valid = serde_json::json!({
        "base64": STANDARD.encode([0_u8]),
        "format": "hwp",
        "to": "hwpx"
    });
    assert!(mcp_hwp::tools::convert::request_from_value(&valid).is_ok());
}

#[test]
fn create_rich_document_schema_matches_parser_contract() {
    let schema = mcp_schema::create_rich_document_schema();
    let document = schema
        .get("properties")
        .and_then(|value| value.get("document"))
        .expect("document schema");
    let blocks = document
        .get("properties")
        .and_then(|value| value.get("blocks"))
        .expect("blocks schema");
    let variants = blocks
        .get("items")
        .and_then(|value| value.get("oneOf"))
        .and_then(|value| value.as_array())
        .expect("block variants");
    let block_types = variants
        .iter()
        .filter_map(|variant| {
            variant
                .get("properties")
                .and_then(|value| value.get("type"))
                .and_then(|value| value.get("const"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
        .collect::<Vec<_>>();

    for expected in [
        "paragraph",
        "heading",
        "table",
        "image",
        "list",
        "page_break",
    ] {
        assert!(
            block_types.iter().any(|value| value == expected),
            "missing block type {expected}"
        );
    }

    let table_variant = variants
        .iter()
        .find(|variant| {
            variant
                .get("properties")
                .and_then(|value| value.get("type"))
                .and_then(|value| value.get("const"))
                .and_then(|value| value.as_str())
                == Some("table")
        })
        .expect("table variant");
    let table_props = table_variant.get("properties").expect("table properties");
    assert!(table_props.get("column_widths").is_some());
    assert!(table_props.get("border_style").is_some());

    let image_variant = variants
        .iter()
        .find(|variant| {
            variant
                .get("properties")
                .and_then(|value| value.get("type"))
                .and_then(|value| value.get("const"))
                .and_then(|value| value.as_str())
                == Some("image")
        })
        .expect("image variant");
    let image_props = image_variant.get("properties").expect("image properties");
    assert!(image_props.get("path").is_some());
    assert!(image_props.get("align").is_some());
    assert!(image_props.get("wrap_text").is_some());

    let valid = serde_json::json!({
        "to": "hwp",
        "document": {
            "title": "Rich",
            "blocks": [
                { "type": "list", "items": ["one", "two"], "ordered": false },
                { "type": "page_break" },
                {
                    "type": "table",
                    "rows": [
                        [
                            { "content": "A", "row_span": 1, "style": { "bold": true } },
                            "B"
                        ]
                    ],
                    "column_widths": [1200, 1200],
                    "border_style": "full"
                },
                {
                    "type": "image",
                    "path": "/tmp/example.png",
                    "align": "center",
                    "wrap_text": true
                }
            ]
        }
    });
    assert!(mcp_hwp::tools::create_rich_document::request_from_value(&valid).is_ok());

    let invalid = serde_json::json!({
        "document": {
            "blocks": [
                { "type": "list" }
            ]
        }
    });
    let error = mcp_hwp::tools::create_rich_document::request_from_value(&invalid).unwrap_err();
    assert_eq!(error.kind, mcp_hwp::errors::INVALID_INPUT);
}

#[test]
fn optional_argument_type_mismatches_are_rejected() {
    let extract_text_args = serde_json::json!({
        "base64": STANDARD.encode([0_u8]),
        "format": "hwp",
        "include_newlines": "false"
    });
    let error = mcp_hwp::tools::extract_text::request_from_value(&extract_text_args).unwrap_err();
    assert_eq!(error.kind, mcp_hwp::errors::INVALID_INPUT);

    let summarize_args = serde_json::json!({
        "base64": STANDARD.encode([0_u8]),
        "format": "hwp",
        "preview_chars": "5"
    });
    let error =
        mcp_hwp::tools::summarize_structure::request_from_value(&summarize_args).unwrap_err();
    assert_eq!(error.kind, mcp_hwp::errors::INVALID_INPUT);
}
