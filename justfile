# PGone 项目构建工具 (Justfile)

# 默认目标：显示帮助信息
default:
    @just --list

# 构建整个 workspace
build:
    cargo build --workspace

# 构建 release 版本
build-release:
    cargo build --workspace --release

# 运行所有测试
test:
    cargo test --workspace

# 格式化代码
fmt:
    cargo fmt --all

# 运行 clippy 检查
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# 清理构建产物
clean:
    cargo clean

# 运行 GUI 应用
run-gui:
    cargo run -p pgone-gui

# 运行 MCP Server (STDIO 模式)
run-mcp-server connections_path="pgone-mcp-server/examples/connections.yaml":
    PGONE_CONNECTIONS_PATH={{connections_path}} \
    PGONE_MCP_STDIO=1 \
    cargo run -p pgone-mcp-server

# 运行 Proxy
run-proxy:
    cargo run -p pgone-proxy

# 运行 API Server
run-apiserver:
    cargo run -p pgone-apiserver

# 运行 A2A
run-a2a:
    cargo run -p pgone-a2a

# 安装开发工具
install-tools:
    cargo install cargo-bundle

# 打包可执行程序
bundle:
    cargo bundle --release

# 组合命令：格式化 + clippy
lint:
    just fmt
    just clippy

# 组合命令：格式化 + clippy + 测试
check:
    just fmt
    just clippy
    just test

# 快速自省（使用环境变量中的 DSN）
introspect dsn:
    PGONE_PG_DSN={{dsn}} cargo run -p pgone

# 运行 MCP Server 并启用 STDIO（使用默认连接配置）
mcp-stdio:
    PGONE_CONNECTIONS_PATH=pgone-mcp-server/examples/connections.yaml \
    PGONE_MCP_STDIO=1 \
    cargo run -p pgone-mcp-server

