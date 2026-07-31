# Project Context

## Purpose

Lexiang CLI (`lx`) is a Rust-based command-line tool that provides a unified interface to the Lexiang knowledge management platform. It wraps MCP (Model Context Protocol) APIs, provides a virtual shell (`lx sh`), Git worktree support, and dynamic command generation from MCP schemas. The CLI serves as the single source of truth for all data operations against the Lexiang backend.

## Tech Stack

- Rust (CLI core, clap for arg parsing, reqwest for HTTP)
- MCP (Model Context Protocol) as the API layer
- SQLite (via rusqlite) for local metadata caching
- JSON Schema for dynamic command generation
- Shell scripting (justfile, bash)

## Project Conventions

### Code Style

- Follow `rustfmt.toml` and `clippy.toml` for Rust formatting and linting
- Use `just` as the task runner (see `justfile`)
- Commit messages in conventional format

### Architecture Patterns

- Static commands defined via clap in `src/cmd/cli.rs`
- Dynamic commands auto-generated from MCP JSON schema (`schemas/lexiang.json`)
- Command dispatch: help → block commands → dynamic commands → smart resolution → static clap
- 6 output formats: json, json-pretty, table, yaml, csv, markdown
- Virtual shell (`lx sh`) provides interactive REPL with bridge commands
- Daemon mode for background operations and VFS

### Testing Strategy

- Unit tests in Rust modules
- Integration tests via `just test`

### Git Workflow

- Main branch development
- Feature branches for significant changes

## Domain Context

Lexiang is an enterprise knowledge management platform with:

- Teams → Spaces → Entries (pages/folders/files) hierarchy
- Block-based content editing (similar to Notion blocks)
- WebDAV protocol support for file sync
- MCP protocol as the unified API layer
- Knowledge search (keyword + vector/embedding)
- PPT generation, meeting scheduling, comments, file management

## Important Constraints

- CLI must work offline for cached data
- MCP schema changes require schema updates in `schemas/lexiang.json`
- All commands must support `--format json` for programmatic consumption
- Publishing: no `workspace:*` protocol; use `file:` for devDeps, SemVer for runtime deps

## External Dependencies

- Lexiang MCP backend (API server)
- Local SQLite database for metadata caching
- Local filesystem for config (`~/.lexiang/`)
