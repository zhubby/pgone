# Repository Guidelines

## Project Structure & Module Organization
- `pgone-mcp-server/` — core MCP server, DB adapters, formatters, examples (`examples/`).
- `pgone/` — CLI entry for quick runs and STDIO mode.
- `pgone-gui/` — desktop GUI (egui/eframe).
- `pgone-a2a/`, `pgone-apiserver/`, `pgone-protocol/` — scaffolding and shared crates.
- Workspace manifest: `Cargo.toml`; build artifacts in `target/`.

## Build, Test, and Development Commands
- Build all: `cargo build --workspace`
- Run MCP server (STDIO): `PGONE_CONNECTIONS_PATH=examples/connections.yaml PGONE_MCP_STDIO=1 cargo run -p pgone-mcp-server`
- One-off introspection: `PGONE_PG_DSN=postgres://… cargo run -p pgone`
- GUI: `cargo run -p pgone-gui`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Format: `cargo fmt --all`
- Tests (workspace): `cargo test --workspace`

## Coding Style & Naming Conventions
- Rust edition: 2024; use `rustfmt` defaults (4-space indent, max width by tool).
- Naming: types and traits `PascalCase`, modules/files `snake_case`, constants `SCREAMING_SNAKE_CASE`.
- Keep functions small; prefer `anyhow::Result<T>` for app-level errors; use `thiserror` for library errors.
- Logging via `tracing`; configure with `RUST_LOG`/`RUST_TRACING` env filters.

## Testing Guidelines
- Framework: Rust built-in `#[test]`; place unit tests in `mod tests { … }` next to code; integration tests under `tests/` per crate when needed.
- Name tests for behavior, e.g., `test_parses_triggers_markdown`.
- Aim to cover parsing/formatting, registry behaviors, and adapter queries (use dockerized DB or mocks).
- Run locally: `cargo test --workspace`.

## Commit & Pull Request Guidelines
- Prefer Conventional Commits: `feat:`, `fix:`, `refactor:`, `docs:`, `chore:`, `test:`.
- Keep subject ≤72 chars; include rationale and impact in body.
- PRs: clear description, linked issues (`Closes #123`), steps to validate, and screenshots for GUI changes. Update docs/examples when behavior changes.

## Security & Configuration Tips
- Do not commit secrets. Provide DSNs via env (`PGONE_PG_DSN`) or local YAML (`examples/connections.yaml`) kept out of VCS.
- Avoid logging credentials; scrub values in errors.
- Long-running processes: prefer STDIO mode for agent integrations (`PGONE_MCP_STDIO=1`).

