use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackendKind {
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "openai_compatible")]
    OpenAICompatible,
    #[serde(rename = "anthropic")]
    Anthropic,
}

impl BackendKind {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "custom" => Ok(BackendKind::Custom),
            "openai_compatible" => Ok(BackendKind::OpenAICompatible),
            "anthropic" => Ok(BackendKind::Anthropic),
            other => Err(format!("Unknown backend kind: {other}")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BackendKind::Custom => "custom",
            BackendKind::OpenAICompatible => "openai_compatible",
            BackendKind::Anthropic => "anthropic",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackendStatus {
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "stopping")]
    Stopping,
    #[serde(rename = "error")]
    Error,
}

impl BackendStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendStatus::Stopped => "stopped",
            BackendStatus::Starting => "starting",
            BackendStatus::Running => "running",
            BackendStatus::Stopping => "stopping",
            BackendStatus::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "stopped" => BackendStatus::Stopped,
            "starting" => BackendStatus::Starting,
            "running" => BackendStatus::Running,
            "stopping" => BackendStatus::Stopping,
            "error" => BackendStatus::Error,
            _ => BackendStatus::Stopped,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackendLogLine {
    pub timestamp: std::time::SystemTime,
    pub message: String,
}

/// Runtime state for a managed backend process
pub struct ManagedBackend {
    pub id: String,
    pub name: String,
    pub kind: BackendKind,
    pub start_command: String,
    pub start_args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub host: String,
    pub port: u16,
    pub health_path: String,
    pub env_json: Option<serde_json::Value>,
    pub auto_restart: bool,
    pub startup_timeout_ms: u64,

    // Runtime state
    status: Arc<RwLock<BackendStatus>>,
    pid: Arc<RwLock<Option<u32>>>,
    last_error: Arc<RwLock<Option<String>>>,
    child: Arc<Mutex<Option<Child>>>,
    log_tx: Arc<mpsc::Sender<String>>,
    logs: Arc<Mutex<Vec<String>>>,
    running: Arc<AtomicBool>,
}

impl ManagedBackend {
    pub fn new(
        id: String,
        name: String,
        kind: BackendKind,
        start_command: String,
        start_args: Option<Vec<String>>,
        working_dir: Option<String>,
        host: String,
        port: u16,
        health_path: String,
        env_json: Option<serde_json::Value>,
        auto_restart: bool,
        startup_timeout_ms: u64,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<String>(256);
        let logs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let logs_clone = logs.clone();
        tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                logs_clone.lock().await.push(line);
                // Keep last 1000 lines
                if logs_clone.lock().await.len() > 1000 {
                    logs_clone.lock().await.remove(0);
                }
            }
        });

        Self {
            id,
            name,
            kind,
            start_command,
            start_args,
            working_dir,
            host,
            port,
            health_path,
            env_json,
            auto_restart,
            startup_timeout_ms,
            status: Arc::new(RwLock::new(BackendStatus::Stopped)),
            pid: Arc::new(RwLock::new(None)),
            last_error: Arc::new(RwLock::new(None)),
            child: Arc::new(Mutex::new(None)),
            log_tx: Arc::new(tx),
            logs,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn status(&self) -> BackendStatus {
        self.status.read().await.clone()
    }

    pub async fn pid(&self) -> Option<u32> {
        *self.pid.read().await
    }

    pub async fn last_error(&self) -> Option<String> {
        self.last_error.read().await.clone()
    }

    pub async fn logs(&self) -> Vec<String> {
        self.logs.lock().await.clone()
    }

    pub async fn start(&self) -> Result<(), String> {
        let current_status = self.status.read().await.clone();
        if matches!(current_status, BackendStatus::Running | BackendStatus::Starting) {
            return Err(format!("Backend {} is already {}", self.name, current_status.as_str()));
        }

        self.set_status(BackendStatus::Starting).await;
        self.log(format!("Starting backend {}...", self.name)).await;

        let mut cmd = Command::new(&self.start_command);
        if let Some(args) = &self.start_args {
            cmd.args(args);
        }
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        if let Some(wd) = &self.working_dir {
            cmd.current_dir(wd);
        }

        // Merge environment variables
        if let Some(env) = &self.env_json {
            if let Some(map) = env.as_object() {
                for (key, value) in map {
                    if let Some(val_str) = value.as_str() {
                        cmd.env(key, val_str);
                    }
                }
            }
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn process: {e}"))?;
        let pid = child.id();
        *self.pid.write().await = pid;
        self.running.store(true, Ordering::SeqCst);

        // Capture stdout
        let stdout = child.stdout.take();
        let log_tx = self.log_tx.clone();
        if let Some(stdout) = stdout {
            let status = self.status.clone();
            let running = self.running.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut reader = tokio::io::BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if running.load(Ordering::SeqCst) {
                        let _ = log_tx.send(format!("[out] {line}")).await;
                    } else {
                        break;
                    }
                }
            });
        }

        // Capture stderr
        let stderr = child.stderr.take();
        let log_tx = self.log_tx.clone();
        let status = self.status.clone();
        let running = self.running.clone();
        let last_error = self.last_error.clone();
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut reader = tokio::io::BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if running.load(Ordering::SeqCst) {
                        let _ = log_tx.send(format!("[err] {line}")).await;
                    } else {
                        break;
                    }
                }
            });
        }

        // Wait for startup
        let id = self.id.clone();
        let name = self.name.clone();
        let port = self.port;
        let host = self.host.clone();
        let health_path = self.health_path.clone();
        let startup_timeout = self.startup_timeout_ms;
        let status = self.status.clone();
        let last_error = self.last_error.clone();
        let child = self.child.clone();
        let running = self.running.clone();

        *child.lock().await = Some(child);

        tokio::spawn(async move {
            // Wait for health check or timeout
            let start = std::time::Instant::now();
            let health_url = format!("http://{host}:{port}{health_path}");
            let mut healthy = false;

            while start.elapsed().as_millis() < startup_timeout as u128 {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(resp) = reqwest::get(&health_url).await {
                    if resp.status().is_success() {
                        healthy = true;
                        break;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }

            if healthy {
                status.write().await = BackendStatus::Running;
            } else {
                status.write().await = BackendStatus::Error;
                last_error.write().await = Some("Health check failed or timeout".to_string());
            }
        });

        self.log(format!("Backend {} started (pid: {:?})", self.name, pid)).await;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        self.set_status(BackendStatus::Stopping).await;
        self.log(format!("Stopping backend {}...", self.name)).await;
        self.running.store(false, Ordering::SeqCst);

        if let Some(mut child) = self.child.lock().await.take() {
            if let Err(e) = child.kill().await {
                self.log(format!("Error killing process: {e}")).await;
            }
        }

        *self.pid.write().await = None;
        self.set_status(BackendStatus::Stopped).await;
        self.log(format!("Backend {} stopped", self.name)).await;
        Ok(())
    }

    async fn set_status(&self, status: BackendStatus) {
        *self.status.write().await = status;
    }

    async fn log(&self, message: String) {
        let _ = self.log_tx.send(message).await;
    }
}
