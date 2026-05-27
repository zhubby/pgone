SHELL := /bin/bash

APP_NAME := PGone
MACOS_TARGET := aarch64-apple-darwin
VERSION := $(shell awk -F' *= *' '/^version = / {gsub(/"/,"",$$2); print $$2; exit}' Cargo.toml)
DIST_DIR := dist/macos
APP_DIR := $(DIST_DIR)/$(APP_NAME).app
DMG_NAME := $(APP_NAME)-$(VERSION)-$(MACOS_TARGET).dmg
DMG_PATH := $(DIST_DIR)/$(DMG_NAME)

.PHONY: help build test fmt clippy clean run-gui run-mcp-server install-tools build-macos-app package-macos-dmg clean-macos-artifacts

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
	@echo "  make install-tools   - 安装开发工具 (cargo-bundle 等)"
	@echo "  make package-macos-dmg - 打包 macOS DMG"
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
	cargo run -p pgone-cli -- gui

# 运行 MCP Server (STDIO 模式)
run-mcp-server:
	cargo run -p pgone-cli -- mcp-server --dbconfig-id default --protocol stdio

# 安装开发工具
install-tools:
	cargo install cargo-bundle

# 打包可执行程序
bundle:
	cargo bundle --release

build-macos-app:
	cargo build --release -p pgone-gui --target $(MACOS_TARGET)
	./scripts/macos/build_app.sh \
		--target $(MACOS_TARGET) \
		--version $(VERSION) \
		--output-dir $(DIST_DIR)

package-macos-dmg: build-macos-app
	./scripts/macos/package_dmg.sh \
		--app-path $(APP_DIR) \
		--output-path $(DMG_PATH)

clean-macos-artifacts:
	rm -rf $(DIST_DIR)

# 组合命令：格式化 + clippy
lint: fmt clippy

# 组合命令：格式化 + clippy + 测试
check: fmt clippy test
