use crate::futures;
use anyhow::Result;
use pgone_mcp::McpClient;
use rmcp::model::{CallToolResult, Tool};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{info, warn};

/// MCP client manager
/// Responsible for managing the MCP server child process and client connection
pub struct McpClientManager {
    client: Option<McpClient>,
    child: Option<tokio::process::Child>,
    storage_path: PathBuf,
}

impl McpClientManager {
    /// Create a new MCP client manager and start the stdio server
    pub async fn new(storage_path: PathBuf) -> Result<Self> {
        info!("Starting MCP server (stdio mode)...");

        // Find pgone-mcp-server executable path
        let server_path = Self::find_server_executable()?;
        info!("MCP server path: {}", server_path.display());

        // Start child process
        let mut child = Command::new(&server_path)
            .env("PGONE_MCP_STDIO", "1")
            .env("RUST_LOG", "info")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start MCP server: {}", e))?;

        // Get stdin/stdout
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get child process stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get child process stdout"))?;

        // Convert ChildStdin/ChildStdout to AsyncRead/AsyncWrite
        // Note: ChildStdin implements AsyncWrite, ChildStdout implements AsyncRead
        // We need to swap them: stdin writes to the child process, stdout reads from it
        let reader = tokio::io::BufReader::new(stdout);
        let writer = stdin;

        // Create MCP client
        let client = McpClient::new_stdio(reader, writer).await?;

        info!("MCP client initialized successfully");

        Ok(Self {
            client: Some(client),
            child: Some(child),
            storage_path,
        })
    }

    /// Find pgone-mcp-server executable
    fn find_server_executable() -> Result<PathBuf> {
        // First try to get from environment variable
        if let Ok(path) = std::env::var("PGONE_MCP_SERVER_PATH") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Ok(path);
            }
        }

        // Try to find from the current executable's directory
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // On macOS, the executable may be in the Contents/MacOS directory
                // Try multiple possible locations
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

        // Finally try using which/where command to find it
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

        // If none found, try cargo run approach (development environment)
        // In this case, we need to use a different method
        Err(anyhow::anyhow!(
            "Cannot find pgone-mcp-server executable. Please set the PGONE_MCP_SERVER_PATH environment variable"
        ))
    }

    /// List all available tools
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        if let Some(ref client) = self.client {
            client.list_tools().await
        } else {
            Err(anyhow::anyhow!("MCP client not initialized"))
        }
    }

    /// Call the specified tool
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<CallToolResult> {
        if let Some(ref client) = self.client {
            client.call_tool(name, arguments).await
        } else {
            Err(anyhow::anyhow!("MCP client not initialized"))
        }
    }

    /// Check if the client is available
    pub fn is_available(&self) -> bool {
        self.client.is_some()
    }

    /// Get the storage path
    pub fn storage_path(&self) -> &PathBuf {
        &self.storage_path
    }
}

impl Drop for McpClientManager {
    fn drop(&mut self) {
        // Release MCP client first, closing the stdio pipes and giving the child process a chance to exit gracefully.
        self.client.take();

        // Shut down the child process
        if let Some(mut child) = self.child.take() {
            info!("Shutting down MCP server child process...");
            let _ = futures::block_on_async(async {
                match tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await {
                    Ok(Ok(status)) => {
                        info!("MCP server child process exited, status: {:?}", status);
                    }
                    Ok(Err(e)) => {
                        warn!("Error waiting for MCP server child process to exit: {}", e);
                    }
                    Err(_) => {
                        warn!(
                            "Timeout waiting for MCP server child process graceful exit, forcing kill"
                        );
                        if let Err(e) = child.kill().await {
                            warn!("Failed to kill MCP server child process: {}", e);
                            return;
                        }

                        match tokio::time::timeout(std::time::Duration::from_secs(2), child.wait())
                            .await
                        {
                            Ok(Ok(status)) => {
                                info!(
                                    "MCP server child process forcefully exited, status: {:?}",
                                    status
                                );
                            }
                            Ok(Err(e)) => {
                                warn!(
                                    "Error waiting for MCP server child process forceful exit: {}",
                                    e
                                );
                            }
                            Err(_) => {
                                warn!("Timeout waiting for MCP server child process forceful exit");
                            }
                        }
                    }
                }
            });
        }
    }
}
