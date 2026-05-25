# Repository Guidelines

## Project Structure & Module Organization

This repository is a Rust workspace for PostgreSQL tooling. Crates are split by responsibility:

- `pgone-mcp`: MCP server/client implementation, database registry, PostgreSQL introspection, STDIO and Streamable HTTP transports.
- `pgone`: CLI entrypoint for quick runs and PostgreSQL extension-related code.
- `pgone-gui`: desktop GUI built with `egui`/`eframe`.
- `pgone-storage`: embedded local storage backed by SQLite/libsql/Turso.
- `pgone-sql`: SQL parsing, database metadata models, and PostgreSQL session helpers.
- `pgone-llm`: LLM provider integrations and related model/tool APIs.
- `pgone-proxy`: PostgreSQL proxy, query extraction, replay, and row/type conversion.
- `pgone-apiserver`: HTTP/gRPC API server and proxy service definitions.
- `pgone-agent`, `pgone-vector`, `pgone-util`: agent scaffolding, vector support, and shared utilities.

Keep new code in the crate that owns the domain concern. Avoid leaking GUI-specific state into protocol/storage crates, and avoid putting database adapter behavior into CLI or UI layers.

## Build, Test, and Development Commands

Use workspace-level commands from the repository root. Per local tooling policy, prefix shell commands with `rtk` when running them from an agent session.

- `cargo check --workspace`: fast compile verification.
- `cargo build --workspace`: build all crates.
- `cargo test --workspace`: run unit and integration tests.
- `cargo test -p pgone-sql --test integration_test`: run PostgreSQL-backed SQL integration tests when `PGONE_TEST_DSN` is set.
- `cargo fmt --all`: apply Rust formatting.
- `cargo clippy --workspace --all-targets -- -D warnings`: lint strictly.
- `cargo run -p pgone-gui`: run the desktop GUI.
- `cargo run -p pgone-proxy`: run the PostgreSQL proxy.
- `cargo run -p pgone-apiserver`: run the API server.

MCP server examples:

```bash
PGONE_MCP_PROTOCOL=stdio \
cargo run -p pgone-mcp --bin pgone-mcp-server -- --protocol stdio --dbconfig-id <id>

PGONE_MCP_PROTOCOL=streamable \
cargo run -p pgone-mcp --bin pgone-mcp-server -- --protocol streamable --addr 127.0.0.1:3000 --dbconfig-id <id>
```

Some existing `justfile`/`Makefile` targets still use `PGONE_MCP_STDIO=1`; prefer `PGONE_MCP_PROTOCOL=stdio` or `--protocol stdio` for new docs and scripts unless maintaining compatibility with those targets.

## Rust Style and Idioms

- Target Rust 2024 for new code and examples.
- Use concrete types (`struct`/`enum`) over `serde_json::Value` wherever the shape is known.
- Match on types and enums, not strings; convert to strings only at serialization/display boundaries.
- Prefer `From`/`TryFrom` conversions over ad hoc helper conversions.
- Prefer streaming or paginated database access for large result sets.
- Run independent async work concurrently with `tokio::join!` or `futures::join_all`.
- Never use `block_on` inside async code. Keep synchronous wrappers at explicit UI or compatibility boundaries only.
- Do not use `Mutex<()>` or `Arc<Mutex<()>>`; mutexes must guard real state.
- Use `anyhow::Result` for application binaries and `thiserror` for library error enums.
- Avoid `.unwrap()`/`.expect()` in production paths. Use `?`, `ok_or_else`, safe defaults, or explicit error handling. Test code may use unwraps when it improves clarity.
- Prefer guard clauses, `let-else`, `matches!`, pattern guards, and `Option`/`Result` combinators when they keep control flow flat and readable.
- Prefer iterators/combinators over manual loops where ownership stays obvious.
- Keep public APIs small and add `#[must_use]` where ignoring a return value is likely a bug.
- Use `tracing` for diagnostics. Do not print from library code except where a protocol explicitly requires stdout/stderr behavior.

## Workspace Dependency Management

Dependencies should be declared once in the root `Cargo.toml` under `[workspace.dependencies]`.

- Crates should reference shared dependencies with `{ workspace = true }`.
- Internal path crates should also be listed in `[workspace.dependencies]` and referenced with `{ workspace = true }`.
- Optional dependencies should use `{ workspace = true, optional = true }`.
- When crate-specific features are needed, use `{ workspace = true, features = [...] }`.

Example:

