# mcp-hwp

Rust CLI for HWP/HWPX processing built on [`hwpers`](https://github.com/42triangles/hwpers), with an optional MCP stdio adapter in the same binary.

## What It Does

- Extract plain text from `.hwp` and `.hwpx`
- Inspect document metadata
- Summarize document structure
- Render pages as SVG
- Convert between HWP and HWPX
- Create plain HWP documents from text
- Create rich HWP/HWPX documents from JSON
- Extract rich block structures from existing documents

## Install

### From source

```bash
cargo install --path .
```

### Build locally

```bash
cargo build --release
```

## CLI Quickstart

Show help:

```bash
mcp-hwp --help
```

Extract text:

```bash
mcp-hwp extract-text --path ./document.hwp
```

Inspect metadata as JSON:

```bash
mcp-hwp inspect-metadata --path ./document.hwp --json
```

Summarize structure with limits:

```bash
mcp-hwp summarize-structure \
  --path ./document.hwp \
  --json \
  --max-paragraphs-per-section 1 \
  --preview-chars 20
```

Convert to HWPX:

```bash
mcp-hwp convert \
  --path ./document.hwp \
  --to hwpx \
  --output ./converted.hwpx
```

Render page 1 as SVG files:

```bash
mcp-hwp render-svg \
  --path ./document.hwp \
  --page 1 \
  --output-dir ./svg-output
```

Create a plain document:

```bash
mcp-hwp create-document \
  --text "Hello\n안녕" \
  --output ./created.hwp
```

Create a rich document from JSON:

```bash
mcp-hwp create-rich-document \
  --input ./document.json \
  --to hwp \
  --output ./rich.hwp
```

Extract a rich block model:

```bash
mcp-hwp extract-rich --path ./document.hwp --json
```

Extract rich content with image resources written to a directory:

```bash
mcp-hwp extract-rich \
  --path ./document.hwp \
  --images resource \
  --output-dir ./images \
  --json
```

## CLI Commands

- `extract-text`
- `inspect-metadata`
- `summarize-structure`
- `render-svg`
- `convert`
- `create-document`
- `create-rich-document`
- `extract-rich`
- `serve --stdio`

File-producing commands are explicit by design:

- `convert`, `create-document`, and `create-rich-document` require `--output`
- `render-svg` requires `--output-dir`

## Inputs

Read operations accept:

- `--path <file>`
- `--base64 <encoded-bytes>`
- optional `--format auto|hwp|hwpx`

Exactly one of `--path` or `--base64` must be provided.

`extract-rich --images` accepts `none`, `metadata`, `inline`, or `resource`.
When using `resource`, pass `--output-dir <dir>` to control where extracted image files are written.

## Rich Document JSON

`create-rich-document --input` expects a JSON object for the `document` payload. The file may contain either the document object directly or a wrapper with a top-level `document` key.

Example:

```json
{
  "title": "Example",
  "blocks": [
    { "type": "heading", "level": 1, "text": "Intro" },
    { "type": "paragraph", "text": "Hello rich world" },
    { "type": "list", "items": ["one", "two"], "ordered": false },
    { "type": "page_break" },
    {
      "type": "table",
      "rows": [
        ["Name", "City"],
        ["Alice", "Seoul"]
      ],
      "header_row": true
    }
  ]
}
```

## Optional MCP Integration

The same binary can serve MCP over stdio:

```bash
mcp-hwp serve --stdio
```

This adapter keeps the existing MCP tool names:

- `hwp.extract_text`
- `hwp.inspect_metadata`
- `hwp.summarize_structure`
- `hwp.render_svg`
- `hwp.convert`
- `hwp.create_document`
- `hwp.create_rich_document`
- `hwp.extract_rich`

Successful MCP responses keep the existing `content`, `structuredContent`, and `isError` shape.

### Claude Desktop

```json
{
  "mcpServers": {
    "mcp-hwp": {
      "command": "mcp-hwp",
      "args": ["serve", "--stdio"]
    }
  }
}
```

### Codex CLI

```toml
[mcp_servers.mcp-hwp]
command = "mcp-hwp"
args = ["serve", "--stdio"]
```

## Breaking Changes

- The project is now documented and structured as CLI-first.
- The full CLI command surface is implemented instead of partially stubbed commands.
- MCP remains available as an optional stdio adapter in the same binary.

## Development

Run the test suite:

```bash
cargo test
```

Check command help:

```bash
cargo run -- --help
cargo run -- create-rich-document --help
```
