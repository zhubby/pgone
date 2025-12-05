# pgone-proxy 示例

本目录包含 pgone-proxy 的使用示例。

## simple_query.rs

一个简单的示例，展示如何连接到 pgone-proxy 代理服务器并发送 SQL 查询。

### 功能说明

- 连接到代理服务器（默认监听在 `127.0.0.1:5432`）
- 发送包含多行注释 YAML 配置的 SQL 查询
- 展示三种不同的查询场景：
  1. 简单的 SELECT 查询（查询 PostgreSQL 版本）
  2. 查询当前时间
  3. 带 SSL 配置的查询示例

### 使用方法

1. **启动代理服务器**（在一个终端中）：
```bash
# 使用默认地址和端口 (127.0.0.1:5432)
cargo run -p pgone-proxy

# 指定自定义地址和端口
cargo run -p pgone-proxy -- --bind-address 0.0.0.0 --port 5433

# 查看所有可用选项
cargo run -p pgone-proxy -- --help
```

2. **运行示例**（在另一个终端中）：
```bash
# 如果使用默认端口
cargo run --example simple_query -p pgone-proxy

# 如果使用自定义端口，需要修改示例中的 proxy_dsn
```

### SQL 查询格式

所有 SQL 查询必须使用多行注释 YAML 格式指定后端数据库连接信息：

```sql
/*
dsn: postgres://user:password@host:port/database
ssl:
  mode: require
*/
SELECT * FROM table;
```

### 配置说明

- `dsn`: 必需，后端 PostgreSQL 数据库的连接字符串
- `ssl`: 可选，SSL/TLS 配置
  - `cert`: SSL 证书文件路径（可选）
  - `key`: SSL 私钥文件路径（可选）
  - `ca`: CA 证书文件路径（可选）
  - `mode`: SSL 模式，可选值：`disable`, `allow`, `prefer`, `require`, `verify-ca`, `verify-full`（默认：`prefer`）

### 命令行参数

代理服务器支持以下命令行参数：

- `--bind-address <ADDRESS>`: 服务器绑定地址（默认：`127.0.0.1`）
- `-p, --port <PORT>`: 服务器监听端口（默认：`5432`）
- `-l, --log-level <LEVEL>`: 日志级别（默认：`info`）
- `--enable-otel`: 启用 OpenTelemetry 追踪
- `--json-log`: 使用 JSON 格式输出日志
- `--service-name <NAME>`: 服务名称（默认：`pgone-proxy`）

### 注意事项

- 确保后端 PostgreSQL 数据库正在运行并可访问
- 根据实际情况修改示例中的 DSN 连接字符串
- 代理服务器默认监听在 `127.0.0.1:5432`，确保该端口未被占用
- 如果使用自定义端口启动服务器，记得修改客户端连接字符串中的端口号

