# PGone

A comprehensive toolkit for PostgreSQL database intelligence, featuring a desktop GUI, MCP server, and local storage layer.

## Overview

PGone provides:
- **Interactive GUI** for database exploration and SQL experimentation
- **MCP Server** exposing database capabilities via the Model Context Protocol
- **Local Storage** using embedded SQLite (libsql/Turso) for session persistence

---

## Architecture

### Core Modules

- **`pgone-gui`** — Desktop application built with egui/eframe
  - Database connection management
  - SQL editor with syntax highlighting
  - Query results visualization
  - Chat panel with LLM integration
  - Schema browser and graph visualization
  - Performance monitoring

- **`pgone-mcp-server`** — MCP protocol server
  - Connection registry (YAML-based or environment variables)
  - Database introspection and schema discovery
  - STDIO and Streamable HTTP transport modes

- **`pgone-storage`** — Embedded local storage
  - SQLite-based persistence (libsql/Turso)
  - Session and message management
  - Automatic migration from legacy JSON format

### Supporting Modules

- **`pgone-sql`** — SQL parsing and database models
- **`pgone-llm`** — LLM provider integrations (OpenAI, Gemini, Ollama)
- **`pgone-auditor`** — Database auditing and query analysis
- **`pgone-apiserver`** — HTTP/gRPC API server
- **`pgone-a2a`** — Agent-to-agent communication
- **`pgone-util`** — Shared utilities and logging
- **`pgone-vector`** — Vector database support
- **`pgone-mcp-client`** — MCP client implementation

---

## Quick Start

### Build

```bash
cargo build --workspace
```

### Run GUI

```bash
cargo run -p pgone-gui
```

### MCP Server

**STDIO mode** (for agent integrations):
```bash
PGONE_CONNECTIONS_PATH=examples/connections.yaml \
PGONE_MCP_PROTOCOL=stdio \
cargo run -p pgone-mcp-server
```

**Streamable HTTP mode** (default):
```bash
PGONE_MCP_PROTOCOL=streamable \
PGONE_MCP_ADDR=127.0.0.1:3000 \
cargo run -p pgone-mcp-server
```

### Quick Introspection

```bash
PGONE_PG_DSN=postgres://user:pass@host:5432/dbname \
cargo run -p pgone-mcp-server
```

---

## Configuration

### Environment Variables

- `PGONE_MCP_PROTOCOL` — MCP transport mode (`stdio` or `streamable`, default: `streamable`)
- `PGONE_MCP_ADDR` — HTTP server address (default: `127.0.0.1:3000`)
- `PGONE_CONNECTIONS_PATH` — Path to connections YAML file
- `PGONE_PG_DSN` — PostgreSQL connection string for quick introspection
- `RUST_LOG` — Logging filter (e.g., `info`, `debug`)

### GitHub OAuth (GUI)

1. Create a GitHub OAuth App with callback URL: `http://127.0.0.1:8765/oauth/github/callback`
2. Set environment variables:
   ```bash
   export GITHUB_CLIENT_ID=<your_client_id>
   export GITHUB_CLIENT_SECRET=<your_client_secret>
   export OAUTH_REDIRECT=http://127.0.0.1:8765/oauth/github/callback
   ```
3. Launch GUI — authentication state is persisted in `pgone.db`

---

## Development

### Build & Test

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

### Code Style

- Rust 2024 edition
- Modules/files: `snake_case`
- Types/traits: `PascalCase`
- Constants: `SCREAMING_SNAKE_CASE`
- Error handling: `anyhow::Result<T>` for application code, `thiserror` for libraries
- Logging: `tracing` with `RUST_LOG` filter

### Testing

- Unit tests: `mod tests {}` next to source code
- Integration tests: `tests/` directory per crate
- Test naming: behavior-focused (e.g., `test_parses_triggers_markdown`)

---

## Storage

The GUI automatically initializes `pgone.db` (SQLite) in the project root on first launch. Legacy `sessions.json` files are automatically migrated and removed.

**Schema** (indexed, no foreign keys):
- `db_configs` — Database connection configurations
- `sessions` — Chat sessions linked to configurations
- `messages` — Session messages (markdown, images, videos)

---

## Security

- **Never commit secrets** — use environment variables or local YAML files
- **Avoid logging credentials** — sanitize connection strings in error messages
- **Prefer STDIO mode** for long-running agent integrations

---

## Packaging

```bash
cargo install cargo-bundle
cargo bundle --release
```

---

## License

Apache-2.0

---

## Contributing

Issues and PRs welcome. Please use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages.
