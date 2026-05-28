# pgone-mcp

`pgone-mcp` owns PGone's agent-facing PostgreSQL introspection surface.

The crate has two responsibilities:

1. expose read-only database introspection as MCP tools, and
2. provide shared introspection models, adapters, and renderers that other PGone crates can reuse without speaking MCP.

It is not a general database UI layer, SQL execution layer, storage layer, or agent runtime.

## Current Role In The Workspace

`pgone-mcp` is used by:

- `pgone-cli`, which starts the MCP server through the `mcp-server` subcommand.
- `pgone-agent`, which reuses the `SqlSessionIntrospector`, core models, and ER/DBML formatters for local tool execution.
- `pgone-gui`, which contains a currently inactive MCP client manager path. GUI screens should prefer direct `pgone-sql` or `pgone-agent` integration unless the feature specifically needs an MCP transport boundary.

## Owned Capabilities

The crate owns these areas:

- MCP server implementation for PGone database tools.
- MCP tool metadata, request validation, and tool dispatch.
- STDIO and Streamable HTTP MCP server transports.
- Read-only PostgreSQL introspection adapters built on `pgone-sql::Session`.
- Agent-facing database schema models under `core`.
- Renderers for agent-friendly formats such as Markdown, Mermaid ER diagrams, and DBML.
- A compatibility binary named `pgone-mcp-server`.

The current tool set includes:

- `introspect_all`
- `get_table`
- `list_triggers`
- `list_routines`
- `list_types`
- `render_er`
- `render_dbml`
- `health_check`

## Explicit Non-Goals

Do not put these responsibilities in `pgone-mcp`:

- GUI state, panels, layouts, or user interaction flows.
- Chat session state, model-provider integration, prompt orchestration, or agent turn management.
- Persistent application settings or database connection storage schema.
- General SQL query execution, mutation workflows, or query result table rendering.
- PostgreSQL protocol/session primitives that belong in `pgone-sql`.
- Desktop process lifecycle policy that belongs in `pgone-gui` or `pgone-cli`.

## Dependency Direction

`pgone-mcp` may depend on:

- `pgone-sql` for PostgreSQL sessions and metadata queries.
- `pgone-storage` to resolve stored database configurations for server runs.
- `pgone-util` for shared application utilities.
- MCP transport/protocol crates such as `rmcp`.

Other crates may depend on `pgone-mcp` when they need its agent-facing introspection contract or its MCP server/client types.

Avoid making `pgone-mcp` depend on `pgone-agent`, `pgone-gui`, or `pgone-cli`. Those crates are consumers of this module, not owners of the introspection protocol.

## Public API Guidance

Use the crate APIs this way:

- Use `pgone_mcp::mcp::run_stdio` or `pgone_mcp::mcp::run_streamable` to start a real MCP server.
- Use `pgone_mcp::mcp::PgoneMcpServer::call_tool_direct` only when a caller wants the same tool behavior without an MCP transport.
- Use `pgone_mcp::adapter::SqlSessionIntrospector` when an internal crate needs read-only schema introspection without MCP.
- Use `pgone_mcp::formatters` when an internal crate needs the same Markdown, Mermaid, or DBML output as MCP tools.
- Treat `pgone_mcp::client::McpClient` as incomplete for STDIO client use. The server transports are the production path today.

## Running The MCP Server

The preferred entrypoint is the unified CLI:

```bash
cargo run -p pgone-cli -- mcp-server --dbconfig-id default --protocol stdio
```

Streamable HTTP mode:

```bash
cargo run -p pgone-cli -- mcp-server --dbconfig-id default --protocol streamable --addr 127.0.0.1:3000
```

The compatibility binary remains available:

```bash
cargo run -p pgone-mcp --bin pgone-mcp-server -- --dbconfig-id default --protocol stdio
```

The server resolves the selected database through PGone storage using `--dbconfig-id`; DSNs should stay in local configuration or storage, not in source-controlled examples.

## Tool Contract Notes

MCP tool arguments are tool-specific and do not require a `connectionId`. The active database is selected by the server's `dbconfig_id` at startup.

Examples:

```json
{"schemas":["public"],"with_indexes":true,"with_routines":true,"format":"markdown"}
```

```json
{"schema":"public","table":"orders","format":"markdown"}
```

```json
{"schemas":["public"]}
```

Tool responses are returned as MCP `CallToolResult` text content containing serialized JSON. Direct tool calls return `serde_json::Value`.

## Safety

The MCP tools are intended to be read-only. They should inspect catalog metadata, render summaries, and check connectivity. Do not add tools here that mutate database data or schema unless the module's charter is explicitly revised.

Generated examples and diagnostics must not expose credentials. DSNs should be read from PGone's local storage or environment-specific configuration and scrubbed from logs and error messages where practical.

## Known Gaps

- The README documents the current implementation, not a future connection registry protocol.
- `McpClient` does not yet implement full STDIO JSON-RPC communication.
- The GUI MCP client manager exists, but the main GUI app currently does not initialize it.
- Introspection currently targets PostgreSQL through `pgone-sql`.

## License

Same as the repository.
