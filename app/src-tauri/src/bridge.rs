//! IPC layer that owns the lumina-bridge child process.
//!
//! Wire protocol is newline-delimited JSON:
//!   send: {"id":"<uuid>","method":"<name>","params":{...}}
//!   recv: {"id":"<uuid>","result":...} | {"id":"<uuid>","error":"..."}
//!   recv: {"event":"<name>","data":...}                         (push)

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

type Waiters = Arc<Mutex<HashMap<String, oneshot::Sender<std::result::Result<Value, String>>>>>;

pub struct Bridge {
    stdin: Mutex<ChildStdin>,
    waiters: Waiters,
    _child: Mutex<Child>,
}

#[derive(Serialize)]
struct OutgoingRequest<'a> {
    id: &'a str,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

impl Bridge {
    pub async fn spawn(app: AppHandle, binary: PathBuf, data_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&data_dir).context("create data dir")?;

        let mut child = Command::new(&binary)
            .env("LUMINA_DATA_DIR", &data_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // bridge logs flow to the dev terminal
            .spawn()
            .with_context(|| format!("spawn bridge binary {}", binary.display()))?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;

        let waiters: Waiters = Arc::new(Mutex::new(HashMap::new()));
        let waiters_for_reader = waiters.clone();
        let app_for_reader = app.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            loop {
                match reader.next_line().await {
                    Ok(Some(line)) => {
                        if let Err(e) = route_line(&app_for_reader, &waiters_for_reader, &line).await
                        {
                            log::error!("bridge route_line error: {e:#} (line={line})");
                        }
                    }
                    Ok(None) => {
                        log::warn!("bridge stdout closed");
                        break;
                    }
                    Err(e) => {
                        log::error!("bridge stdout read error: {e}");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            stdin: Mutex::new(stdin),
            waiters,
            _child: Mutex::new(child),
        })
    }

    pub async fn call(&self, method: &str, params: Option<Value>) -> Result<Value, String> {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.waiters.lock().await.insert(id.clone(), tx);

        let req = OutgoingRequest { id: &id, method, params };
        let mut line = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        line.push(b'\n');

        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(&line).await.map_err(|e| e.to_string())?;
            stdin.flush().await.map_err(|e| e.to_string())?;
        }

        rx.await.map_err(|_| "bridge dropped response".to_string())?
    }
}

async fn route_line(app: &AppHandle, waiters: &Waiters, line: &str) -> Result<()> {
    let v: Value = serde_json::from_str(line).context("parse bridge line")?;
    if let Some(id) = v.get("id").and_then(Value::as_str) {
        let waiter = waiters.lock().await.remove(id);
        if let Some(tx) = waiter {
            if let Some(err) = v.get("error").and_then(Value::as_str) {
                let _ = tx.send(Err(err.to_string()));
            } else {
                let result = v.get("result").cloned().unwrap_or(Value::Null);
                let _ = tx.send(Ok(result));
            }
        } else {
            log::warn!("bridge response for unknown id: {id}");
        }
    } else if let Some(name) = v.get("event").and_then(Value::as_str) {
        let payload = v.get("data").cloned().unwrap_or(Value::Null);
        let event_name = format!("lumina://{name}");
        app.emit(&event_name, payload).context("emit tauri event")?;
    } else {
        log::warn!("bridge line had neither id nor event: {line}");
    }
    Ok(())
}
