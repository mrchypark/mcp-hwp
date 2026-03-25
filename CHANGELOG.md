# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog and this project adheres to Semantic Versioning.

## [Unreleased]
- Restructured the project around a CLI-first binary with typed tool operations in the shared core.
- Completed the public CLI surface for conversion, SVG rendering, plain document creation, rich document creation, and rich extraction.
- Moved MCP stdio routing and schemas into dedicated adapter modules while preserving existing MCP tool names and success payload shapes.
