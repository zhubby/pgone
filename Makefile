.PHONY: help build test fmt clippy clean run-gui run-mcp-server run-auditor run-apiserver run-a2a install-tools

# 默认目标
help:
	@echo "PGone 项目构建工具"
	@echo ""
	@echo "可用命令:"
	@echo "  make build          - 构建整个 workspace"
	@echo "  make test           - 运行所有测试"
	@echo "  make fmt            - 格式化代码"
	@echo "  make clippy         - 运行 clippy 检查"
	@echo "  make clean           - 清理构建产物"
	@echo "  make run-gui         - 运行 GUI 应用"
	@echo "  make run-mcp-server  - 运行 MCP Server (STDIO 模式)"
	@echo "  make run-auditor     - 运行 Auditor"
	@echo "  make run-apiserver   - 运行 API Server"
	@echo "  make run-a2a         - 运行 A2A"
	@echo "  make install-tools   - 安装开发工具 (cargo-bundle 等)"
	@echo "  make lint            - 运行 fmt 和 clippy"
	@echo "  make check           - 运行 fmt, clippy 和 test"

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
run-mcp-server:
	PGONE_CONNECTIONS_PATH=pgone-mcp-server/examples/connections.yaml \
	PGONE_MCP_STDIO=1 \
	cargo run -p pgone-mcp-server

# 运行 Auditor
run-auditor:
	cargo run -p pgone-auditor

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
lint: fmt clippy

# 组合命令：格式化 + clippy + 测试
check: fmt clippy test

