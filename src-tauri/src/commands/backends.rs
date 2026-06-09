use crate::database::Database;
use crate::services::backend_registry::BackendRegistry;
use crate::services::backend_runtime::{BackendKind, BackendStatus};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBackendRequest {
    pub name: String,
    pub kind: String,
    pub start_command: String,
    pub start_args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub host: Option<String>,
    pub port: u16,
    pub health_path: Option<String>,
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
    pub env_json: Option<serde_json::Value>,
    pub auto_restart: bool,
    pub startup_timeout_ms: u64,
    pub status: String,
    pub pid: Option<u32>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[tauri::command]
pub async fn list_backends(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<BackendInfo>, String> {
    let db = state.db.clone();
    let backends = db.get_all_backends().map_err(|e| e.to_string())?;
    Ok(backends.into_iter().map(BackendInfo::from_row).collect())
}

#[tauri::command]
pub async fn get_backend(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<BackendInfo, String> {
    let db = state.db.clone();
    let backend = db.get_backend(&id).map_err(|e| e.to_string())?;
    Ok(BackendInfo::from_row(backend))
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
            req.port,
            req.health_path.as_deref().unwrap_or("/health"),
            req.env_json.as_ref(),
            req.auto_restart.unwrap_or(true),
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
    db.update_backend(
        &id,
        req.name.as_deref(),
        req.start_command.as_deref(),
        req.start_args.as_ref(),
        req.working_dir.as_deref(),
        req.host.as_deref(),
        req.port,
        req.health_path.as_deref(),
        req.env_json.as_ref(),
        req.auto_restart,
        req.startup_timeout_ms,
        req.enabled,
    )
    .map_err(|e| e.to_string())?;
    let backend = db.get_backend(&id).map_err(|e| e.to_string())?;
    Ok(BackendInfo::from_row(backend))
}

#[tauri::command]
pub async fn delete_backend(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.clone();
    // Stop first if running
    let _ = stop_backend(id.clone(), state.clone()).await;
    db.delete_backend(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_backend(
    id: String,
    _app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<BackendInfo, String> {
    let db = state.db.clone();
    let backend = db.get_backend(&id).map_err(|e| e.to_string())?;
    let registry = BackendRegistry::get_or_init(db.clone());
    registry.start(&id).await.map_err(|e| e.to_string())?;
    let updated = db.get_backend(&id).map_err(|e| e.to_string())?;
    Ok(BackendInfo::from_row(updated))
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
    Ok(BackendInfo::from_row(updated))
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

impl BackendInfo {
    fn from_row(b: crate::database::ManagedBackend) -> Self {
        Self {
            id: b.id,
            name: b.name,
            kind: b.kind,
            enabled: b.enabled,
            managed: b.managed,
            start_command: b.start_command,
            start_args: b.start_args,
            working_dir: b.working_dir,
            host: b.host,
            port: b.port,
            health_path: b.health_path,
            env_json: b.env_json,
            auto_restart: b.auto_restart,
            startup_timeout_ms: b.startup_timeout_ms,
            status: b.status,
            pid: b.pid.map(|p| p as u32),
            last_error: b.last_error,
            created_at: b.created_at,
            updated_at: b.updated_at,
        }
    }
}
