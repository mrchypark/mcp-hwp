use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{Map, Value, json};
use std::io::{self, BufRead, Write};
use std::process;

mod input;
mod mcp;
mod tools;

#[derive(Parser)]
#[command(name = "hwpers-cli-mcp")]
#[command(
    version,
    about = "CLI utilities for HWP processing and MCP integration"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args, Clone)]
#[command(
    group(
        clap::ArgGroup::new("input")
            .required(true)
            .multiple(false)
            .args(["path", "base64"])
    )
)]
struct InputArgs {
    /// Path to the HWP/HWPX file
    #[arg(long)]
    path: Option<String>,
    /// Base64-encoded HWP/HWPX bytes
    #[arg(long)]
    base64: Option<String>,
    /// Input format override
    #[arg(long, value_enum)]
    format: Option<FormatArg>,
}

#[derive(Clone, Copy, ValueEnum)]
enum FormatArg {
    Auto,
    Hwp,
    Hwpx,
}

impl FormatArg {
    fn as_str(self) -> &'static str {
        match self {
            FormatArg::Auto => "auto",
            FormatArg::Hwp => "hwp",
            FormatArg::Hwpx => "hwpx",
        }
    }
}

#[derive(Args, Clone)]
struct ExtractTextArgs {
    #[command(flatten)]
    input: InputArgs,
    /// Output JSON structuredContent
    #[arg(long)]
    json: bool,
    /// Maximum characters to return
    #[arg(long)]
    max_chars: Option<u64>,
    /// Preserve newline characters (true/false)
    #[arg(long)]
    include_newlines: Option<bool>,
    /// Normalize whitespace (true/false)
    #[arg(long)]
    normalize_whitespace: Option<bool>,
}

#[derive(Args, Clone)]
struct InspectMetadataArgs {
    #[command(flatten)]
    input: InputArgs,
    /// Output JSON structuredContent
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct SummarizeStructureArgs {
    #[command(flatten)]
    input: InputArgs,
    /// Output JSON structuredContent
    #[arg(long)]
    json: bool,
    /// Maximum sections to return
    #[arg(long)]
    max_sections: Option<u64>,
    /// Maximum paragraphs per section
    #[arg(long)]
    max_paragraphs_per_section: Option<u64>,
    /// Preview character length
    #[arg(long)]
    preview_chars: Option<u64>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start MCP stdio server
    Serve {
        /// Serve MCP over stdio (NDJSON)
        #[arg(long)]
        stdio: bool,
    },
    /// Extract text from HWP inputs
    ExtractText(ExtractTextArgs),
    /// Inspect HWP metadata
    InspectMetadata(InspectMetadataArgs),
    /// Summarize document structure
    SummarizeStructure(SummarizeStructureArgs),
    /// Render SVG for pages or elements
    RenderSvg,
    /// Convert HWP to other formats
    Convert,
    /// Create new HWP documents
    Create,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { stdio } => {
            if stdio {
                run_stdio_server()
            } else {
                anyhow::bail!("only --stdio transport is supported")
            }
        }
        Commands::ExtractText(args) => run_extract_text(args),
        Commands::InspectMetadata(args) => run_inspect_metadata(args),
        Commands::SummarizeStructure(args) => run_summarize_structure(args),
        Commands::RenderSvg => stub("render-svg"),
        Commands::Convert => stub("convert"),
        Commands::Create => stub("create"),
    }
}

fn stub(command: &str) -> Result<()> {
    println!("{command} stub (not implemented yet)");
    Ok(())
}

fn run_extract_text(args: ExtractTextArgs) -> Result<()> {
    let mut map = build_input_args(&args.input);
    if let Some(max_chars) = args.max_chars {
        map.insert("max_chars".to_string(), json!(max_chars));
    }
    if let Some(include_newlines) = args.include_newlines {
        map.insert("include_newlines".to_string(), json!(include_newlines));
    }
    if let Some(normalize_whitespace) = args.normalize_whitespace {
        map.insert(
            "normalize_whitespace".to_string(),
            json!(normalize_whitespace),
        );
    }
    let result = tools::extract_text::call(&Value::Object(map));
    print_tool_result(result, args.json)
}

