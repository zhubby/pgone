# intellid-mcp-server

一个基于 MCP（Model Context Protocol）的数据库自省服务。当前支持 PostgreSQL，功能：
- 列出/描述表、视图（含物化视图）、触发器、例程（函数/过程/聚合）、类型（枚举/域/复合）
- 输出结构化 JSON 与 LLM 友好的 Markdown 摘要
- 生成 ER 图（Mermaid）与 DBML 文本
- 多数据库：以固定 `connectionId` 管理；内置缓存与按表刷新

> 未来可扩展更多数据库，只需新增适配器实现统一自省接口。

---

## 快速开始

### MCP STDIO 模式（推荐）
1) 可选：准备 YAML 连接配置
```yaml
connections:
  - id: main
    engine: postgres
    dsn: postgres://user:pass@host:5432/dbname
```
2) 启动（在工作区根目录或本模块目录执行）
```bash
INTELLID_CONNECTIONS_PATH=/path/to/connections.yaml INTELLID_MCP_STDIO=1 cargo run -p intellid-mcp-server
```
3) 与进程通过 STDIO 交换“每行一个 JSON”的消息。

### 一次性快速自省（非 MCP）
```bash
INTELLID_PG_DSN='postgres://user:pass@host:5432/dbname' cargo run -p intellid-mcp-server
```
运行后打印数据库结构 JSON。

---

## 交互与约定
- 每条请求/响应为一行 JSON 文本。
- 请求示例：
```json
{"id":1,"method":"list_connections","params":{}}
```
- 成功响应：`{"id":1,"result":...}`；失败响应：`{"id":1,"error":{"code":...,"message":"..."}}`
- 参数命名采用 camelCase；连接以固定 `connectionId` 标识。

---

## 方法（methods）

- register_connection
  - 入参：`{ id: string, engine: 'postgres', dsn: string }`
  - 出参：`{ ok: true }`

- list_connections
  - 入参：`{}`
  - 出参：`[{ id: string, engine: 'postgres' }]`

- remove_connection
  - 入参：`{ id: string }`
  - 出参：`{ removed: boolean }`

- health_check
  - 入参：`{ id: string }`
  - 出参：`{ ok: boolean }`

- introspect_all
  - 入参：`{ connectionId: string, schemas?: string[], withIndexes?: boolean, withRoutines?: boolean, withTypes?: boolean, withTriggers?: boolean, page?: number, pageSize?: number, format?: 'markdown' }`
  - 出参：
    - Markdown：`{ markdown: string }`
    - JSON：`DatabaseSchema`（分页时附 `pageInfo: { page, pageSize, total }`）

- list_triggers
  - 入参：`{ connectionId: string, schema?: string }`
  - 出参：`TriggerDetail[]`

- list_routines
  - 入参：`{ connectionId: string, schema?: string, kind?: 'function'|'procedure'|'aggregate' }`
  - 出参：`RoutineDetail[]`

- list_types
  - 入参：`{ connectionId: string, schema?: string, kind?: 'enum'|'domain'|'composite'|'base' }`
  - 出参：`TypeDetail[]`

- get_table
  - 入参：`{ connectionId: string, schema: string, table: string, format?: 'markdown' }`
  - 出参：Markdown：`{ markdown: string }` 或 JSON：`TableDetail`

- refresh_cache
  - 入参：`{ id: string, scope?: 'table', schema?: string, table?: string }`
  - 出参：`{ refreshed: true }`

- reload_connections
  - 入参：`{ path: string }`
  - 出参：`{ ok: true }`

- render_er
  - 入参：`{ connectionId: string, schemas?: string[] }`
  - 出参：`{ mermaid: string }`

- render_dbml
  - 入参：`{ connectionId: string, schemas?: string[] }`
  - 出参：`{ dbml: string }`

---

## 示例（STDIO）

- 列出连接
```json
{"id":1,"method":"list_connections","params":{}}
```

- 健康检查
```json
{"id":2,"method":"health_check","params":{"id":"main"}}
```

- 全库 Markdown 摘要
```json
{"id":3,"method":"introspect_all","params":{"connectionId":"main","format":"markdown","withRoutines":true,"withTypes":true,"withTriggers":true}}
```

- 分页返回（JSON）
```json
{"id":4,"method":"introspect_all","params":{"connectionId":"main","page":1,"pageSize":50}}
```

- 获取单表（Markdown）
```json
{"id":5,"method":"get_table","params":{"connectionId":"main","schema":"public","table":"orders","format":"markdown"}}
```

- 生成 ER 图（Mermaid）
```json
{"id":6,"method":"render_er","params":{"connectionId":"main","schemas":["public"]}}
```

- 生成 DBML
```json
{"id":7,"method":"render_dbml","params":{"connectionId":"main","schemas":["public"]}}
```

- 按表刷新缓存
```json
{"id":8,"method":"refresh_cache","params":{"id":"main","scope":"table","schema":"public","table":"orders"}}
```

---

## 数据模型（节选）

- DatabaseSchema（缩略）
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
          "comment": "订单主表",
          "columns": [
            { "name": "id", "dataType": "uuid", "nullable": false, "default": "gen_random_uuid()", "comment": "订单ID" }
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

## 安全与性能
- 只读：仅执行自省查询
- 缓存：内存缓存（默认 TTL 5 分钟），支持按表/全量刷新
- 过滤：默认排除 `pg_catalog` / `information_schema`
- 超时/并发：建议通过连接池与外层网关控制

---

## 已知限制
- 目前仅支持 PostgreSQL；后续扩展其他数据库
- `introspect_all` 分页按“表”粒度；例程/类型/触发器摘要按需附带
- 复杂索引表达式和包含列已支持基础解析，极端方言可能存在边界

---

## 许可证
与仓库一致。
