<div align="center">

# PGone

<img src="pgone-gui/assets/images/banner.png" alt="PGone banner">

**A local PostgreSQL workspace with a desktop GUI, SQL workbench, schema explorer, performance views, persistent sessions, and an MCP server for AI agents.**

PostgreSQL insight, query execution, visual exploration, and agent-ready introspection in one Rust workspace.

</div>

## What PGone Does

PGone is a toolkit for working with PostgreSQL locally. It combines a desktop application for humans with an MCP server for AI agents, backed by local storage for database configurations and sessions.

Core capabilities:

- **Connection management**: save, edit, test, and reuse PostgreSQL connection profiles from the desktop GUI.
- **SQL workbench**: write and run SQL, inspect paginated results, and switch the target database from the active connection.
- **Schema exploration**: browse databases, schemas, tables, columns, indexes, constraints, triggers, routines, and custom types.
- **Visual modeling**: inspect table relationships in the GUI and render schema output as Mermaid ER diagrams or DBML through MCP tools.
- **Database monitoring**: view activity, statement statistics, tables, indexes, locks, replication, and bgwriter metrics.
- **Agent integration**: expose read-only PostgreSQL introspection through the Model Context Protocol.
- **Local persistence**: store database configs, chat sessions, and messages in `~/.pgone/pgone.db`.

## Quick Start

### 1. Build

```bash
cargo build --workspace
```

### 2. Launch The Desktop GUI

The default command opens the GUI:

```bash
cargo run -p pgone-cli --
```

The explicit GUI command is equivalent:

```bash
cargo run -p pgone-cli -- gui
```

After launch, add a PostgreSQL connection in the GUI. Connection settings are stored locally in `~/.pgone/pgone.db` and are not written to the repository.

### 3. Explore And Query PostgreSQL

Once a connection is selected, use the GUI to:

- run SQL and inspect query results;
- browse database structure, table DDL, indexes, and related objects;
- open relationship graphs for selected database structures;
- inspect PostgreSQL runtime monitoring panels.

### 4. Run The MCP Server

The MCP server uses database configurations saved in PGone local storage. Pass the saved configuration ID with `--dbconfig-id`.

STDIO mode is intended for local agent integrations:

```bash
cargo run -p pgone-cli -- mcp-server --dbconfig-id default --protocol stdio
```

Streamable HTTP mode exposes the MCP server over HTTP:

```bash
cargo run -p pgone-cli -- mcp-server --dbconfig-id default --protocol streamable --addr 127.0.0.1:3000
```

You can also set the default protocol with an environment variable:

```bash
PGONE_MCP_PROTOCOL=stdio cargo run -p pgone-cli -- mcp-server --dbconfig-id default
```

## MCP Tools

PGone MCP currently exposes read-only PostgreSQL introspection tools:

- `introspect_all`: return a database structure overview.
- `get_table`: inspect columns, constraints, indexes, and table metadata.
- `list_triggers`: list triggers.
- `list_routines`: list functions and procedures.
- `list_types`: list custom PostgreSQL types.
- `render_er`: render a Mermaid ER diagram.
- `render_dbml`: render DBML.
- `health_check`: verify that the configured database connection is available.

Example tool arguments:

```json
{"schemas":["public"],"with_indexes":true,"with_routines":true,"format":"markdown"}
```

```json
{"schema":"public","table":"orders","format":"markdown"}
```

## Common Commands

```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Set the log level for CLI commands:

```bash
cargo run -p pgone-cli -- --log-level debug gui
cargo run -p pgone-cli -- mcp-server --dbconfig-id default --protocol stdio --log-level warn
```

The compatibility MCP binary is still available:

```bash
cargo run -p pgone-mcp --bin pgone-mcp-server -- --dbconfig-id default --protocol stdio
```

## Workspace Layout

- `pgone-cli`: unified command-line entrypoint for launching the GUI or MCP server.
- `pgone-gui`: desktop application with connection management, SQL workbench, schema exploration, monitoring, and sessions.
- `pgone-mcp`: MCP server, tool definitions, request handling, PostgreSQL introspection, and formatted output.
- `pgone-sql`: PostgreSQL sessions, SQL parsing, and database metadata models.
- `pgone-storage`: local SQLite/libsql storage for connection profiles, sessions, and messages.
- `pgone-agent`: agent-side tool wrappers and direct local invocation support.
- `pgone-util`: shared utilities and logging setup.

## Configuration And Data

Common environment variables:

- `PGONE_MCP_PROTOCOL`: MCP transport protocol, either `stdio` or `streamable`.
- `PGONE_MCP_ADDR`: Streamable HTTP bind address, defaulting to `127.0.0.1:3000`.
- `RUST_LOG`: Rust log filter, such as `info` or `debug`.

Local data paths:

- `~/.pgone/pgone.db`: database configurations, sessions, and messages.
- `~/.pgone/data/`: local indexed file data.

## Security

- Do not commit DSNs, passwords, tokens, or local storage files.
- Store PostgreSQL credentials in PGone local storage, environment variables, or local-only configuration.
- MCP tools are currently intended for read-only introspection and should not run destructive SQL.
- Scrub credentials from logs, screenshots, examples, and generated output.

## License

Apache-2.0

## Contributing

Issues and PRs welcome. Please use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages.
