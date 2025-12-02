use anyhow::Result;
use rmcp::model::{CallToolResult, Tool};
use rmcp::service::RoleClient;
use rmcp::transport::async_rw::AsyncRwTransport;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::RwLock;
use thiserror::Error;

/// 客户端错误类型
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Tool call error: {0}")]
    ToolCall(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<anyhow::Error> for ClientError {
    fn from(e: anyhow::Error) -> Self {
        ClientError::Transport(e.to_string())
    }
}

/// 传输方式枚举
#[derive(Clone, Debug)]
pub enum Transport {
    /// STDIO 传输方式
    Stdio,
    /// Streamable HTTP 传输方式
    StreamableHttp { url: String },
}

/// MCP 客户端
pub struct McpClient {
    transport: Transport,
    // 对于 Streamable HTTP，我们需要一个 HTTP 客户端
    #[allow(dead_code)]
    http_client: Option<reqwest::Client>,
    // 对于 STDIO，我们需要保持传输层和请求 ID 计数器
    // 注意：传输层类型取决于实际的 reader/writer 类型
    stdio_request_id: Option<Arc<RwLock<u64>>>,
}

impl McpClient {
    /// 创建新的 MCP 客户端
    pub async fn new(transport: Transport) -> Result<Self> {
        let http_client = match &transport {
            Transport::StreamableHttp { .. } => Some(reqwest::Client::new()),
            Transport::Stdio => None,
        };

        Ok(Self {
            transport,
            http_client,
            stdio_request_id: None,
        })
    }

    /// 创建新的 STDIO MCP 客户端
    pub async fn new_stdio<R, W>(reader: R, writer: W) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        // 创建传输层（暂时不保存，因为需要实现 JSON-RPC 通信）
        let _transport: AsyncRwTransport<RoleClient, R, W> = AsyncRwTransport::new_client(reader, writer);
        let request_id = Arc::new(RwLock::new(1u64));

        // TODO: 实现完整的 stdio 传输通信
        // 需要：
        // 1. 保存传输层引用
        // 2. 实现 JSON-RPC 消息发送和接收
        // 3. 实现 list_tools 和 call_tool 方法

        Ok(Self {
            transport: Transport::Stdio,
            http_client: None,
            stdio_request_id: Some(request_id),
        })
    }

    /// 列出所有可用的工具
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        match &self.transport {
            Transport::Stdio => self.list_tools_stdio().await,
            Transport::StreamableHttp { url } => self.list_tools_streamable(url).await,
        }
    }

    /// 调用指定的工具
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<CallToolResult> {
        match &self.transport {
            Transport::Stdio => self.call_tool_stdio(name, arguments).await,
            Transport::StreamableHttp { url } => self.call_tool_streamable(url, name, arguments).await,
        }
    }

    // STDIO 传输实现
    // 注意：这些方法需要实现 JSON-RPC 协议通信
    // 暂时返回错误，提示使用 Streamable HTTP
    async fn list_tools_stdio(&self) -> Result<Vec<Tool>> {
        tracing::warn!("STDIO transport for list_tools not fully implemented. Use StreamableHttp transport instead.");
        Err(anyhow::anyhow!("STDIO transport not yet implemented. Please use StreamableHttp transport."))
    }

    async fn call_tool_stdio(&self, _name: &str, _arguments: Value) -> Result<CallToolResult> {
        tracing::warn!("STDIO transport for call_tool not fully implemented");
        Err(anyhow::anyhow!("STDIO transport not yet implemented"))
    }

    // Streamable HTTP 传输实现
    async fn list_tools_streamable(&self, url: &str) -> Result<Vec<Tool>> {
        let client = self.http_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!("HTTP client not initialized")
        })?;

        // 构建请求 URL - Streamable HTTP 使用 SSE 端点
        let request_url = format!("{}/mcp", url.trim_end_matches('/'));
        
        // 创建请求体 - MCP Streamable HTTP 使用特定的格式
        // 这里我们需要发送一个初始化请求或直接调用工具列表端点
        // 暂时使用简单的 HTTP POST 请求
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        });

        // 发送 POST 请求
        let response = client
            .post(&request_url)
            .json(&request_body)
            .send()
            .await?;

        // 检查响应状态
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("HTTP error {}: {}", status, text));
        }

        // 解析响应
        let result: Value = response.json().await?;
        
        // 提取工具列表
        if let Some(tools_array) = result.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
            let mut tools = Vec::new();
            for tool_value in tools_array {
                match serde_json::from_value::<Tool>(tool_value.clone()) {
                    Ok(tool) => tools.push(tool),
                    Err(e) => {
                        tracing::warn!("Failed to parse tool: {}", e);
                        continue;
                    }
                }
            }
            Ok(tools)
        } else if let Some(error) = result.get("error") {
            Err(anyhow::anyhow!("MCP error: {}", error))
        } else {
            Err(anyhow::anyhow!("Invalid response format: {}", result))
        }
    }

    async fn call_tool_streamable(&self, url: &str, name: &str, arguments: Value) -> Result<CallToolResult> {
        let client = self.http_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!("HTTP client not initialized")
        })?;

        // 构建请求 URL
        let request_url = format!("{}/mcp", url.trim_end_matches('/'));
        
        // 将 arguments 转换为 HashMap
        let args_map = arguments.as_object()
            .ok_or_else(|| anyhow::anyhow!("Arguments must be an object"))?
            .clone();

        // 创建请求体
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": args_map
            }
        });

        // 发送 POST 请求
        let response = client
            .post(&request_url)
            .json(&request_body)
            .send()
            .await?;

        // 检查响应状态
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("HTTP error {}: {}", status, text));
        }

        // 解析响应
        let result: Value = response.json().await?;
        
        // 检查错误
        if let Some(error) = result.get("error") {
            return Err(anyhow::anyhow!("Tool call error: {}", error));
        }

        // 提取结果
        if let Some(result_data) = result.get("result") {
            serde_json::from_value(result_data.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse result: {}", e))
        } else {
            Err(anyhow::anyhow!("Invalid response format: {}", result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let transport = Transport::StreamableHttp {
            url: "http://localhost:3000".to_string(),
        };
        let client = McpClient::new(transport).await;
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_client_creation_stdio() {
        let transport = Transport::Stdio;
        let client = McpClient::new(transport).await;
        assert!(client.is_ok());
    }

    // 注意：实际的工具调用测试需要运行中的服务器
    // 这些测试可以在集成测试中实现
}
