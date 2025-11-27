## PGone — PostgreSQL One 工具集

PGone 是一套围绕数据库智能化的本地开发工具集，包含桌面 GUI、MCP Server 以及本地存储层，旨在提供：
- 会话式图形交互与 SQL 试验台
- 面向 Agent 的数据库能力暴露（MCP 协议）
- 轻量、本地可嵌入的持久化存储

---

### 架构概览
- **入口 `pgone`**：程序主入口。主线程启动 GUI，同时在后台线程启动 MCP（STDIO 或一键自省）。
- **桌面 GUI `pgone-gui`**：基于 `egui/eframe` 的桌面应用（窗口标题 "PGone GUI"）。
- **MCP Server `pgone-mcp-server`**：连接注册、适配器和 STDIO 模式；支持一键自省输出 JSON。
- **本地存储 `pgone-storage`**：基于 `libsql`（Turso）的嵌入式存储，替代历史 `sessions.json`。
- 其余：`pgone-protocol`、`pgone-a2a`、`pgone-apiserver` 为协议与扩展脚手架。

依赖关系（简化）：
- `pgone` → `pgone-gui`、`pgone-mcp-server`
- `pgone-gui` → `pgone-storage`
- `pgone-mcp-server` → SQL 适配与 MCP 协议

---

### 工作区结构
```text
pgone/                  # 程序入口（同时启动 GUI 与 MCP）
pgone-gui/              # 桌面 GUI（egui/eframe）
pgone-mcp-server/       # MCP server、连接注册、DB 适配器
pgone-storage/          # 本地嵌入式存储（libsql/Turso）
pgone-protocol/         # 协议与共享类型
pgone-a2a/              # 预留扩展
pgone-apiserver/        # 预留扩展（Web/API）
Cargo.toml                 # workspace 清单
```

---

### 快速开始
- 构建所有 crate：
```bash
cargo build --workspace
```
- 启动 GUI（后台同时启动 MCP 线程）：
```bash
cargo run -p pgone
```
- MCP STDIO 模式（仍会启动 GUI；用于 Agent 集成调试）：
```bash
PGONE_CONNECTIONS_PATH=examples/connections.yaml \
PGONE_MCP_STDIO=1 \
cargo run -p pgone
```
- 一键自省（打印数据库结构 JSON，适合快速检查）：
```bash
PGONE_PG_DSN=postgres://user:pass@host:5432/dbname \
cargo run -p pgone
```

---

### GUI 功能摘要（`pgone-gui`）
- 左侧：会话列表与 DB 配置（Engine、DSN，快速 Connect/Disconnect）
- 中间：SQL 编辑与结果表（基础高亮、CSV 导出）
- 右侧：聊天面板（Markdown/图片/视频消息渲染；计划与 OpenAI/MCP 深度整合）
- 设置：主题、发送快捷键、OpenAI Key/模型

首次运行会自动初始化本地存储（见下）。

---

### 本地存储（`pgone-storage`）
- 基于 **`libsql`（Turso）** 的嵌入式存储，默认数据库文件为 `pgone.db`（工程根目录）。
- 取代历史 `sessions.json`；首次打开 GUI 时会进行一次性迁移并删除旧文件。
- 表设计（不使用外键，由程序保证关联，已建必要索引）：
  - `db_configs(id, engine, dsn, default_schemas, include_system, created_at, updated_at)`
  - `sessions(id, title, config_id, created_at, updated_at)` 索引：`(config_id)`、`(updated_at)`
  - `messages(id, session_id, role, timestamp, kind, content_markdown, image_*, video_*)` 索引：`(session_id, timestamp)`
- API：提供异步接口与阻塞包装（`StorageBlocking`），便于 GUI 主线程使用。
- 程序侧完整性：删除配置会级联删除其会话与消息；删除会话会清理其消息。

---

### MCP Server（`pgone-mcp-server`）
- 连接注册：
  - 从 `PGONE_CONNECTIONS_PATH`（YAML）加载多个连接
  - 或通过 `PGONE_PG_DSN` 快速注册一个默认连接
- 运行模式：
  - `PGONE_MCP_STDIO=1` 时走 STDIO 模式
  - 未设置时可进行一次性数据库自省（打印 JSON 到 stdout）

---

### 配置与环境变量
- `PGONE_MCP_STDIO`：启用 MCP STDIO 模式
- `PGONE_CONNECTIONS_PATH`：连接配置 YAML 路径（用于 MCP 注册）
- `PGONE_PG_DSN`：一键自省的 DSN（Postgres 示例）
- `RUST_LOG`：日志过滤（基于 `tracing` / `tracing-subscriber`）

#### GitHub OAuth（GUI 登录，回环 + PKCE）
- 新建 GitHub OAuth App：
  - Authorization callback URL: `http://127.0.0.1:8765/oauth/github/callback`
- 设置环境变量：
  - `GITHUB_CLIENT_ID=<your_client_id>`
  - `GITHUB_CLIENT_SECRET=<your_client_secret>`
  - `OAUTH_REDIRECT=http://127.0.0.1:8765/oauth/github/callback`
- 运行 GUI：`cargo run -p pgone-gui`；首次启动会弹出 GitHub 登录窗口，授权成功后写入本地 `pgone.db` 的 `auth_users`/`auth_tokens` 表；后续启动若检测到已登录用户则不再弹窗。

---

### 开发指南
- 构建/测试/格式：
```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```
- 代码风格：Rust 2024；模块/文件 `snake_case`，类型/trait `PascalCase`，常量 `SCREAMING_SNAKE_CASE`；应用层错误优先 `anyhow::Result<T>`；库级错误用 `thiserror`；日志使用 `tracing`。
- 测试：就近单测 `mod tests {}`；需要时将集成测试置于 crate `tests/`；命名强调行为（例如 `test_parses_triggers_markdown`）。

---

### 迁移说明（sessions.json → pgone.db）
- GUI 启动时若检测到工程根的 `sessions.json`，会：
  1. 初始化 `pgone.db`
  2. 将会话与消息按类型（Markdown/图片/视频）迁移至新库
  3. 删除旧文件 `sessions.json`
- 迁移失败不会阻止 GUI 启动；请关注日志/错误提示。

---

### 安全与配置建议
- 不要在仓库提交任何密钥/DSN；通过环境变量或本地 YAML（示例：`examples/connections.yaml`）管理。
- 日志中避免输出敏感信息（连接串、凭据）。
- 长运行场景优先 STDIO 模式，便于与外部 Agent 整合。

---

### 路线与后续
- GUI 与 MCP 更紧密的交互桥接
- 完整将 GUI 读路径切换到存储层
- 增补更多数据库引擎适配与查询能力
- 完善单元测试与文档

欢迎提交 Issue/PR（建议使用 Conventional Commits）。


### 打包可执行程序
`cargo install cargo-bundle`
`cargo bundle --release`