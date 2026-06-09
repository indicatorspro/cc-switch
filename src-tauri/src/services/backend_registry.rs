use crate::database::Database;
use crate::services::backend_runtime::{BackendKind, ManagedBackend};
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
        let managed = Arc::new(ManagedBackend::new(
            backend_row.id,
            backend_row.name,
            BackendKind::from_str(&backend_row.kind)?,
            backend_row.start_command,
            backend_row.start_args,
            backend_row.working_dir,
            backend_row.host,
            backend_row.port,
            backend_row.health_path,
            backend_row.env_json,
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

    pub async fn stop_all(&self) {
        let map = self.backends.read().await;
        for backend in map.values() {
            let _ = backend.stop().await;
        }
    }
}