```toml
# Root Cargo.toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }

# Sub-crate Cargo.toml
[dependencies]
serde = { workspace = true }
tokio = { workspace = true, features = ["sync"] }
```

## Coding Style & Naming Conventions

Follow `rustfmt` defaults:

- modules, functions, files: `snake_case`
- types and traits: `PascalCase`
- constants: `SCREAMING_SNAKE_CASE`
- small modules with explicit ownership boundaries

For MCP tools and agent-facing APIs, make metadata planner-friendly:

- Write descriptions that clearly state when the tool should be called.
- Give parameter schemas clear field semantics, constraints, defaults, and practical examples.
- Return structured data when possible; only format Markdown at display boundaries.

## Testing Guidelines

- Place unit tests next to implementation in `mod tests`.
- Place crate integration tests under `tests/`.
- Name tests by behavior, for example `parses_trigger_definition` or `registry_rejects_missing_dsn`.
- Add regression tests for bug fixes and protocol/database edge cases.
- PostgreSQL-backed tests must use explicit environment variables such as `PGONE_TEST_DSN`; never depend on a developer's default database.
- For MCP/server changes, test request validation, transport selection, registry behavior, and formatted results.
- For storage changes, test migrations, persistence round trips, and compatibility with existing `pgone.db` records where practical.

## Database, Storage, and Configuration Safety

- Do not commit secrets. Keep DSNs in environment variables, local YAML, or local GUI storage.
- Scrub credentials from logs, errors, screenshots, and generated examples.
- Treat `pgone.db` as local state. Do not make migrations or tests depend on an unversioned developer database file.
- Avoid destructive SQL in examples and tests unless the target is a clearly isolated test database.
- Prefer connection pools or shared storage handles over opening duplicate independent connections to the same local database for background writers.
- When changing persisted schema or config formats, provide forward migrations and document any compatibility implications.

## GUI Layout and Runtime Notes

- Keep `egui` render/update paths responsive. Avoid network calls, database introspection, disk-heavy operations, or blocking waits directly in UI callbacks.
- Prefer background tasks with explicit pending state and completion/error notifications.
- Use `try_recv`/polling or request handles for GUI-to-runtime communication instead of synchronous waits.
- Coalesce periodic refreshes so a slow refresh cannot queue duplicate work.
- Prefer toast notifications or compact status areas for operation feedback; avoid large inline success/error blocks that shift layout.
- Keep text and controls within stable dimensions; avoid UI that resizes unpredictably when query results, labels, or errors change.
- Add regression coverage for GUI/runtime bridge changes where feasible, especially for slow commands, shutdown, and storage persistence.

## Documentation Guidelines

Keep documentation close to the crate it describes:

- Update the root `README.md` when workspace-level behavior, setup, or architecture changes.
- Each crate with user-facing behavior should maintain a crate-level `README.md`.
- Add or update a crate `CHANGELOG.md` when modifying that crate's behavior, public API, protocol surface, storage schema, or user-visible workflow.
- Use fenced code blocks with language tags for commands, SQL, TOML, YAML, and JSON.
- Keep command examples executable and consistent with current binary names and environment variables.
- Document required external services such as PostgreSQL, Ollama, OpenAI/Gemini APIs, or GitHub OAuth.

## Commit Guidelines

Commit messages follow Conventional Commits. Keep each commit to one logical change.

Format:

```text
<type>(<scope>): <subject>

<body>

<footer>
```

- Subject line: imperative mood, lowercase, no trailing period, max 72 characters.
- Body: explain what changed and why.
- Footer: use for `BREAKING CHANGE:`, `Closes #123`, and similar metadata.

Common types:

| Type | Description |
| --- | --- |
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation changes |
| `style` | Formatting-only changes |
| `refactor` | Code refactoring without behavior change |
| `perf` | Performance improvements |
| `test` | Test additions or corrections |
| `chore` | Maintenance tasks, dependencies, tooling |
| `ci` | CI/CD configuration changes |
| `build` | Build system or external dependency changes |
| `revert` | Reverting a previous commit |

Examples:

```text
feat(mcp): add streamable transport selection

fix(gui): avoid blocking status refresh in database panel

docs: expand agent repository guidelines
```

## Pull Request Guidelines

PRs should include:

- purpose and impacted crates,
- test evidence with commands run and results,
- config, migration, or documentation updates when behavior changes,
- sample CLI/MCP output when user-facing behavior is modified,
- screenshots or short recordings for GUI changes.
