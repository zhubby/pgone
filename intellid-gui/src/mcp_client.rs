use serde_json::{Value, json};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};

pub struct McpClient {
    #[allow(dead_code)]
    child: Child,
    tx: mpsc::Sender<(u64, String)>,
    waiters: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    seq: AtomicU64,
}

impl McpClient {
    pub async fn spawn_with_default() -> anyhow::Result<Self> {
        // 默认通过 cargo 运行本地 mcp-server（开发环境）
        let mut cmd = Command::new("cargo");
        cmd.arg("run").arg("-p").arg("intellid-mcp-server");
        cmd.env("INTELLID_MCP_STDIO", "1");
        cmd.env("INTELLID_CONNECTIONS_PATH", "examples/connections.yaml");
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");

        let (tx, mut rx) = mpsc::channel::<(u64, String)>(64);
        let waiters: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let waiters_writer = waiters.clone();
        let mut writer = tokio::io::BufWriter::new(stdin);
        tokio::spawn(async move {
            while let Some((id, line)) = rx.recv().await {
                let _ = writer.write_all(line.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
                let _ = writer.flush().await;
                // Note: request is sent; response will be handled by reader
                // nothing to do here
                let _ = id;
            }
        });

        let waiters_reader = waiters.clone();
        let mut reader = tokio::io::BufReader::new(stdout).lines();
        tokio::spawn(async move {
            while let Ok(Some(line)) = reader.next_line().await {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    let id_opt = v.get("id").cloned();
                    if let Some(idv) = id_opt
                        && let Some(id) = idv.as_i64().or_else(|| idv.as_u64().map(|u| u as i64))
                        && let Some(tx) = waiters_reader.lock().await.remove(&(id as u64))
                    {
                        let _ = tx.send(v);
                    }
                }
            }
        });

        Ok(Self {
            child,
            tx,
            waiters: waiters_writer,
            seq: AtomicU64::new(1),
        })
    }

    pub async fn call(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        let id = self.seq.fetch_add(1, Ordering::Relaxed);
        let (tx_resp, rx_resp) = oneshot::channel();
        self.waiters.lock().await.insert(id, tx_resp);
        let req = json!({"id": id, "method": method, "params": params});
        let line = serde_json::to_string(&req)?;
        let _ = self.tx.send((id, line)).await;
        let v = rx_resp.await?;
        if let Some(err) = v.get("error") {
            anyhow::bail!(format!("mcp error: {}", err));
        }
        Ok(v.get("result").cloned().unwrap_or(json!(null)))
    }
}
