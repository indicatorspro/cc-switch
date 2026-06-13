use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
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
    pub api_key: Option<String>,
    pub env_json: Option<serde_json::Value>,
    pub auto_restart: bool,
    pub startup_timeout_ms: u64,

    // Runtime state
    status: Arc<RwLock<BackendStatus>>,
    pid: Arc<RwLock<Option<u32>>>,
    last_error: Arc<RwLock<Option<String>>>,
    child: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    #[cfg(target_os = "windows")]
    process_job: Arc<Mutex<Option<WindowsProcessJob>>>,
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
        api_key: Option<String>,
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
            api_key,
            env_json,
            auto_restart,
            startup_timeout_ms,
            status: Arc::new(RwLock::new(BackendStatus::Stopped)),
            pid: Arc::new(RwLock::new(None)),
            last_error: Arc::new(RwLock::new(None)),
            child: Arc::new(Mutex::new(None)),
            stdin: Arc::new(Mutex::new(None)),
            #[cfg(target_os = "windows")]
            process_job: Arc::new(Mutex::new(None)),
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

    pub async fn set_health_result(&self, ok: bool, message: Option<String>) {
        if ok {
            if self.running.load(Ordering::SeqCst) || self.pid().await.is_some() {
                self.set_status(BackendStatus::Running).await;
            }
            *self.last_error.write().await = None;
        } else {
            *self.last_error.write().await = message;
            if !self.running.load(Ordering::SeqCst) && self.pid().await.is_none() {
                self.set_status(BackendStatus::Error).await;
            }
        }
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
        *self.last_error.write().await = None;
        self.log(format!("Starting backend {}...", self.name)).await;

        let mut cmd = if self
            .start_args
            .as_ref()
            .map(|args| !args.is_empty())
            .unwrap_or(false)
        {
            let mut command = Command::new(&self.start_command);
            if let Some(args) = &self.start_args {
                command.args(args);
            }
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                command.creation_flags(CREATE_NO_WINDOW);
            }
            command
        } else {
            shell_command(&self.start_command)
        };

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::piped())
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
        if self.port > 0 {
            cmd.env("PORT", self.port.to_string());
        }
        if !self.host.trim().is_empty() {
            cmd.env("HOST", &self.host);
        }
        if let Some(api_key) = self.api_key.as_deref().filter(|value| !value.is_empty()) {
            cmd.env("API_KEY", api_key);
            cmd.env("OPENAI_API_KEY", api_key);
            cmd.env("ANTHROPIC_API_KEY", api_key);
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn process: {e}"))?;
        let pid = child.id();
        *self.pid.write().await = pid;
        *self.stdin.lock().await = child.stdin.take();
        self.running.store(true, Ordering::SeqCst);

        #[cfg(target_os = "windows")]
        if let Some(pid) = pid {
            match WindowsProcessJob::attach(pid) {
                Ok(job) => {
                    *self.process_job.lock().await = Some(job);
                }
                Err(err) => {
                    self.log(format!("Failed to attach process cleanup job: {err}")).await;
                }
            }
        }

        let stdout = child.stdout.take();
        if let Some(stdout) = stdout {
            spawn_stream_reader(stdout, "[out]", self.log_tx.clone(), self.running.clone());
        }

        let stderr = child.stderr.take();
        if let Some(stderr) = stderr {
            spawn_stream_reader(stderr, "[err]", self.log_tx.clone(), self.running.clone());
        }

        let port = self.port;
        let host = self.host.clone();
        let health_path = self.health_path.clone();
        let startup_timeout = self.startup_timeout_ms;
        let status = self.status.clone();
        let last_error = self.last_error.clone();
        let child_state = self.child.clone();
        let running = self.running.clone();

        *child_state.lock().await = Some(child);

        {
            let child = self.child.clone();
            let status = self.status.clone();
            let last_error = self.last_error.clone();
            let running = self.running.clone();
            let pid_ref = self.pid.clone();
            let log_tx = self.log_tx.clone();
            #[cfg(target_os = "windows")]
            let process_job = self.process_job.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(750)).await;
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }

                    let exit_status = {
                        let mut guard = child.lock().await;
                        match guard.as_mut() {
                            Some(child) => match child.try_wait() {
                                Ok(Some(status)) => {
                                    guard.take();
                                    Some(Ok(status))
                                }
                                Ok(None) => None,
                                Err(err) => {
                                    guard.take();
                                    Some(Err(err))
                                }
                            },
                            None => break,
                        }
                    };

                    match exit_status {
                        Some(Ok(exit)) => {
                            running.store(false, Ordering::SeqCst);
                            *pid_ref.write().await = None;
                            #[cfg(target_os = "windows")]
                            {
                                *process_job.lock().await = None;
                            }
                            if exit.success() {
                                *status.write().await = BackendStatus::Stopped;
                                let _ = log_tx.send(format!("Process exited: {exit}")).await;
                            } else {
                                *status.write().await = BackendStatus::Error;
                                *last_error.write().await = Some(format!("Process exited: {exit}"));
                                let _ = log_tx.send(format!("Process exited with error: {exit}")).await;
                            }
                            break;
                        }
                        Some(Err(err)) => {
                            running.store(false, Ordering::SeqCst);
                            *pid_ref.write().await = None;
                            #[cfg(target_os = "windows")]
                            {
                                *process_job.lock().await = None;
                            }
                            *status.write().await = BackendStatus::Error;
                            *last_error.write().await = Some(format!("Failed to watch process: {err}"));
                            let _ = log_tx.send(format!("Failed to watch process: {err}")).await;
                            break;
                        }
                        None => {}
                    }
                }
            });
        }

        if port == 0 || health_path.trim().is_empty() {
            self.set_status(BackendStatus::Running).await;
        } else {
            tokio::spawn(async move {
                // Wait for health check or timeout.
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
                    *status.write().await = BackendStatus::Running;
                } else {
                    *status.write().await = BackendStatus::Error;
                    *last_error.write().await =
                        Some("Health check failed or timeout".to_string());
                }
            });
        }

        self.log(format!("Backend {} started (pid: {:?})", self.name, pid)).await;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        self.set_status(BackendStatus::Stopping).await;
        self.log(format!("Stopping backend {}...", self.name)).await;
        self.running.store(false, Ordering::SeqCst);
        *self.stdin.lock().await = None;
        #[cfg(target_os = "windows")]
        {
            *self.process_job.lock().await = None;
        }

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

    pub async fn send_input(&self, input: &str) -> Result<(), String> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(format!("Backend {} is not running", self.name));
        }

        let mut guard = self.stdin.lock().await;
        let stdin = guard
            .as_mut()
            .ok_or_else(|| format!("Backend {} does not accept input", self.name))?;

        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to process stdin: {e}"))?;
        if !input.ends_with('\n') && !input.ends_with('\r') {
            #[cfg(target_os = "windows")]
            let newline = b"\r\n".as_slice();
            #[cfg(not(target_os = "windows"))]
            let newline = b"\n".as_slice();

            stdin
                .write_all(newline)
                .await
                .map_err(|e| format!("Failed to write newline to process stdin: {e}"))?;
        }
        stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush process stdin: {e}"))?;

        self.log(format!("[in] {}", input.trim_end())).await;
        Ok(())
    }

    async fn set_status(&self, status: BackendStatus) {
        *self.status.write().await = status;
    }

    async fn log(&self, message: String) {
        let _ = self.log_tx.send(message).await;
    }
}

