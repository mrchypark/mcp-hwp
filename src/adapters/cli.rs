use clap::{Args, Parser, Subcommand, ValueEnum};
use mcp_hwp::errors::AppError;
use mcp_hwp::tools;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mcp-hwp")]
#[command(
    version,
    about = "CLI utilities for HWP/HWPX processing with optional MCP stdio serving"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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
    /// Render SVG for pages
    RenderSvg(RenderSvgArgs),
    /// Convert HWP/HWPX between formats
    Convert(ConvertArgs),
    /// Create a plain HWP document from text
    CreateDocument(CreateDocumentArgs),
    /// Create a rich HWP/HWPX document from a JSON file
    CreateRichDocument(CreateRichDocumentArgs),
    /// Extract a rich block representation from a document
    ExtractRich(ExtractRichArgs),
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
    #[arg(long)]
    path: Option<String>,
    #[arg(long)]
    base64: Option<String>,
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

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormatArg {
    Hwp,
    Hwpx,
}

impl OutputFormatArg {
    fn as_str(self) -> &'static str {
        match self {
            OutputFormatArg::Hwp => "hwp",
            OutputFormatArg::Hwpx => "hwpx",
        }
    }
}

#[derive(Args, Clone)]
pub struct ExtractTextArgs {
    #[command(flatten)]
    input: InputArgs,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    max_chars: Option<u64>,
    #[arg(long)]
    include_newlines: Option<bool>,
    #[arg(long)]
    normalize_whitespace: Option<bool>,
}

#[derive(Args, Clone)]
pub struct InspectMetadataArgs {
    #[command(flatten)]
    input: InputArgs,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
pub struct SummarizeStructureArgs {
    #[command(flatten)]
    input: InputArgs,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    max_sections: Option<u64>,
    #[arg(long)]
    max_paragraphs_per_section: Option<u64>,
    #[arg(long)]
    preview_chars: Option<u64>,
}

#[derive(Args, Clone)]
pub struct RenderSvgArgs {
    #[command(flatten)]
    input: InputArgs,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    page: Option<u64>,
    #[arg(long, value_delimiter = ',')]
    pages: Vec<u64>,
    #[arg(long)]
    output_dir: String,
}

#[derive(Args, Clone)]
pub struct ConvertArgs {
    #[command(flatten)]
    input: InputArgs,
    #[arg(long)]
    json: bool,
    #[arg(long, value_enum)]
    to: OutputFormatArg,
    #[arg(long)]
    output: String,
}

#[derive(Args, Clone)]
pub struct CreateDocumentArgs {
    #[arg(long)]
    json: bool,
    #[arg(long)]
    text: String,
    #[arg(long)]
    output: String,
}

#[derive(Args, Clone)]
pub struct CreateRichDocumentArgs {
    #[arg(long)]
    json: bool,
    #[arg(long)]
    input: String,
    #[arg(long, value_enum)]
    to: Option<OutputFormatArg>,
    #[arg(long)]
    output: String,
}

#[derive(Args, Clone)]
pub struct ExtractRichArgs {
    #[command(flatten)]
    input: InputArgs,
    #[arg(long)]
    json: bool,
    #[arg(long, value_enum, default_value = "metadata")]
    images: ExtractRichImageMode,
    #[arg(long)]
    max_image_bytes: Option<u64>,
    #[arg(long)]
    output_dir: Option<String>,
}

#[derive(Clone, Copy, ValueEnum)]
enum ExtractRichImageMode {
    None,
    Metadata,
    Inline,
    Resource,
}

impl ExtractRichImageMode {
    fn as_str(self) -> &'static str {
        match self {
            ExtractRichImageMode::None => "none",
            ExtractRichImageMode::Metadata => "metadata",
            ExtractRichImageMode::Inline => "inline",
            ExtractRichImageMode::Resource => "resource",
        }
    }
}

