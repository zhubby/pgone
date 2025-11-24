# PGone A2A Protocol Server

基于 A2A (Agent-to-Agent) 协议的 PostgreSQL Schema 查询智能体工具。

## 功能特性

- 通过 HTTP API 接收 schema 查询请求
- 支持查询任意 PostgreSQL 数据库的 schema 信息
- 支持灵活的查询选项（索引、函数、类型、触发器等）
- 返回结构化的 JSON 响应

## 快速开始

### 启动服务器

```bash
# 使用默认地址 (0.0.0.0:8080)
cargo run -p pgone-a2a

# 或指定自定义地址
PGONE_A2A_ADDR=0.0.0.0:3000 cargo run -p pgone-a2a
```

### API 使用

#### 查询 Schema

**端点**: `POST /schema/query`

**请求体**:
```json
{
  "dsn": "postgres://user:password@localhost:5432/dbname",
  "schemas": ["public"],
  "with_indexes": true,
  "with_routines": false,
  "with_types": false,
  "with_triggers": false
}
```

**请求字段说明**:
- `dsn` (必需): PostgreSQL 数据库连接字符串
- `schemas` (可选): 要查询的 schema 列表，`null` 表示查询所有
- `with_indexes` (可选, 默认 `true`): 是否包含索引信息
- `with_routines` (可选, 默认 `false`): 是否包含函数/存储过程信息
- `with_types` (可选, 默认 `false`): 是否包含类型信息
- `with_triggers` (可选, 默认 `false`): 是否包含触发器信息

**响应示例**:
```json
{
  "success": true,
  "error": null,
  "schema": {
    "database": "mydb",
    "schemas": [
      {
        "name": "public",
        "tables": [
          {
            "schema": "public",
            "name": "users",
            "comment": null,
            "columns": [
              {
                "name": "id",
                "data_type": "integer",
                "udt_name": "int4",
                "nullable": false,
                "default": "nextval('users_id_seq'::regclass)",
                "character_maximum_length": null,
                "numeric_precision": 32,
                "numeric_scale": 0,
                "comment": null
              }
            ],
            "primary_key": {
              "columns": ["id"]
            },
            "foreign_keys": [],
            "indexes": []
          }
        ],
        "views": []
      }
    ]
  }
}
```

**错误响应**:
```json
{
  "success": false,
  "error": "Failed to query schema: connection error"
}
```

## 使用示例

### 使用 curl

```bash
curl -X POST http://localhost:8080/schema/query \
  -H "Content-Type: application/json" \
  -d '{
    "dsn": "postgres://user:pass@localhost:5432/mydb",
    "schemas": ["public"],
    "with_indexes": true
  }'
```

### 使用 Python

```python
import requests
import json

url = "http://localhost:8080/schema/query"
payload = {
    "dsn": "postgres://user:pass@localhost:5432/mydb",
    "schemas": ["public"],
    "with_indexes": True
}

response = requests.post(url, json=payload)
print(json.dumps(response.json(), indent=2))
```

## 架构说明

该工具基于以下组件构建：

- **HTTP 服务器**: 使用 `axum` 框架提供 RESTful API
- **数据库连接**: 使用 `sqlx` 创建 PostgreSQL 连接池
- **Schema 查询**: 复用 `pgone-mcp-server` 中的 `PostgresIntrospector` 实现
- **协议**: 基于 JSON 的 A2A 协议消息格式

## 开发

### 构建

```bash
cargo build -p pgone-a2a
```

### 测试

```bash
cargo test -p pgone-a2a
```

### 运行

```bash
cargo run -p pgone-a2a
```

## 许可证

MIT License

