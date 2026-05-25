use crate::futures;
use anyhow::Result;
use pgone_mcp::McpClient;
use rmcp::model::{CallToolResult, Tool};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{info, warn};

/// MCP 客户端管理器
/// 负责管理 MCP server 子进程和客户端连接
pub struct McpClientManager {
    client: Option<McpClient>,
    child: Option<tokio::process::Child>,
    storage_path: PathBuf,
}

impl McpClientManager {
    /// 创建新的 MCP 客户端管理器并启动 stdio 服务器
    pub async fn new(storage_path: PathBuf) -> Result<Self> {
        info!("启动 MCP server (stdio 模式)...");

        // 查找 pgone-mcp-server 可执行文件路径
        let server_path = Self::find_server_executable()?;
        info!("MCP server 路径: {}", server_path.display());

        // 启动子进程
        let mut child = Command::new(&server_path)
            .env("PGONE_MCP_STDIO", "1")
            .env("RUST_LOG", "info")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("启动 MCP server 失败: {}", e))?;

        // 获取 stdin/stdout
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("无法获取子进程 stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("无法获取子进程 stdout"))?;

        // 将 ChildStdin/ChildStdout 转换为 AsyncRead/AsyncWrite
        // 注意：ChildStdin 实现了 AsyncWrite，ChildStdout 实现了 AsyncRead
        // 但我们需要交换它们：stdin 是写入到子进程，stdout 是从子进程读取
        let reader = tokio::io::BufReader::new(stdout);
        let writer = stdin;

        // 创建 MCP 客户端
        let client = McpClient::new_stdio(reader, writer).await?;

        info!("MCP client 初始化成功");

        Ok(Self {
            client: Some(client),
            child: Some(child),
            storage_path,
        })
    }

    /// 查找 pgone-mcp-server 可执行文件
    fn find_server_executable() -> Result<PathBuf> {
        // 首先尝试从环境变量获取
        if let Ok(path) = std::env::var("PGONE_MCP_SERVER_PATH") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Ok(path);
            }
        }

        // 尝试从当前可执行文件所在目录查找
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // 在 macOS 上，可执行文件可能在 Contents/MacOS 目录下
                // 尝试多个可能的位置
                let possible_paths = vec![
                    exe_dir.join("pgone-mcp-server"),
                    exe_dir.join("../pgone-mcp-server"),
                    exe_dir.join("../../pgone-mcp-server"),
                    exe_dir.join("../../../pgone-mcp-server"),
                ];

                for path in possible_paths {
                    if path.exists() {
                        return Ok(path);
                    }
                }
            }
        }

        // 最后尝试使用 which/where 命令查找
        let which_cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };

        if let Ok(output) = std::process::Command::new(which_cmd)
            .arg("pgone-mcp-server")
            .output()
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    return Ok(PathBuf::from(path_str));
                }
            }
        }

        // 如果都找不到，尝试使用 cargo run 的方式（开发环境）
        // 这种情况下，我们需要使用不同的方法
        Err(anyhow::anyhow!(
            "无法找到 pgone-mcp-server 可执行文件。请设置 PGONE_MCP_SERVER_PATH 环境变量"
        ))
    }

    /// 列出所有可用的工具
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        if let Some(ref client) = self.client {
            client.list_tools().await
        } else {
            Err(anyhow::anyhow!("MCP client 未初始化"))
        }
    }

    /// 调用指定的工具
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<CallToolResult> {
        if let Some(ref client) = self.client {
            client.call_tool(name, arguments).await
        } else {
            Err(anyhow::anyhow!("MCP client 未初始化"))
        }
    }

    /// 检查客户端是否可用
    pub fn is_available(&self) -> bool {
        self.client.is_some()
    }

    /// 获取 storage 路径
    pub fn storage_path(&self) -> &PathBuf {
        &self.storage_path
    }
}

impl Drop for McpClientManager {
    fn drop(&mut self) {
        // 先释放 MCP client，让 stdio 管道关闭并给子进程一个正常退出机会。
        self.client.take();

        // 关闭子进程
        if let Some(mut child) = self.child.take() {
            info!("正在关闭 MCP server 子进程...");
            let _ = futures::block_on_async(async {
                match tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await {
                    Ok(Ok(status)) => {
                        info!("MCP server 子进程已退出，状态: {:?}", status);
                    }
                    Ok(Err(e)) => {
                        warn!("等待 MCP server 子进程退出时出错: {}", e);
                    }
                    Err(_) => {
                        warn!("等待 MCP server 子进程优雅退出超时，开始强制关闭");
                        if let Err(e) = child.kill().await {
                            warn!("关闭 MCP server 子进程失败: {}", e);
                            return;
                        }

                        match tokio::time::timeout(std::time::Duration::from_secs(2), child.wait())
                            .await
                        {
                            Ok(Ok(status)) => {
                                info!("MCP server 子进程已强制退出，状态: {:?}", status);
                            }
                            Ok(Err(e)) => {
                                warn!("等待 MCP server 子进程强制退出时出错: {}", e);
                            }
                            Err(_) => {
                                warn!("等待 MCP server 子进程强制退出超时");
                            }
                        }
                    }
                }
            });
        }
    }
}