pub fn run(command: Commands) -> Result<(), AppError> {
    match command {
        Commands::Serve { .. } => Err(AppError::invalid_input(
            "serve --stdio is handled by the MCP adapter",
        )),
        Commands::ExtractText(args) => run_extract_text(args),
        Commands::InspectMetadata(args) => run_inspect_metadata(args),
        Commands::SummarizeStructure(args) => run_summarize_structure(args),
        Commands::RenderSvg(args) => run_render_svg(args),
        Commands::Convert(args) => run_convert(args),
        Commands::CreateDocument(args) => run_create_document(args),
        Commands::CreateRichDocument(args) => run_create_rich_document(args),
        Commands::ExtractRich(args) => run_extract_rich(args),
    }
}

fn run_extract_text(args: ExtractTextArgs) -> Result<(), AppError> {
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

    let req = tools::extract_text::request_from_value(&Value::Object(map))?;
    let response = tools::extract_text::run(req)?;

    if args.json {
        return print_json_value(&json!({ "text": response.text }));
    }

    println!("{}", response.text);
    Ok(())
}

fn run_inspect_metadata(args: InspectMetadataArgs) -> Result<(), AppError> {
    let req =
        tools::inspect_metadata::request_from_value(&Value::Object(build_input_args(&args.input)))?;
    let response = tools::inspect_metadata::run(req)?;

    if args.json {
        return print_json_serializable(&response);
    }

    println!(
        "sections: {}, paragraphs: {}",
        response.sections, response.paragraphs
    );
    Ok(())
}

fn run_summarize_structure(args: SummarizeStructureArgs) -> Result<(), AppError> {
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

    let req = tools::summarize_structure::request_from_value(&Value::Object(map))?;
    let response = tools::summarize_structure::run(req)?;

    if args.json {
        return print_json_serializable(&response);
    }

    let section_count = response.sections.len();
    let paragraph_count: usize = response
        .sections
        .iter()
        .map(|section| section.paragraphs.len())
        .sum();
    println!("sections: {section_count}, paragraphs: {paragraph_count}");
    Ok(())
}

fn run_render_svg(args: RenderSvgArgs) -> Result<(), AppError> {
    let mut map = build_input_args(&args.input);
    if let Some(page) = args.page {
        map.insert("page".to_string(), json!(page));
    }
    if !args.pages.is_empty() {
        map.insert("pages".to_string(), json!(args.pages));
    }
    map.insert("output".to_string(), json!("inline"));

    let req = tools::render_svg::request_from_value(&Value::Object(map))?;
    let response = tools::render_svg::run(req)?;
    let structured = response.structured_content;
    let pages = structured
        .get("pages")
        .and_then(|value| value.as_array())
        .ok_or_else(|| AppError::internal("render_svg response is missing pages"))?;

    let output_dir = PathBuf::from(&args.output_dir);
    fs::create_dir_all(&output_dir)
        .map_err(|err| AppError::internal(format!("failed to create output directory: {err}")))?;

    let mut written_pages = Vec::new();
    for page in pages {
        let page_number = page
            .get("page")
            .and_then(|value| value.as_u64())
            .ok_or_else(|| AppError::internal("render_svg page is missing page number"))?;
        let svg = page
            .get("svg")
            .and_then(|value| value.as_str())
            .ok_or_else(|| AppError::internal("render_svg page is missing svg content"))?;
        let path = output_dir.join(format!("page-{page_number}.svg"));
        fs::write(&path, svg)
            .map_err(|err| AppError::internal(format!("failed to write svg output: {err}")))?;
        written_pages.push(json!({
            "page": page_number,
            "path": path.to_string_lossy().to_string()
        }));
    }

    let output = json!({
        "format": structured.get("format").cloned().unwrap_or(Value::Null),
        "pages": written_pages,
        "warnings": structured.get("warnings").cloned().unwrap_or_else(|| json!([]))
    });

    if args.json {
        return print_json_value(&output);
    }

    println!(
        "rendered {} page(s) into {}",
        output
            .get("pages")
            .and_then(|value| value.as_array())
            .map(|pages| pages.len())
            .unwrap_or(0),
        output_dir.display()
    );
    Ok(())
}

