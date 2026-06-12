use crate::database::Database;
use crate::services::backend_runtime::{BackendKind, BackendStatus, ManagedBackend};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;

static BACKEND_REGISTRY: OnceLock<Arc<BackendRegistry>> = OnceLock::new();
use tokio::sync::RwLock;

pub struct BackendRegistry {
    db: Arc<Database>,
    backends: RwLock<HashMap<String, Arc<ManagedBackend>>>,
}

impl BackendRegistry {
    pub fn get_or_init(db: Arc<crate::database::Database>) -> Arc<Self> {
        BACKEND_REGISTRY.get_or_init(|| Self::new(db)).clone()
    }

    pub fn new(db: Arc<Database>) -> Arc<Self> {
        Arc::new(Self {
            db,
            backends: RwLock::new(HashMap::new()),
        })
    }

    pub async fn get_or_create(&self, id: &str) -> Result<Arc<ManagedBackend>, String> {
        // Check cache first
        {
            let map = self.backends.read().await;
            if let Some(backend) = map.get(id) {
                return Ok(backend.clone());
            }
        }

        // Load from database
        let backend_row = self.db.get_backend(id)?;
        let start_args = backend_row
            .start_args
            .as_deref()
            .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok());
        let env_json = backend_row
            .env_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok());
        let managed = Arc::new(ManagedBackend::new(
            backend_row.id,
            backend_row.name,
            BackendKind::from_str(&backend_row.kind)?,
            backend_row.start_command,
            start_args,
            backend_row.working_dir,
            backend_row.host,
            backend_row.port,
            backend_row.health_path,
            backend_row.api_key,
            env_json,
            backend_row.auto_restart,
            backend_row.startup_timeout_ms,
        ));

        self.backends.write().await.insert(id.to_string(), managed.clone());
        Ok(managed)
    }

    pub async fn start(&self, id: &str) -> Result<(), String> {
        let backend = self.get_or_create(id).await?;
        backend.start().await
    }

    pub async fn stop(&self, id: &str) -> Result<(), String> {
        let backend = self.get_or_create(id).await?;
        backend.stop().await
    }

    pub async fn get_logs(&self, id: &str) -> Result<Vec<String>, String> {
        let backend = self.get_or_create(id).await?;
        Ok(backend.logs().await)
    }

    pub async fn send_input(&self, id: &str, input: &str) -> Result<(), String> {
        let backend = self.get_or_create(id).await?;
        backend.send_input(input).await
    }

    pub async fn record_health_result(
        &self,
        id: &str,
        ok: bool,
        message: Option<String>,
    ) -> Result<(), String> {
        let backend = self.get_or_create(id).await?;
        backend.set_health_result(ok, message).await;
        Ok(())
    }

    pub async fn forget(&self, id: &str) {
        self.backends.write().await.remove(id);
    }

    pub async fn runtime_state(
        &self,
        id: &str,
    ) -> Option<(BackendStatus, Option<u32>, Option<String>)> {
        let backend = {
            let map = self.backends.read().await;
            map.get(id).cloned()
        }?;

        Some((
            backend.status().await,
            backend.pid().await,
            backend.last_error().await,
        ))
    }

    pub async fn stop_all(&self) {
        let map = self.backends.read().await;
        for backend in map.values() {
            let _ = backend.stop().await;
        }
    }
}
