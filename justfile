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
    cargo run -p pgone-cli -- gui

# 运行 MCP Server (STDIO 模式)
run-mcp-server dbconfig_id protocol="stdio":
    cargo run -p pgone-cli -- mcp-server --dbconfig-id {{dbconfig_id}} --protocol {{protocol}}

# 安装开发工具
install-tools:
    cargo install cargo-bundle

# 打包可执行程序
bundle:
    cargo bundle --release

# 构建 macOS app bundle
build-macos-app:
    cargo build --release -p pgone-gui --target aarch64-apple-darwin
    ./scripts/macos/build_app.sh --target aarch64-apple-darwin --version $(awk -F' *= *' '/^version = / {gsub(/"/,"",$2); print $2; exit}' Cargo.toml) --output-dir dist/macos

# 打包 macOS DMG
package-macos-dmg:
    just build-macos-app
    ./scripts/macos/package_dmg.sh --app-path dist/macos/PGone.app --output-path dist/macos/PGone-$(awk -F' *= *' '/^version = / {gsub(/"/,"",$2); print $2; exit}' Cargo.toml)-aarch64-apple-darwin.dmg

# 清理 macOS 打包产物
clean-macos-artifacts:
    rm -rf dist/macos

# 组合命令：格式化 + clippy
lint:
    just fmt
    just clippy

# 组合命令：格式化 + clippy + 测试
check:
    just fmt
    just clippy
    just test

# 快速启动 MCP（使用本地存储中的数据库配置 ID）
introspect dbconfig_id:
    cargo run -p pgone-cli -- mcp-server --dbconfig-id {{dbconfig_id}}

# 运行 MCP Server 并启用 STDIO（使用默认连接配置）
mcp-stdio dbconfig_id:
    cargo run -p pgone-cli -- mcp-server --dbconfig-id {{dbconfig_id}} --protocol stdio