fn run_convert(args: ConvertArgs) -> Result<(), AppError> {
    let mut map = build_input_args(&args.input);
    map.insert("to".to_string(), json!(args.to.as_str()));
    map.insert("output_path".to_string(), json!(args.output));

    let req = tools::convert::request_from_value(&Value::Object(map))?;
    let response = tools::convert::run(req)?;

    if args.json {
        return print_json_value(&response.structured_content);
    }

    let path = response
        .structured_content
        .get("path")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    println!("written {path}");
    Ok(())
}

fn run_create_document(args: CreateDocumentArgs) -> Result<(), AppError> {
    let req = tools::create_document::request_from_value(&json!({
        "text": args.text,
        "output_path": args.output
    }))?;
    let response = tools::create_document::run(req)?;
    let file = response
        .file
        .ok_or_else(|| AppError::internal("create_document response is missing file output"))?;
    let output = json!({
        "path": file.path,
        "bytes_len": file.bytes_len
    });

    if args.json {
        return print_json_value(&output);
    }

    println!(
        "{}",
        output
            .get("path")
            .and_then(|value| value.as_str())
            .unwrap_or("")
    );
    Ok(())
}

fn run_create_rich_document(args: CreateRichDocumentArgs) -> Result<(), AppError> {
    let raw = fs::read_to_string(&args.input)
        .map_err(|err| AppError::invalid_input(format!("failed to read input json: {err}")))?;
    let document_value: Value = serde_json::from_str(&raw)
        .map_err(|err| AppError::invalid_input(format!("failed to parse input json: {err}")))?;
    let document = document_value
        .get("document")
        .cloned()
        .unwrap_or(document_value);

    let mut args_json = json!({
        "output_path": args.output,
        "document": document
    });
    if let Some(to) = args.to {
        args_json["to"] = json!(to.as_str());
    }

    let req = tools::create_rich_document::request_from_value(&args_json)?;
    let response = tools::create_rich_document::run(req)?;
    let file = response.file.ok_or_else(|| {
        AppError::internal("create_rich_document response is missing file output")
    })?;
    let output = json!({
        "to": response.to.as_str(),
        "path": file.path,
        "bytes_len": file.bytes_len,
        "warnings": response.warnings
    });

    if args.json {
        return print_json_value(&output);
    }

    println!(
        "{}",
        output
            .get("path")
            .and_then(|value| value.as_str())
            .unwrap_or("")
    );
    Ok(())
}

fn run_extract_rich(args: ExtractRichArgs) -> Result<(), AppError> {
    let mut map = build_input_args(&args.input);
    map.insert("images".to_string(), json!(args.images.as_str()));
    if let Some(max_image_bytes) = args.max_image_bytes {
        map.insert("max_image_bytes".to_string(), json!(max_image_bytes));
    }
    if let Some(output_dir) = args.output_dir {
        map.insert("output_path".to_string(), json!(output_dir));
    }

    let req = tools::extract_rich::request_from_value(&Value::Object(map))?;
    let response = tools::extract_rich::run(req)?;

    if args.json {
        return print_json_serializable(&response);
    }

    println!("extracted {} blocks", response.blocks.len());
    Ok(())
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

fn print_json_serializable<T: serde::Serialize>(value: &T) -> Result<(), AppError> {
    let serialized = serde_json::to_string_pretty(value)
        .map_err(|err| AppError::internal(format!("failed to serialize json output: {err}")))?;
    println!("{serialized}");
    Ok(())
}

fn print_json_value(value: &Value) -> Result<(), AppError> {
    let serialized = serde_json::to_string_pretty(value)
        .map_err(|err| AppError::internal(format!("failed to serialize json output: {err}")))?;
    println!("{serialized}");
    Ok(())
}