fn run_inspect_metadata(args: InspectMetadataArgs) -> Result<()> {
    let map = build_input_args(&args.input);
    let result = tools::inspect_metadata::call(&Value::Object(map));
    print_tool_result(result, args.json)
}

fn run_summarize_structure(args: SummarizeStructureArgs) -> Result<()> {
    let mut map = build_input_args(&args.input);
    if let Some(max_sections) = args.max_sections {
        map.insert("max_sections".to_string(), json!(max_sections));
    }
    if let Some(max_paragraphs_per_section) = args.max_paragraphs_per_section {
        map.insert(
            "max_paragraphs_per_section".to_string(),
            json!(max_paragraphs_per_section),
        );
    }
    if let Some(preview_chars) = args.preview_chars {
        map.insert("preview_chars".to_string(), json!(preview_chars));
    }
    let result = tools::summarize_structure::call(&Value::Object(map));
    print_tool_result(result, args.json)
}

fn build_input_args(input: &InputArgs) -> Map<String, Value> {
    let mut map = Map::new();
    if let Some(path) = &input.path {
        map.insert("path".to_string(), json!(path));
    }
    if let Some(base64) = &input.base64 {
        map.insert("base64".to_string(), json!(base64));
    }
    if let Some(format) = input.format {
        map.insert("format".to_string(), json!(format.as_str()));
    }
    map
}

fn print_tool_result(result: Value, json_output: bool) -> Result<()> {
    let is_error = result
        .get("isError")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    if is_error {
        let message = result
            .get("structuredContent")
            .and_then(|value| value.get("error"))
            .and_then(|value| value.get("message"))
            .and_then(|value| value.as_str())
            .unwrap_or("tool error");
        eprintln!("{message}");
        process::exit(1);
    }

    if json_output {
        let structured = result
            .get("structuredContent")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let output = serde_json::to_string_pretty(&structured)?;
        println!("{output}");
        return Ok(());
    }

    let text = result
        .get("content")
        .and_then(|value| value.as_array())
        .and_then(|arr| arr.first())
        .and_then(|value| value.get("text"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    println!("{text}");
    Ok(())
}

fn run_stdio_server() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let reader = stdin.lock().lines();
    let mut writer = io::BufWriter::new(stdout.lock());

    for line in reader {
        let line = line.context("failed to read stdin")?;
        if line.trim().is_empty() {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let method = request.get("method").and_then(|value| value.as_str());
        let id = request.get("id").cloned();
        let response = match (method, id) {
            (Some("initialize"), Some(id)) => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": env!("CARGO_PKG_NAME"),
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            })),
            (Some("tools/list"), Some(id)) => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": mcp::tool_definitions()
                }
            })),
            (Some("tools/call"), Some(id)) => {
                let result = handle_tool_call(&request);
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result
                }))
            }
            _ => None,
        };

        if let Some(response) = response {
            let serialized =
                serde_json::to_string(&response).context("failed to serialize response")?;
            writeln!(writer, "{serialized}").context("failed to write response")?;
            writer.flush().context("failed to flush response")?;
        }
    }

    Ok(())
}

fn handle_tool_call(request: &serde_json::Value) -> serde_json::Value {
    let params = request.get("params");
    let Some(params) = params.and_then(|value| value.as_object()) else {
        return tools::error_result(mcp::errors::INVALID_INPUT, "params must be an object", None);
    };

    let name = params.get("name").and_then(|value| value.as_str());
    let Some(name) = name else {
        return tools::error_result(
            mcp::errors::INVALID_INPUT,
            "params.name must be a string",
            None,
        );
    };

    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match name {
        mcp::contracts::TOOL_EXTRACT_TEXT => tools::extract_text::call(&args),
        mcp::contracts::TOOL_INSPECT_METADATA => tools::inspect_metadata::call(&args),
        mcp::contracts::TOOL_SUMMARIZE_STRUCTURE => tools::summarize_structure::call(&args),
        mcp::contracts::TOOL_RENDER_SVG => tools::render_svg::call(&args),
        mcp::contracts::TOOL_CONVERT => tools::convert::call(&args),
        mcp::contracts::TOOL_CREATE_DOCUMENT => tools::create_document::call(&args),
        _ => tools::error_result(
            mcp::errors::INVALID_INPUT,
            format!("tool not implemented: {name}"),
            Some(name),
        ),
    }
}
