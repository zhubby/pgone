use anyhow::Result;
use rmcp::model::{CallToolResult, Tool};
use rmcp::service::RoleClient;
use rmcp::transport::async_rw::AsyncRwTransport;
use serde_json::{Value, json};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

/// Client error types
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

/// Transport type enum
#[derive(Clone, Debug)]
pub enum Transport {
    /// STDIO transport
    Stdio,
    /// Streamable HTTP transport
    StreamableHttp { url: String },
}

/// MCP client
pub struct McpClient {
    transport: Transport,
    // For Streamable HTTP, we need an HTTP client
    #[allow(dead_code)]
    http_client: Option<reqwest::Client>,
    // For STDIO, we need to hold the transport layer
    // Note: transport layer type depends on the actual reader/writer types
}

impl McpClient {
    /// Creates a new MCP client
    pub async fn new(transport: Transport) -> Result<Self> {
        let http_client = match &transport {
            Transport::StreamableHttp { .. } => Some(reqwest::Client::new()),
            Transport::Stdio => None,
        };

        Ok(Self {
            transport,
            http_client,
        })
    }

    /// Creates a new STDIO MCP client
    pub async fn new_stdio<R, W>(reader: R, writer: W) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        // Create transport layer (not saved yet, as JSON-RPC communication needs to be implemented)
        let _transport: AsyncRwTransport<RoleClient, R, W> =
            AsyncRwTransport::new_client(reader, writer);

        // TODO: Implement full stdio transport communication
        // Needs:
        // 1. Save transport layer reference
        // 2. Implement JSON-RPC message sending and receiving
        // 3. Implement list_tools and call_tool methods

        Ok(Self {
            transport: Transport::Stdio,
            http_client: None,
        })
    }

    /// Lists all available tools
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        match &self.transport {
            Transport::Stdio => self.list_tools_stdio().await,
            Transport::StreamableHttp { url } => self.list_tools_streamable(url).await,
        }
    }

    /// Calls the specified tool
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<CallToolResult> {
        match &self.transport {
            Transport::Stdio => self.call_tool_stdio(name, arguments).await,
            Transport::StreamableHttp { url } => {
                self.call_tool_streamable(url, name, arguments).await
            }
        }
    }

    // STDIO transport implementation
    // Note: these methods need JSON-RPC protocol communication implemented
    // Temporarily returns error, suggesting to use Streamable HTTP
    async fn list_tools_stdio(&self) -> Result<Vec<Tool>> {
        tracing::warn!(
            "STDIO transport for list_tools not fully implemented. Use StreamableHttp transport instead."
        );
        Err(anyhow::anyhow!(
            "STDIO transport not yet implemented. Please use StreamableHttp transport."
        ))
    }

    async fn call_tool_stdio(&self, _name: &str, _arguments: Value) -> Result<CallToolResult> {
        tracing::warn!("STDIO transport for call_tool not fully implemented");
        Err(anyhow::anyhow!("STDIO transport not yet implemented"))
    }

    // Streamable HTTP transport implementation
    async fn list_tools_streamable(&self, url: &str) -> Result<Vec<Tool>> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("HTTP client not initialized"))?;

        // Build request URL - Streamable HTTP uses SSE endpoint
        let request_url = format!("{}/mcp", url.trim_end_matches('/'));

        // Create request body - MCP Streamable HTTP uses a specific format
        // Here we need to send an init request or directly call the tool list endpoint
        // Temporarily using a simple HTTP POST request
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        });

        // Send POST request
        let response = client.post(&request_url).json(&request_body).send().await?;

        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("HTTP error {}: {}", status, text));
        }

        // Parse response
        let result: Value = response.json().await?;

        // Extract tool list
        if let Some(tools_array) = result
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
        {
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

    async fn call_tool_streamable(
        &self,
        url: &str,
        name: &str,
        arguments: Value,
    ) -> Result<CallToolResult> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("HTTP client not initialized"))?;

        // Build request URL
        let request_url = format!("{}/mcp", url.trim_end_matches('/'));

        // Convert arguments to HashMap
        let args_map = arguments
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Arguments must be an object"))?
            .clone();

        // Create request body
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": args_map
            }
        });

        // Send POST request
        let response = client.post(&request_url).json(&request_body).send().await?;

        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("HTTP error {}: {}", status, text));
        }

        // Parse response
        let result: Value = response.json().await?;

        // Check for errors
        if let Some(error) = result.get("error") {
            return Err(anyhow::anyhow!("Tool call error: {}", error));
        }

        // Extract result
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

    // Note: actual tool call tests require a running server
    // These tests can be implemented in integration tests
}
