use crate::model::{Data, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, sync::Mutex};

pub struct Store {
    pub connection: Mutex<Connection>,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA busy_timeout=5000;
            CREATE TABLE IF NOT EXISTS ledger (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS auth (id INTEGER PRIMARY KEY CHECK(id=1), hash TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS sessions (token_hash TEXT PRIMARY KEY, expires INTEGER NOT NULL);") .map_err(|e| e.to_string())?;
        db.execute(
            "INSERT OR IGNORE INTO ledger(id,json) VALUES(1,?1)",
            [serde_json::to_string(&Data::default()).unwrap()],
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            connection: Mutex::new(db),
        })
    }
    pub fn read(&self) -> Result<Data> {
        let db = self.connection.lock().map_err(|_| "Database lock failed")?;
        let json: String = db
            .query_row("SELECT json FROM ledger WHERE id=1", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }
    pub fn mutate<T>(
        &self,
        revision: Option<u64>,
        f: impl FnOnce(&mut Data) -> Result<T>,
    ) -> Result<T> {
        let mut db = self.connection.lock().map_err(|_| "Database lock failed")?;
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let json: String = tx
            .query_row("SELECT json FROM ledger WHERE id=1", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let mut data: Data = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        if revision.is_some_and(|r| r != data.revision) {
            return Err("Data changed on another device. Refresh and try again.".into());
        }
        let result = f(&mut data)?;
        data.validate()?;
        data.revision += 1;
        tx.execute(
            "UPDATE ledger SET json=?1 WHERE id=1",
            [serde_json::to_string(&data).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }
    pub fn password_hash(&self) -> Result<Option<String>> {
        self.connection
            .lock()
            .map_err(|_| "Database lock failed")?
            .query_row("SELECT hash FROM auth WHERE id=1", [], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())
    }
    pub fn setup(&self, hash: &str) -> Result<()> {
        self.connection
            .lock()
            .map_err(|_| "Database lock failed")?
            .execute("INSERT INTO auth(id,hash) VALUES(1,?1)", [hash])
            .map_err(|_| "Wallet has already been set up".to_string())?;
        Ok(())
    }
    pub fn session(&self, token_hash: &str, expires: i64) -> Result<()> {
        let db = self.connection.lock().map_err(|_| "Database lock failed")?;
        db.execute(
            "DELETE FROM sessions WHERE expires < ?1",
            [chrono::Utc::now().timestamp()],
        )
        .map_err(|e| e.to_string())?;
        db.execute(
            "INSERT INTO sessions(token_hash,expires) VALUES(?1,?2)",
            params![token_hash, expires],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn has_session(&self, token_hash: &str) -> bool {
        self.connection
            .lock()
            .ok()
            .and_then(|db| {
                db.query_row(
                    "SELECT 1 FROM sessions WHERE token_hash=?1 AND expires>?2",
                    params![token_hash, chrono::Utc::now().timestamp()],
                    |_| Ok(()),
                )
                .optional()
                .ok()
                .flatten()
            })
            .is_some()
    }
    pub fn logout(&self, token_hash: &str) {
        if let Ok(db) = self.connection.lock() {
            let _ = db.execute("DELETE FROM sessions WHERE token_hash=?1", [token_hash]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restart_persistence_and_failed_write_rollback() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("piko.db");
        {
            let s = Store::open(&file).unwrap();
            s.mutate(None, |d| {
                d.settings.name = "Piko test".into();
                Ok(())
            })
            .unwrap();
            assert!(s
                .mutate(None, |d| {
                    d.settings.currency = "bad".into();
                    Ok(())
                })
                .is_err());
        }
        let s = Store::open(&file).unwrap();
        assert_eq!(s.read().unwrap().settings.name, "Piko test");
        assert_eq!(s.read().unwrap().settings.currency, "USD");
    }
    #[test]
    fn stale_revision_rejected() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        s.mutate(Some(0), |_| Ok(())).unwrap();
        assert!(s.mutate(Some(0), |_| Ok(())).is_err());
    }
}
