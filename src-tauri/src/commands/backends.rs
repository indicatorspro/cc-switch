use crate::services::backend_registry::BackendRegistry;
use crate::services::backend_runtime::{BackendKind, BackendStatus};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBackendRequest {
    pub name: String,
    pub kind: String,
    pub start_command: String,
    pub start_args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub health_path: Option<String>,
    pub api_key: Option<String>,
    pub env_json: Option<serde_json::Value>,
    pub auto_restart: Option<bool>,
    pub startup_timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateBackendRequest {
    pub name: Option<String>,
    pub start_command: Option<String>,
    pub start_args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub health_path: Option<String>,
    pub api_key: Option<String>,
    pub env_json: Option<serde_json::Value>,
    pub auto_restart: Option<bool>,
    pub startup_timeout_ms: Option<u64>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BackendInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub managed: bool,
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
    pub status: String,
    pub pid: Option<u32>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct BackendHealthResult {
    pub ok: bool,
    pub url: String,
    pub status: Option<u16>,
    pub latency_ms: u128,
    pub message: String,
    pub backend: BackendInfo,
}

#[derive(Debug, Serialize)]
pub struct BackendModelsResult {
    pub url: String,
    pub models: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BackendEnvFile {
    pub path: String,
    pub exists: bool,
    pub content: String,
}

#[tauri::command]
pub async fn list_backends(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<BackendInfo>, String> {
    let db = state.db.clone();
    let registry = BackendRegistry::get_or_init(db.clone());
    let backends = db.get_all_backends().map_err(|e| e.to_string())?;
    let mut result = Vec::with_capacity(backends.len());
    for backend in backends {
        let runtime = registry.runtime_state(&backend.id).await;
        result.push(BackendInfo::from_row_with_runtime(
            backend,
            runtime,
        ));
    }
    Ok(result)
}

#[tauri::command]
pub async fn get_backend(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<BackendInfo, String> {
    let db = state.db.clone();
    let backend = db.get_backend(&id).map_err(|e| e.to_string())?;
    let registry = BackendRegistry::get_or_init(db.clone());
    let runtime = registry.runtime_state(&id).await;
    Ok(BackendInfo::from_row_with_runtime(backend, runtime))
}

#[tauri::command]
pub async fn create_backend(
    req: CreateBackendRequest,
    state: tauri::State<'_, AppState>,
) -> Result<BackendInfo, String> {
    let db = state.db.clone();
    let kind = BackendKind::from_str(&req.kind)
        .map_err(|e| format!("Invalid backend kind: {e}"))?;
    let id = db.insert_backend(&req.name, kind.as_str(),
            &req.start_command,
            req.start_args.as_ref(),
            req.working_dir.as_deref(),
            req.host.as_deref().unwrap_or("127.0.0.1"),
            req.port.unwrap_or(0),
            req.health_path.as_deref().unwrap_or(""),
            clean_optional_string(req.api_key.as_deref()).as_deref(),
            req.env_json.as_ref(),
            req.auto_restart.unwrap_or(false),
            req.startup_timeout_ms.unwrap_or(10000),
        )
        .map_err(|e| e.to_string())?;
    let backend = db.get_backend(&id).map_err(|e| e.to_string())?;
    Ok(BackendInfo::from_row(backend))
}

#[tauri::command]
pub async fn update_backend(
    id: String,
    req: UpdateBackendRequest,
    state: tauri::State<'_, AppState>,
) -> Result<BackendInfo, String> {
    let db = state.db.clone();
    let registry = BackendRegistry::get_or_init(db.clone());
    if registry.runtime_state(&id).await.is_some() {
        let _ = registry.stop(&id).await;
        registry.forget(&id).await;
    }
    db.update_backend(
        &id,
        req.name.as_deref(),
        req.start_command.as_deref(),
        req.start_args.as_ref(),
        req.working_dir.as_deref(),
        req.host.as_deref(),
        req.port,
        req.health_path.as_deref(),
        req.api_key.as_deref(),
        req.env_json.as_ref(),
        req.auto_restart,
        req.startup_timeout_ms,
        req.enabled,
    )
    .map_err(|e| e.to_string())?;
    let backend = db.get_backend(&id).map_err(|e| e.to_string())?;
    let runtime = registry.runtime_state(&id).await;
    Ok(BackendInfo::from_row_with_runtime(backend, runtime))
}

#[tauri::command]
pub async fn delete_backend(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.clone();
    let registry = BackendRegistry::get_or_init(db.clone());
    let _ = registry.stop(&id).await;
    registry.forget(&id).await;
    db.delete_backend(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_backend(
    id: String,
    _app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<BackendInfo, String> {
    let db = state.db.clone();
    let registry = BackendRegistry::get_or_init(db.clone());
    registry.start(&id).await.map_err(|e| e.to_string())?;
    let updated = db.get_backend(&id).map_err(|e| e.to_string())?;
    let runtime = registry.runtime_state(&id).await;
    Ok(BackendInfo::from_row_with_runtime(updated, runtime))
}

#[tauri::command]
pub async fn stop_backend(
    id: String,
    _app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<BackendInfo, String> {
    let db = state.db.clone();
    let registry = BackendRegistry::get_or_init(db.clone());
    registry.stop(&id).await.map_err(|e| e.to_string())?;
    let updated = db.get_backend(&id).map_err(|e| e.to_string())?;
    let runtime = registry.runtime_state(&id).await;
    Ok(BackendInfo::from_row_with_runtime(updated, runtime))
}

#[tauri::command]
pub async fn restart_backend(
    id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<BackendInfo, String> {
    let _ = stop_backend(id.clone(), app.clone(), state.clone()).await;
    start_backend(id, app, state).await
}

#[tauri::command]
pub async fn get_backend_logs(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let db = state.db.clone();
    let registry = BackendRegistry::get_or_init(db.clone());
    registry.get_logs(&id).await
}

#[tauri::command]
pub async fn send_backend_input(
    id: String,
    input: String,
    state: tauri::State<'_, AppState>,
) -> Result<BackendInfo, String> {
    let db = state.db.clone();
    let registry = BackendRegistry::get_or_init(db.clone());
    registry.send_input(&id, &input).await?;
    let backend = db.get_backend(&id).map_err(|e| e.to_string())?;
    let runtime = registry.runtime_state(&id).await;
    Ok(BackendInfo::from_row_with_runtime(backend, runtime))
}

#[tauri::command]
pub async fn check_backend_health(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<BackendHealthResult, String> {
    let db = state.db.clone();
    let registry = BackendRegistry::get_or_init(db.clone());
    let backend = db.get_backend(&id).map_err(|e| e.to_string())?;
    let url = backend_health_url(&backend)?;
    let start = Instant::now();
    let response = match request_builder(&backend, &url).send().await {
        Ok(response) => response,
        Err(err) => {
            let message = format!("Health check failed: {err}");
            registry
                .record_health_result(&id, false, Some(message.clone()))
                .await?;
            let updated = db.get_backend(&id).map_err(|e| e.to_string())?;
            let runtime = registry.runtime_state(&id).await;
            return Ok(BackendHealthResult {
                ok: false,
                url,
                status: None,
                latency_ms: start.elapsed().as_millis(),
                message,
                backend: BackendInfo::from_row_with_runtime(updated, runtime),
            });
        }
    };
    let status = response.status();
    let message = if status.is_success() {
        format!("Health check OK ({status})")
    } else {
        let body = response.text().await.unwrap_or_default();
        if body.trim().is_empty() {
            format!("Health check failed ({status})")
        } else {
            format!("Health check failed ({status}): {}", trim_for_display(&body, 240))
        }
    };
    let ok = status.is_success();
    registry
        .record_health_result(&id, ok, (!ok).then(|| message.clone()))
        .await?;
    let updated = db.get_backend(&id).map_err(|e| e.to_string())?;
    let runtime = registry.runtime_state(&id).await;

    Ok(BackendHealthResult {
        ok,
        url,
        status: Some(status.as_u16()),
        latency_ms: start.elapsed().as_millis(),
        message,
        backend: BackendInfo::from_row_with_runtime(updated, runtime),
    })
}

#[tauri::command]
pub async fn list_backend_models(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<BackendModelsResult, String> {
    let backend = state.db.get_backend(&id).map_err(|e| e.to_string())?;
    if backend.port == 0 {
        return Err("Port is required to list models".to_string());
    }
    let url = format!("http://{}:{}/v1/models", backend.host, backend.port);
    let response = request_builder(&backend, &url)
        .send()
        .await
        .map_err(|e| format!("Failed to list models: {e}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read models response: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "Models request failed ({status}): {}",
            trim_for_display(&body, 240)
        ));
    }
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Invalid models JSON: {e}"))?;
    Ok(BackendModelsResult {
        url,
        models: extract_model_ids(&value),
    })
}

#[tauri::command]
pub async fn read_backend_env_file(working_dir: String) -> Result<BackendEnvFile, String> {
    let path = env_file_path(&working_dir)?;
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(BackendEnvFile {
            path: path.display().to_string(),
            exists: true,
            content,
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(BackendEnvFile {
            path: path.display().to_string(),
            exists: false,
            content: String::new(),
        }),
        Err(err) => Err(format!("Failed to read .env: {err}")),
    }
}

#[tauri::command]
pub async fn write_backend_env_file(
    working_dir: String,
    content: String,
) -> Result<BackendEnvFile, String> {
    let path = env_file_path(&working_dir)?;
    std::fs::write(&path, content).map_err(|err| format!("Failed to write .env: {err}"))?;
    read_backend_env_file(working_dir).await
}

impl BackendInfo {
    fn from_row(b: crate::database::ManagedBackend) -> Self {
        Self::from_row_with_runtime(b, None)
    }

    fn from_row_with_runtime(
        b: crate::database::ManagedBackend,
        runtime: Option<(BackendStatus, Option<u32>, Option<String>)>,
    ) -> Self {
        let start_args = b
            .start_args
            .as_deref()
            .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok());
        let env_json = b
            .env_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok());
        let (status, pid, last_error) = runtime
            .map(|(status, pid, last_error)| (status.as_str().to_string(), pid, last_error))
            .unwrap_or_else(|| (BackendStatus::Stopped.as_str().to_string(), None, b.last_error));

        Self {
            id: b.id,
            name: b.name,
            kind: b.kind,
            enabled: b.enabled,
            managed: b.managed,
            start_command: b.start_command,
            start_args,
            working_dir: b.working_dir,
            host: b.host,
            port: b.port,
            health_path: b.health_path,
            api_key: b.api_key,
            env_json,
            auto_restart: b.auto_restart,
            startup_timeout_ms: b.startup_timeout_ms,
            status,
            pid,
            last_error,
            created_at: b.created_at,
            updated_at: b.updated_at,
        }
    }
}

fn backend_health_url(backend: &crate::database::ManagedBackend) -> Result<String, String> {
    if backend.port == 0 {
        return Err("Port is required to run a health check".to_string());
    }
    let path = if backend.health_path.trim().is_empty() {
        "/"
    } else {
        backend.health_path.as_str()
    };
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    Ok(format!("http://{}:{}{}", backend.host, backend.port, path))
}

fn request_builder(backend: &crate::database::ManagedBackend, url: &str) -> reqwest::RequestBuilder {
    let client = reqwest::Client::new();
    let mut request = client.get(url);
    if let Some(api_key) = backend.api_key.as_deref().filter(|value| !value.is_empty()) {
        request = request.bearer_auth(api_key);
    }
    request
}

fn extract_model_ids(value: &serde_json::Value) -> Vec<String> {
    let source = value
        .get("data")
        .and_then(|data| data.as_array())
        .or_else(|| value.as_array());

    let mut models = source
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get("id")
                .and_then(|id| id.as_str())
                .or_else(|| item.as_str())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    models
}

fn clean_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn trim_for_display(value: &str, max_len: usize) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..max_len])
    }
}

fn env_file_path(working_dir: &str) -> Result<PathBuf, String> {
    let trimmed = working_dir.trim();
    if trimmed.is_empty() {
        return Err("Working directory is required".to_string());
    }

    let dir = PathBuf::from(trimmed);
    if !dir.exists() {
        return Err(format!("Working directory does not exist: {}", dir.display()));
    }
    if !dir.is_dir() {
        return Err(format!("Working directory is not a folder: {}", dir.display()));
    }

    Ok(dir.join(".env"))
}
