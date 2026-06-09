use crate::database::lock_conn;
use crate::error::AppError;
use crate::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManagedBackend {
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
    pub env_json: Option<String>,
    pub auto_restart: bool,
    pub startup_timeout_ms: u64,
    pub status: String,
    pub pid: Option<i32>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn row_to_backend(row: &rusqlite::Row) -> Result<ManagedBackend, rusqlite::Error> {
    Ok(ManagedBackend {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: row.get(2)?,
        enabled: row.get(3)?,
        managed: row.get(4)?,
        start_command: row.get(5)?,
        start_args: row.get(6)?,
        working_dir: row.get(7)?,
        host: row.get(8)?,
        port: row.get(9)?,
        health_path: row.get(10)?,
        env_json: row.get(11)?,
        auto_restart: row.get(12)?,
        startup_timeout_ms: row.get(13)?,
        status: row.get(14)?,
        pid: row.get(15)?,
        last_error: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

impl Database {
    pub fn get_all_backends(&self) -> Result<Vec<ManagedBackend>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, enabled, managed, start_command, start_args, working_dir, host, port, health_path, env_json, auto_restart, startup_timeout_ms, status, pid, last_error, created_at, updated_at FROM managed_backends ORDER BY name"
        )?;
        let rows = stmt.query_map([], row_to_backend)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn get_backend(&self, id: &str) -> Result<ManagedBackend, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, enabled, managed, start_command, start_args, working_dir, host, port, health_path, env_json, auto_restart, startup_timeout_ms, status, pid, last_error, created_at, updated_at FROM managed_backends WHERE id = ?"
        )?;
        stmt.query_row([id], row_to_backend)
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn insert_backend(
        &self,
        name: &str,
        kind: &str,
        start_command: &str,
        start_args: Option<&Vec<String>>,
        working_dir: Option<&str>,
        host: &str,
        port: u16,
        health_path: &str,
        env_json: Option<&serde_json::Value>,
        auto_restart: bool,
        startup_timeout_ms: u64,
    ) -> Result<String, AppError> {
        let id = format!("bk_{}", uuid::Uuid::new_v4().simple());
        let sa = start_args.map(|a| serde_json::to_string(a).unwrap_or_default());
        let ej = env_json.map(|v| serde_json::to_string(v).unwrap_or_default());
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT INTO managed_backends (id,name,kind,start_command,start_args,working_dir,host,port,health_path,env_json,auto_restart,startup_timeout_ms,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,strftime('%s','now'),strftime('%s','now'))",
            rusqlite::params![id, name, kind, start_command, sa, working_dir, host, port, health_path, ej, auto_restart, startup_timeout_ms],
        )?;
        Ok(id)
    }


    pub fn update_backend(
        &self, id: &str, name: Option<&str>, start_command: Option<&str>, start_args: Option<&Vec<String>>, working_dir: Option<&str>, host: Option<&str>, port: Option<u16>, health_path: Option<&str>, env_json: Option<&serde_json::Value>, auto_restart: Option<bool>, startup_timeout_ms: Option<u64>, enabled: Option<bool>) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let mut sets: Vec<String> = Vec::new();
        let mut vals: Vec<String> = Vec::new();
        if let Some(v) = name { sets.push("name=?"); vals.push(v.to_string()); }
        if let Some(v) = start_command { sets.push("start_command=?"); vals.push(v.to_string()); }
        if let Some(v) = start_args { sets.push("start_args=?"); vals.push(serde_json::to_string(v).unwrap_or_default()); }
        if let Some(v) = working_dir { sets.push("working_dir=?"); vals.push(v.to_string()); }
        if let Some(v) = host { sets.push("host=?"); vals.push(v.to_string()); }
        if let Some(v) = port { sets.push("port=?"); vals.push(v.to_string()); }
        if let Some(v) = health_path { sets.push("health_path=?"); vals.push(v.to_string()); }
        if let Some(v) = env_json { sets.push("env_json=?"); vals.push(serde_json::to_string(v).unwrap_or_default()); }
        if let Some(v) = auto_restart { sets.push("auto_restart=?"); vals.push(v.to_string()); }
        if let Some(v) = startup_timeout_ms { sets.push("startup_timeout_ms=?"); vals.push(v.to_string()); }
        if let Some(v) = enabled { sets.push("enabled=?"); vals.push(v.to_string()); }
        sets.push("updated_at=strftime('%s','now')");
        if sets.is_empty() { return Ok(()); }
        let sql = format!("UPDATE managed_backends SET {} WHERE id=?", sets.join(","));
        vals.push(id.to_string());
        let mut stmt = conn.prepare(&sql)?;
        stmt.execute(rusqlite::params_from_iter(vals.iter().map(|s| s.as_str())))?;
        Ok(())
    }

    pub fn delete_backend(&self, id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute("DELETE FROM managed_backends WHERE id=?", [id])?;
        Ok(())
    }
}
