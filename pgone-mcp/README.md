# pgone-mcp

A database introspection module based on MCP (Model Context Protocol), including server and client implementations. The server currently supports PostgreSQL, with features:
- List/describe tables, views (including materialized views), triggers, routines (functions/procedures/aggregates), types (enums/domains/composites)
- Output structured JSON and LLM-friendly Markdown summaries
- Generate ER diagrams (Mermaid) and DBML text
- Multi-database: managed with a fixed `connectionId`; built-in caching and per-table refresh

> More databases can be added in the future by implementing a new adapter for the unified introspection interface.

---

## Quick Start

### MCP STDIO Mode (Recommended)
1) Optional: prepare a YAML connection configuration
```yaml
connections:
  - id: main
    engine: postgres
    dsn: postgres://user:pass@host:5432/dbname
```
2) Start (run from the workspace root or this module's directory)
```bash
PGONE_CONNECTIONS_PATH=/path/to/connections.yaml PGONE_MCP_STDIO=1 cargo run -p pgone-mcp --bin pgone-mcp-server
```
The compatible executable name is still `pgone-mcp-server`, provided by the `pgone-mcp` crate.
```bash
PGONE_CONNECTIONS_PATH=/path/to/connections.yaml PGONE_MCP_STDIO=1 cargo run --bin pgone-mcp-server
```
You can also start via the unified CLI:
```bash
cargo run -p pgone-cli -- mcp-server --dbconfig-id default --protocol stdio
```
3) Exchange "one JSON per line" messages with the process via STDIO.

### One-shot Quick Introspection (Non-MCP)
```bash
PGONE_PG_DSN='postgres://user:pass@host:5432/dbname' cargo run -p pgone-mcp --bin pgone-mcp-server
```
Prints the database schema JSON after running.

---

## Interaction and Conventions
- Each request/response is a single line of JSON text.
- Request example:
```json
{"id":1,"method":"list_connections","params":{}}
```
- Success response: `{"id":1,"result":...}`; failure response: `{"id":1,"error":{"code":...,"message":"..."}}`
- Parameter naming uses camelCase; connections are identified by a fixed `connectionId`.

---

## Methods

- register_connection
  - Input: `{ id: string, engine: 'postgres', dsn: string }`
  - Output: `{ ok: true }`

- list_connections
  - Input: `{}`
  - Output: `[{ id: string, engine: 'postgres' }]`

- remove_connection
  - Input: `{ id: string }`
  - Output: `{ removed: boolean }`

- health_check
  - Input: `{ id: string }`
  - Output: `{ ok: boolean }`

- introspect_all
  - Input: `{ connectionId: string, schemas?: string[], withIndexes?: boolean, withRoutines?: boolean, withTypes?: boolean, withTriggers?: boolean, page?: number, pageSize?: number, format?: 'markdown' }`
  - Output:
    - Markdown: `{ markdown: string }`
    - JSON: `DatabaseSchema` (paginated responses include `pageInfo: { page, pageSize, total }`)

- list_triggers
  - Input: `{ connectionId: string, schema?: string }`
  - Output: `TriggerDetail[]`

- list_routines
  - Input: `{ connectionId: string, schema?: string, kind?: 'function'|'procedure'|'aggregate' }`
  - Output: `RoutineDetail[]`

- list_types
  - Input: `{ connectionId: string, schema?: string, kind?: 'enum'|'domain'|'composite'|'base' }`
  - Output: `TypeDetail[]`

- get_table
  - Input: `{ connectionId: string, schema: string, table: string, format?: 'markdown' }`
  - Output: Markdown: `{ markdown: string }` or JSON: `TableDetail`

- refresh_cache
  - Input: `{ id: string, scope?: 'table', schema?: string, table?: string }`
  - Output: `{ refreshed: true }`

- reload_connections
  - Input: `{ path: string }`
  - Output: `{ ok: true }`

- render_er
  - Input: `{ connectionId: string, schemas?: string[] }`
  - Output: `{ mermaid: string }`

- render_dbml
  - Input: `{ connectionId: string, schemas?: string[] }`
  - Output: `{ dbml: string }`

---

## Examples (STDIO)

- List connections
```json
{"id":1,"method":"list_connections","params":{}}
```

- Health check
```json
{"id":2,"method":"health_check","params":{"id":"main"}}
```

- Full database Markdown summary
```json
{"id":3,"method":"introspect_all","params":{"connectionId":"main","format":"markdown","withRoutines":true,"withTypes":true,"withTriggers":true}}
```

- Paginated response (JSON)
```json
{"id":4,"method":"introspect_all","params":{"connectionId":"main","page":1,"pageSize":50}}
```

- Get a single table (Markdown)
```json
{"id":5,"method":"get_table","params":{"connectionId":"main","schema":"public","table":"orders","format":"markdown"}}
```

- Generate ER diagram (Mermaid)
```json
{"id":6,"method":"render_er","params":{"connectionId":"main","schemas":["public"]}}
```

- Generate DBML
```json
{"id":7,"method":"render_dbml","params":{"connectionId":"main","schemas":["public"]}}
```

- Per-table cache refresh
```json
{"id":8,"method":"refresh_cache","params":{"id":"main","scope":"table","schema":"public","table":"orders"}}
```

---

## Data Models (Excerpt)

- DatabaseSchema (abbreviated)
```json
{
  "database": "dbname",
  "schemas": [
    {
      "name": "public",
      "tables": [
        {
          "schema": "public",
          "name": "orders",
          "comment": "Main orders table",
          "columns": [
            { "name": "id", "dataType": "uuid", "nullable": false, "default": "gen_random_uuid()", "comment": "Order ID" }
          ],
          "primaryKey": { "columns": ["id"] },
          "foreignKeys": [ { "columns": ["user_id"], "refTable": "public.users", "refColumns": ["id"], "onDelete": "CASCADE" } ],
          "indexes": [ { "name": "orders_user_id_idx", "unique": false, "columns": ["user_id"], "include": [], "definition": "CREATE INDEX ..." } ]
        }
      ],
      "views": [ { "schema": "public", "name": "v_orders" } ]
    }
  ]
}
```

---

## Security and Performance
- Read-only: only executes introspection queries
- Caching: in-memory cache (default TTL 5 minutes), supports per-table/full refresh
- Filtering: excludes `pg_catalog` / `information_schema` by default
- Timeout/concurrency: recommended to control via connection pools and external gateways

---

## Known Limitations
- Currently only supports PostgreSQL; other databases will be added later
- `introspect_all` pagination is at the "table" granularity; routines/types/triggers summaries are appended on demand
- Complex index expressions and include columns are supported with basic parsing; edge cases with extreme dialects may exist

---

## License
Same as the repository.
