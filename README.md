# mcp-hwp (hwpers-cli-mcp)

Rust CLI + stdio MCP server for reading/writing HWP/HWPX using the `hwpers` crate.

## What This Is

- CLI: run from terminal for local conversions/extraction
- MCP server: stdio JSON-RPC (NDJSON: one JSON message per line) for integration with MCP clients (e.g., Claude Desktop)

## Supported Inputs

- `path` (local file path)
- `base64` (base64-encoded bytes)
- Exactly one of `path` or `base64` must be provided.
- Optional `format`: `auto` | `hwp` | `hwpx`

## Implemented MCP Tools

- `hwp.extract_text`
- `hwp.inspect_metadata`
- `hwp.summarize_structure`
- `hwp.render_svg`
- `hwp.convert`
- `hwp.create_document`
- `hwp.create_rich_document`
- `hwp.extract_rich`

## Quickstart

### Build

```bash
cargo build
```

### Install locally

```bash
cargo install --path .
```

### Run MCP stdio server

```bash
mcp-hwp serve --stdio
```

## MCP Client Setup

This MCP server uses stdio. Most clients require `command` + `args`.

### Claude Desktop

Config file location:

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`
- Linux: `~/.config/Claude/claude_desktop_config.json`

Add:

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

If you don't `cargo install`, you can point the config to your built binary:

```json
{
  "mcpServers": {
    "mcp-hwp": {
      "command": "./target/debug/mcp-hwp",
      "args": ["serve", "--stdio"]
    }
  }
}
```

### Claude Code (Anthropic)

Option A: register via CLI (stdio):

```bash
claude mcp add --transport stdio mcp-hwp -- mcp-hwp serve --stdio
```

Option B: project-scoped config via `.mcp.json` (checked into repo):

```json
{
  "mcpServers": {
    "mcp-hwp": {
      "type": "stdio",
      "command": "mcp-hwp",
      "args": ["serve", "--stdio"]
    }
  }
}
```

### OpenAI Codex CLI

Option A: register via CLI:

```bash
codex mcp add mcp-hwp -- mcp-hwp serve --stdio
```

Option B: edit `~/.codex/config.toml` (or project-scoped `.codex/config.toml`):

```toml
[mcp_servers.mcp-hwp]
command = "mcp-hwp"
args = ["serve", "--stdio"]
```

### Gemini CLI

Option A: register via CLI (default transport is stdio):

```bash
gemini mcp add mcp-hwp mcp-hwp serve --stdio
```

Option B: edit `~/.gemini/settings.json` (or project-scoped `.gemini/settings.json`):

```json
{
  "mcpServers": {
    "mcp-hwp": {
      "command": "mcp-hwp",
      "args": ["serve", "--stdio"],
      "timeout": 30000,
      "trust": false
    }
  }
}
```

### OpenCode

Add to your OpenCode config:

- global: `~/.config/opencode/opencode.json`
- per-project: `opencode.json` in project root

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "mcp-hwp": {
      "type": "local",
      "command": ["mcp-hwp", "serve", "--stdio"],
      "enabled": true
    }
  }
}
```

## CLI Usage

Show help:

```bash
mcp-hwp --help
```

Extract text (human output):

```bash
mcp-hwp extract-text --path ./document.hwp
```

Inspect metadata (JSON):

```bash
mcp-hwp inspect-metadata --path ./document.hwp --json
```

Summarize structure (JSON with limits):

```bash
mcp-hwp summarize-structure --path ./document.hwp --json --max-paragraphs-per-section 1 --preview-chars 20
```

## MCP Protocol Notes

- Transport: stdio
- Framing: NDJSON (one JSON-RPC request/response per line)
- Minimal methods supported:
  - `initialize`
  - `tools/list`
  - `tools/call`

### Example: initialize

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"example","version":"0"}}}
```

### Example: tools/list

```json
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
```

### Example: tools/call

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"hwp.extract_text","arguments":{"path":"./document.hwp"}}}
```

## Tool Reference (Arguments + Outputs)

All tools return:

- `result.isError`: boolean
- `result.content`: array of content blocks (human-oriented)
- `result.structuredContent`: JSON object (machine-oriented)

### hwp.extract_text

Arguments:
- `path` or `base64`
- `format`: `auto`|`hwp`|`hwpx`
- `max_chars`: integer
- `include_newlines`: boolean
- `normalize_whitespace`: boolean

structuredContent:
- `{ "text": "..." }`

### hwp.inspect_metadata

Arguments:
- `path` or `base64`
- `format`: `auto`|`hwp`|`hwpx`

structuredContent (best-effort):
- `format`: `hwp`|`hwpx`
- `sections`: integer
- `paragraphs`: integer
- `warnings`: string[]
- `encrypted`: boolean
- `compressed`: boolean
- `version`: string

### hwp.summarize_structure

Arguments:
- `path` or `base64`
- `format`: `auto`|`hwp`|`hwpx`
- `max_sections`: integer
- `max_paragraphs_per_section`: integer
- `preview_chars`: integer

structuredContent:
- `format`: `hwp`|`hwpx`
- `sections`: array of `{ index, paragraphs: [{ index, char_count, preview }] }`
- `warnings`: string[]

### hwp.render_svg

Arguments:
- `path` or `base64`
- `format`: `auto`|`hwp`|`hwpx`
- `page`: integer (1-based)
- `pages`: integer[] (1-based)
- `output`: `inline`|`resource`

