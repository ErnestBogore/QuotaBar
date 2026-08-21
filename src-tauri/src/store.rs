use chrono::{DateTime, Duration, Utc};
use quota_core::{MeterEngine, RateLimitSnapshot, UsageEvent};
use rusqlite::{params, Connection};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let store = Self { path };
        store.migrate()?;
        Ok(store)
    }

    fn connection(&self) -> Result<Connection, String> {
        Connection::open(&self.path).map_err(|error| error.to_string())
    }

    fn migrate(&self) -> Result<(), String> {
        self.connection()?
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA foreign_keys=ON;
                 CREATE TABLE IF NOT EXISTS app_state (
                   key TEXT PRIMARY KEY,
                   value TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS rate_samples (
                   id INTEGER PRIMARY KEY AUTOINCREMENT,
                   observed_at TEXT NOT NULL,
                   quota_id TEXT NOT NULL,
                   payload TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS usage_events (
                   id INTEGER PRIMARY KEY AUTOINCREMENT,
                   observed_at TEXT NOT NULL,
                   source TEXT NOT NULL,
                   weighted_tokens REAL NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS daily_usage (
                   day TEXT PRIMARY KEY,
                   weighted_tokens REAL NOT NULL
                 );",
            )
            .map_err(|error| error.to_string())
    }

    pub fn load_engine(&self) -> Result<MeterEngine, String> {
        let connection = self.connection()?;
        let result: Result<String, _> = connection.query_row(
            "SELECT value FROM app_state WHERE key = 'meter_engine'",
            [],
            |row| row.get(0),
        );
        match result {
            Ok(json) => {
                let mut engine: MeterEngine =
                    serde_json::from_str(&json).map_err(|error| error.to_string())?;
                engine.migrate_persisted_state();
                Ok(engine)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(MeterEngine::default()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn save_engine(&self, engine: &MeterEngine) -> Result<(), String> {
        let json = serde_json::to_string(engine).map_err(|error| error.to_string())?;
        self.connection()?
            .execute(
                "INSERT INTO app_state(key, value, updated_at) VALUES('meter_engine', ?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![json, Utc::now().to_rfc3339()],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn record_rate(&self, snapshot: &RateLimitSnapshot) -> Result<(), String> {
        let json = serde_json::to_string(snapshot).map_err(|error| error.to_string())?;
        self.connection()?
            .execute(
                "INSERT INTO rate_samples(observed_at, quota_id, payload) VALUES(?1, ?2, ?3)",
                params![snapshot.observed_at.to_rfc3339(), snapshot.quota_id, json],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn record_usage(&self, event: &UsageEvent) -> Result<(), String> {
        let source =
            serde_json::to_string(&event.source).unwrap_or_else(|_| "\"unknown\"".to_string());
        let connection = self.connection()?;
        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO usage_events(observed_at, source, weighted_tokens) VALUES(?1, ?2, ?3)",
                params![
                    event.observed_at.to_rfc3339(),
                    source,
                    event.weighted_tokens
                ],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO daily_usage(day, weighted_tokens) VALUES(?1, ?2)
                 ON CONFLICT(day) DO UPDATE SET weighted_tokens = weighted_tokens + excluded.weighted_tokens",
                params![event.observed_at.date_naive().to_string(), event.weighted_tokens],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn prune_details(&self, now: DateTime<Utc>) -> Result<(), String> {
        let cutoff = (now - Duration::days(30)).to_rfc3339();
        self.connection()?
            .execute_batch(&format!(
                "DELETE FROM rate_samples WHERE observed_at < '{}';
                 DELETE FROM usage_events WHERE observed_at < '{}';",
                cutoff.replace('\'', "''"),
                cutoff.replace('\'', "''")
            ))
            .map_err(|error| error.to_string())
    }

    pub fn delete_history(&self) -> Result<(), String> {
        self.connection()?
            .execute_batch(
                "DELETE FROM rate_samples;
                 DELETE FROM usage_events;
                 DELETE FROM daily_usage;
                 DELETE FROM app_state WHERE key = 'meter_engine';",
            )
            .map_err(|error| error.to_string())
    }
}