fn spawn_stream_reader<R>(
    mut stream: R,
    prefix: &'static str,
    log_tx: Arc<mpsc::Sender<String>>,
    running: Arc<AtomicBool>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buffer = [0_u8; 1024];
        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            match stream.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buffer[..n])
                        .replace("\r\n", "\n")
                        .replace('\r', "\n");
                    if text.is_empty() {
                        continue;
                    }
                    let _ = log_tx.send(format!("{prefix} {text}")).await;
                }
                Err(err) => {
                    let _ = log_tx
                        .send(format!("{prefix} Failed to read stream: {err}"))
                        .await;
                    break;
                }
            }
        }
    });
}

#[cfg(target_os = "windows")]
struct WindowsProcessJob {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(target_os = "windows")]
unsafe impl Send for WindowsProcessJob {}

#[cfg(target_os = "windows")]
unsafe impl Sync for WindowsProcessJob {}

#[cfg(target_os = "windows")]
impl WindowsProcessJob {
    fn attach(pid: u32) -> Result<Self, String> {
        use std::mem::size_of;
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
            JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };

        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err("CreateJobObjectW failed".to_string());
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if configured == 0 {
                CloseHandle(job);
                return Err("SetInformationJobObject failed".to_string());
            }

            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process.is_null() {
                CloseHandle(job);
                return Err(format!("OpenProcess failed for pid {pid}"));
            }

            let assigned = AssignProcessToJobObject(job, process);
            CloseHandle(process);
            if assigned == 0 {
                CloseHandle(job);
                return Err(format!("AssignProcessToJobObject failed for pid {pid}"));
            }

            Ok(Self { handle: job })
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

fn shell_command(command: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", command]);
        cmd
    }
}