structuredContent:
- `format`: `hwp`|`hwpx`
- `pages`: array of:
  - inline: `{ page, svg }`
  - resource: `{ page, path, uri }`
- `warnings`: string[]

### hwp.convert

Arguments:
- `path` or `base64`
- `format`: `auto`|`hwp`|`hwpx`
- `to` (required): `hwp`|`hwpx`
- `output_path` (optional)

structuredContent:
- inline: `{ to, base64, bytes_len, warnings }`
- resource: `{ to, path, uri, bytes_len, warnings }`

### hwp.create_document

Arguments:
- `text` (required)
- `output_path` (optional)

Behavior:
- splits `text` by newline into paragraphs; preserves blank lines as empty paragraphs

structuredContent:
- inline: `{ base64, bytes_len }`
- resource: `{ path, uri, bytes_len }`

### hwp.create_rich_document

Arguments:
- `to` (optional): `hwp`|`hwpx` (default: `hwp`)
- `output_path` (optional)
- `document` (required): block-based spec
  - `title` (optional)
  - `author` (optional)
  - `header` / `footer` (optional; best-effort, varies by output format)
  - `blocks` (required): array of
    - `paragraph`: `{ type: "paragraph", text, style? }`
      - `style`: `{ font_name?, font_size?, bold?, italic?, underline?, color? }`
        - `color`: hex string (e.g., `"0xFF0000"`, `"#FF0000"`)
    - `heading`: `{ type: "heading", level, text }`
    - `table`: `{ type: "table", rows, header_row? }`
    - `image`: `{ type: "image", path? | data_base64?, mimeType?, width_mm?, height_mm?, caption?, align?, wrap_text? }`
      - `path`: local file path to image (alternative to `data_base64`)
      - `data_base64`: base64-encoded image data (requires `mimeType`)
      - `mimeType`: `"image/png"`, `"image/jpeg"`, `"image/gif"`, `"image/bmp"`
      - `align`: `"left"`, `"center"`, `"right"`, `"inline"` (default: `"center"`)
      - `wrap_text`: boolean (default: `false`)
    - `page_break`: `{ type: "page_break" }` - **not fully supported** (adds empty paragraph)
    - `list`: `{ type: "list", items, list_type? | ordered? }`
      - `items`: array of strings
      - `list_type`: `"bullet"`, `"numbered"`, `"alphabetic"`, `"roman"`, `"korean"` (default: `"bullet"`)
      - `ordered`: boolean (legacy, use `list_type: "numbered"` instead)
    - `heading`: `{ type: "heading", level, text }`
    - `table`: `{ type: "table", rows, header_row?, border_style? }`
      - `rows`: array of arrays (cells can be strings or objects)
        - Simple: `["cell1", "cell2"]`
        - Advanced: `{ "content": "text", "row_span?": number, "col_span?": number }`
      - `border_style`: `"none"`, `"basic"`, `"full"` (default: none)
      - Note: `row_span`/`col_span` supported for cell merging; cell-level styling not supported
    - `image`: `{ type: "image", path? | data_base64?, mimeType?, width_mm?, height_mm?, caption?, align?, wrap_text? }`

structuredContent:
- inline: `{ to, base64, bytes_len, warnings }`
- resource: `{ to, path, uri, bytes_len, warnings }`

### hwp.extract_rich

Arguments:
- `path` or `base64`
- `format`: `auto`|`hwp`|`hwpx`
- `images`: `none`|`metadata`|`inline`|`resource` (default: `metadata`)
- `max_image_bytes` (optional)
- `output_path` (optional): custom directory for saving extracted images (when `images` is `resource`)

structuredContent:
- `{ format, blocks, warnings }`
- `blocks` contains a best-effort ordered list of:
  - `{ type: "paragraph", text, section_index, paragraph_index }`
  - `{ type: "table", rows, inferred, cells_count, section_index, paragraph_index }`
  - `{ type: "image", caption?, ... }` (caption-anchored; image bytes may be unavailable depending on parser)
  - Images with `images: "resource"` include `path` and `uri` fields

## Errors

Tool failures are returned as tool results (not JSON-RPC errors):

```json
{
  "content": [{"type": "text", "text": "Error: ..."}],
  "structuredContent": {
    "error": {
      "kind": "invalid_input",
      "message": "...",
      "source": "path:..."
    }
  },
  "isError": true
}
```

`error.kind` taxonomy:
- `invalid_input`
- `too_large`
- `unsupported_format`
- `encrypted`
- `parse_failed`
- `internal_error`

## Limits

Defaults are constants in `src/mcp/contracts.rs`:

- `MAX_INPUT_BYTES = 50 MiB` (decoded bytes)
- `MAX_OUTPUT_BYTES = 20 MiB` (inline base64 outputs)
- `MAX_SVG_OUTPUT_BYTES = 50 MiB` (SVG total)
- `MAX_PARSE_MS = 10_000` (reserved; not enforced everywhere yet)

## Security Notes

- No URL fetching; inputs are local `path` or provided `base64`.
- `output_path` writes files to disk. Treat it as a privileged operation and avoid untrusted paths.
- Size limits are enforced to reduce memory/transport risk.

## Limitations

This project depends on `hwpers`, which (as of writing):

- targets HWP 5.0; older formats may not parse
- does not support password-encrypted documents
- may not fully support all objects (shapes/charts/equations/etc.) for parsing/rendering

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

## License

MIT (see `LICENSE`).
