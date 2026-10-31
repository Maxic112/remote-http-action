//! SQLite storage: schema migrations and repositories.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::error::{AppError, AppResult};
use crate::model::*;
use crate::settings::Settings;

/// Ordered list of migrations. Index + 1 is stored in `PRAGMA user_version`.
const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE api_keys (
    id           INTEGER PRIMARY KEY,
    name         TEXT NOT NULL,
    key_prefix   TEXT NOT NULL,
    key_hash     TEXT NOT NULL UNIQUE,
    enabled      INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL,
    last_used_at TEXT
);
CREATE TABLE access_groups (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL
);
CREATE TABLE api_key_groups (
    api_key_id INTEGER NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    group_id   INTEGER NOT NULL REFERENCES access_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (api_key_id, group_id)
);
CREATE TABLE flows (
    id         INTEGER PRIMARY KEY,
    path       TEXT NOT NULL UNIQUE,
    data       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE group_flows (
    group_id INTEGER NOT NULL REFERENCES access_groups(id) ON DELETE CASCADE,
    flow_id  INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
    PRIMARY KEY (group_id, flow_id)
);
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE runs (
    id           TEXT PRIMARY KEY,
    flow_id      INTEGER REFERENCES flows(id) ON DELETE SET NULL,
    flow_name    TEXT NOT NULL,
    flow_path    TEXT NOT NULL,
    source       TEXT NOT NULL,
    api_key_id   INTEGER,
    api_key_name TEXT,
    client_ip    TEXT,
    status       TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    started_at   TEXT,
    finished_at  TEXT,
    error        TEXT,
    steps        TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX runs_created_at ON runs(created_at);
"#];

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> AppResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> AppResult<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn migrate(&self) -> AppResult<()> {
        let mut conn = self.conn();
        let version: usize =
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))? as usize;
        if version > MIGRATIONS.len() {
            return Err(AppError::internal(format!(
                "database schema version {version} is newer than this application supports"
            )));
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Loads settings; on first start stores and returns the defaults.
    pub fn load_settings(&self) -> AppResult<Settings> {
        let conn = self.conn();
        let raw: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key = 'app'", [], |r| {
                r.get(0)
            })
            .optional()?;
        match raw {
            Some(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            None => {
                let s = Settings::default();
                conn.execute(
                    "INSERT INTO settings(key, value) VALUES ('app', ?1)",
                    [serde_json::to_string(&s).unwrap()],
                )?;
                Ok(s)
            }
        }
    }

    pub fn save_settings(&self, s: &Settings) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO settings(key, value) VALUES ('app', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [serde_json::to_string(s).unwrap()],
        )?;
        Ok(())
    }

    /// Arbitrary JSON state stored next to the settings (e.g. update check results).
    pub fn get_state<T: serde::de::DeserializeOwned>(&self, key: &str) -> AppResult<Option<T>> {
        let raw: Option<String> = self
            .conn()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(raw.and_then(|r| serde_json::from_str(&r).ok()))
    }

    pub fn set_state<T: serde::Serialize>(&self, key: &str, value: &T) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(value).unwrap()],
        )?;
        Ok(())
    }

    fn key_from_row(conn: &Connection, r: &Row) -> rusqlite::Result<ApiKey> {
        let id: Id = r.get("id")?;
        Ok(ApiKey {
            id,
            name: r.get("name")?,
            key_prefix: r.get("key_prefix")?,
            enabled: r.get("enabled")?,
            group_ids: ids(
                conn,
                "SELECT group_id FROM api_key_groups WHERE api_key_id = ?1 ORDER BY group_id",
                id,
            )?,
            created_at: r.get("created_at")?,
            last_used_at: r.get("last_used_at")?,
        })
    }

    pub fn list_keys(&self) -> AppResult<Vec<ApiKey>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT * FROM api_keys ORDER BY id")?;
        let rows = stmt.query_map([], |r| Self::key_from_row(&conn, r))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn get_key(&self, id: Id) -> AppResult<ApiKey> {
        let conn = self.conn();
        conn.query_row("SELECT * FROM api_keys WHERE id = ?1", [id], |r| {
            Self::key_from_row(&conn, r)
        })
        .optional()?
        .ok_or_else(|| AppError::not_found(format!("API key {id} not found")))
    }

    pub fn find_key_by_hash(&self, hash: &str) -> AppResult<Option<ApiKey>> {
        let conn = self.conn();
        Ok(conn
            .query_row("SELECT * FROM api_keys WHERE key_hash = ?1", [hash], |r| {
                Self::key_from_row(&conn, r)
            })
            .optional()?)
    }

    pub fn insert_key(
        &self,
        name: &str,
        prefix: &str,
        hash: &str,
        group_ids: &[Id],
    ) -> AppResult<Id> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO api_keys(name, key_prefix, key_hash, enabled, created_at) VALUES (?1, ?2, ?3, 1, ?4)",
            params![name, prefix, hash, now()],
        )?;
        let id = tx.last_insert_rowid();
        set_links(
            &tx,
            "api_key_groups",
            "api_key_id",
            "group_id",
            id,
            group_ids,
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn update_key(
        &self,
        id: Id,
        name: Option<&str>,
        enabled: Option<bool>,
        group_ids: Option<&[Id]>,
    ) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        ensure_exists(&tx, "api_keys", id, "API key")?;
        if let Some(name) = name {
            tx.execute(
                "UPDATE api_keys SET name = ?1 WHERE id = ?2",
                params![name, id],
            )?;
        }
        if let Some(enabled) = enabled {
            tx.execute(
                "UPDATE api_keys SET enabled = ?1 WHERE id = ?2",
                params![enabled, id],
            )?;
        }
        if let Some(group_ids) = group_ids {
            set_links(
                &tx,
                "api_key_groups",
                "api_key_id",
                "group_id",
                id,
                group_ids,
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_key_secret(&self, id: Id, prefix: &str, hash: &str) -> AppResult<()> {
        let n = self.conn().execute(
            "UPDATE api_keys SET key_prefix = ?1, key_hash = ?2, last_used_at = NULL WHERE id = ?3",
            params![prefix, hash, id],
        )?;
        if n == 0 {
            return Err(AppError::not_found(format!("API key {id} not found")));
        }
        Ok(())
    }

    pub fn touch_key(&self, id: Id) -> AppResult<()> {
        self.conn().execute(
            "UPDATE api_keys SET last_used_at = ?1 WHERE id = ?2",
            params![now(), id],
        )?;
        Ok(())
    }

    pub fn delete_key(&self, id: Id) -> AppResult<()> {
        delete_by_id(&self.conn(), "api_keys", id, "API key")
    }

    fn group_from_row(conn: &Connection, r: &Row) -> rusqlite::Result<AccessGroup> {
        let id: Id = r.get("id")?;
        Ok(AccessGroup {
            id,
            name: r.get("name")?,
            description: r.get("description")?,
            key_ids: ids(
                conn,
                "SELECT api_key_id FROM api_key_groups WHERE group_id = ?1 ORDER BY api_key_id",
                id,
            )?,
            flow_ids: ids(
                conn,
                "SELECT flow_id FROM group_flows WHERE group_id = ?1 ORDER BY flow_id",
                id,
            )?,
            created_at: r.get("created_at")?,
        })
    }

    pub fn list_groups(&self) -> AppResult<Vec<AccessGroup>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT * FROM access_groups ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| Self::group_from_row(&conn, r))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn get_group(&self, id: Id) -> AppResult<AccessGroup> {
        let conn = self.conn();
        conn.query_row("SELECT * FROM access_groups WHERE id = ?1", [id], |r| {
            Self::group_from_row(&conn, r)
        })
        .optional()?
        .ok_or_else(|| AppError::not_found(format!("group {id} not found")))
    }

    pub fn insert_group(
        &self,
        name: &str,
        description: &str,
        key_ids: &[Id],
        flow_ids: &[Id],
    ) -> AppResult<Id> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO access_groups(name, description, created_at) VALUES (?1, ?2, ?3)",
            params![name, description, now()],
        )
        .map_err(|e| unique_msg(e, format!("group \"{name}\" already exists")))?;
        let id = tx.last_insert_rowid();
        set_links(&tx, "api_key_groups", "group_id", "api_key_id", id, key_ids)?;
        set_links(&tx, "group_flows", "group_id", "flow_id", id, flow_ids)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn update_group(
        &self,
        id: Id,
        name: Option<&str>,
        description: Option<&str>,
        key_ids: Option<&[Id]>,
        flow_ids: Option<&[Id]>,
    ) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        ensure_exists(&tx, "access_groups", id, "group")?;
        if let Some(name) = name {
            tx.execute(
                "UPDATE access_groups SET name = ?1 WHERE id = ?2",
                params![name, id],
            )
            .map_err(|e| unique_msg(e, format!("group \"{name}\" already exists")))?;
        }
        if let Some(d) = description {
            tx.execute(
                "UPDATE access_groups SET description = ?1 WHERE id = ?2",
                params![d, id],
            )?;
        }
        if let Some(k) = key_ids {
            set_links(&tx, "api_key_groups", "group_id", "api_key_id", id, k)?;
        }
        if let Some(f) = flow_ids {
            set_links(&tx, "group_flows", "group_id", "flow_id", id, f)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_group(&self, id: Id) -> AppResult<()> {
        delete_by_id(&self.conn(), "access_groups", id, "group")
    }

    fn flow_from_row(r: &Row) -> rusqlite::Result<Flow> {
        let data: String = r.get("data")?;
        let input: FlowInput = serde_json::from_str(&data).unwrap_or_default();
        Ok(Flow {
            id: r.get("id")?,
            input,
            created_at: r.get("created_at")?,
            updated_at: r.get("updated_at")?,
        })
    }

    pub fn list_flows(&self) -> AppResult<Vec<Flow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT * FROM flows ORDER BY path")?;
        let rows = stmt.query_map([], Self::flow_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn get_flow(&self, id: Id) -> AppResult<Flow> {
        self.conn()
            .query_row(
                "SELECT * FROM flows WHERE id = ?1",
                [id],
                Self::flow_from_row,
            )
            .optional()?
            .ok_or_else(|| AppError::not_found(format!("flow {id} not found")))
    }

    pub fn find_flow_by_path(&self, path: &str) -> AppResult<Option<Flow>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT * FROM flows WHERE path = ?1",
                [path],
                Self::flow_from_row,
            )
            .optional()?)
    }

    pub fn insert_flow(&self, input: &FlowInput) -> AppResult<Id> {
        let conn = self.conn();
        let ts = now();
        conn.execute(
            "INSERT INTO flows(path, data, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![input.path, serde_json::to_string(input).unwrap(), ts],
        )
        .map_err(|e| unique_msg(e, format!("flow path \"{}\" is already used", input.path)))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_flow(&self, id: Id, input: &FlowInput) -> AppResult<()> {
        let n = self
            .conn()
            .execute(
                "UPDATE flows SET path = ?1, data = ?2, updated_at = ?3 WHERE id = ?4",
                params![input.path, serde_json::to_string(input).unwrap(), now(), id],
            )
            .map_err(|e| unique_msg(e, format!("flow path \"{}\" is already used", input.path)))?;
        if n == 0 {
            return Err(AppError::not_found(format!("flow {id} not found")));
        }
        Ok(())
    }

    pub fn delete_flow(&self, id: Id) -> AppResult<()> {
        delete_by_id(&self.conn(), "flows", id, "flow")
    }

    /// Flows reachable by a key through its groups (key and flow enabled state is not checked here).
    pub fn flow_ids_for_key(&self, key_id: Id) -> AppResult<Vec<Id>> {
        ids(
            &self.conn(),
            "SELECT DISTINCT gf.flow_id FROM group_flows gf
             JOIN api_key_groups kg ON kg.group_id = gf.group_id
             WHERE kg.api_key_id = ?1 ORDER BY gf.flow_id",
            key_id,
        )
        .map_err(Into::into)
    }

    fn run_from_row(r: &Row) -> rusqlite::Result<Run> {
        let steps: String = r.get("steps")?;
        Ok(Run {
            id: r.get("id")?,
            flow_id: r.get("flow_id")?,
            flow_name: r.get("flow_name")?,
            flow_path: r.get("flow_path")?,
            source: RunSource::parse(&r.get::<_, String>("source")?),
            api_key_id: r.get("api_key_id")?,
            api_key_name: r.get("api_key_name")?,
            client_ip: r.get("client_ip")?,
            status: RunStatus::parse(&r.get::<_, String>("status")?),
            created_at: r.get("created_at")?,
            started_at: r.get("started_at")?,
            finished_at: r.get("finished_at")?,
            error: r.get("error")?,
            steps: serde_json::from_str(&steps).unwrap_or_default(),
        })
    }

    pub fn save_run(&self, run: &Run) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO runs(id, flow_id, flow_name, flow_path, source, api_key_id, api_key_name, client_ip,
                              status, created_at, started_at, finished_at, error, steps)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET status = excluded.status, started_at = excluded.started_at,
                 finished_at = excluded.finished_at, error = excluded.error, steps = excluded.steps",
            params![
                run.id,
                run.flow_id,
                run.flow_name,
                run.flow_path,
                run.source.as_str(),
                run.api_key_id,
                run.api_key_name,
                run.client_ip,
                run.status.as_str(),
                run.created_at,
                run.started_at,
                run.finished_at,
                run.error,
                serde_json::to_string(&run.steps).unwrap(),
            ],
        )?;
        Ok(())
    }

    pub fn get_run(&self, id: &str) -> AppResult<Run> {
        self.conn()
            .query_row("SELECT * FROM runs WHERE id = ?1", [id], Self::run_from_row)
            .optional()?
            .ok_or_else(|| AppError::not_found(format!("run {id} not found")))
    }

    pub fn list_runs(&self, flow_id: Option<Id>, limit: u32) -> AppResult<Vec<Run>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM runs WHERE (?1 IS NULL OR flow_id = ?1) ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![flow_id, limit], Self::run_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Deletes finished runs beyond the newest `keep`.
    pub fn prune_runs(&self, keep: u32) -> AppResult<()> {
        self.conn().execute(
            "DELETE FROM runs WHERE status IN ('success','failed','canceled','timeout') AND id NOT IN
               (SELECT id FROM runs ORDER BY created_at DESC LIMIT ?1)",
            [keep],
        )?;
        Ok(())
    }

    /// Marks runs left unfinished by a previous process as failed.
    pub fn fail_interrupted_runs(&self) -> AppResult<()> {
        self.conn().execute(
            "UPDATE runs SET status = 'failed', error = 'interrupted: application stopped', finished_at = ?1
             WHERE status IN ('pending','delayed','running')",
            [now()],
        )?;
        Ok(())
    }
}

fn ids(conn: &Connection, sql: &str, id: Id) -> rusqlite::Result<Vec<Id>> {
    let mut stmt = conn.prepare_cached(sql)?;
    let rows = stmt.query_map([id], |r| r.get(0))?;
    rows.collect()
}

fn set_links(
    conn: &Connection,
    table: &str,
    own_col: &str,
    other_col: &str,
    id: Id,
    others: &[Id],
) -> AppResult<()> {
    conn.execute(&format!("DELETE FROM {table} WHERE {own_col} = ?1"), [id])?;
    for other in others {
        conn.execute(
            &format!("INSERT OR IGNORE INTO {table}({own_col}, {other_col}) VALUES (?1, ?2)"),
            params![id, other],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                AppError::validation(format!("referenced item {other} does not exist"))
            }
            e => e.into(),
        })?;
    }
    Ok(())
}

fn ensure_exists(conn: &Connection, table: &str, id: Id, what: &str) -> AppResult<()> {
    let found: Option<i64> = conn
        .query_row(&format!("SELECT 1 FROM {table} WHERE id = ?1"), [id], |r| {
            r.get(0)
        })
        .optional()?;
    found
        .map(|_| ())
        .ok_or_else(|| AppError::not_found(format!("{what} {id} not found")))
}

fn delete_by_id(conn: &Connection, table: &str, id: Id, what: &str) -> AppResult<()> {
    let n = conn.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [id])?;
    if n == 0 {
        return Err(AppError::not_found(format!("{what} {id} not found")));
    }
    Ok(())
}

fn unique_msg(e: rusqlite::Error, msg: String) -> AppError {
    match &e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            AppError::conflict(msg)
        }
        _ => e.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(path: &str) -> FlowInput {
        FlowInput {
            name: path.into(),
            path: path.into(),
            ..Default::default()
        }
    }

    #[test]
    fn migrations_are_idempotent_on_file_db() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        Db::open(&path).unwrap();
        let db = Db::open(&path).unwrap();
        assert!(db.list_keys().unwrap().is_empty());
    }

    #[test]
    fn settings_are_created_on_first_load() {
        let db = Db::open_in_memory().unwrap();
        let s = db.load_settings().unwrap();
        assert_eq!(s, Settings::default());
        let mut s2 = s.clone();
        s2.server.port = 1111;
        db.save_settings(&s2).unwrap();
        assert_eq!(db.load_settings().unwrap().server.port, 1111);
    }

    #[test]
    fn key_group_flow_links_work_in_both_directions() {
        let db = Db::open_in_memory().unwrap();
        let f1 = db.insert_flow(&flow("sleep")).unwrap();
        let f2 = db.insert_flow(&flow("reboot")).unwrap();
        let g = db.insert_group("ops", "", &[], &[f1]).unwrap();
        let k = db.insert_key("phone", "rha_x", "h1", &[g]).unwrap();

        assert_eq!(db.get_group(g).unwrap().key_ids, vec![k]);
        assert_eq!(db.get_key(k).unwrap().group_ids, vec![g]);
        assert_eq!(db.flow_ids_for_key(k).unwrap(), vec![f1]);

        db.update_group(g, None, None, None, Some(&[f1, f2]))
            .unwrap();
        assert_eq!(db.flow_ids_for_key(k).unwrap(), vec![f1, f2]);

        db.update_group(g, None, None, Some(&[]), None).unwrap();
        assert!(db.get_key(k).unwrap().group_ids.is_empty());
        assert!(db.flow_ids_for_key(k).unwrap().is_empty());

        db.update_key(k, None, None, Some(&[g])).unwrap();
        db.delete_flow(f1).unwrap();
        assert_eq!(db.flow_ids_for_key(k).unwrap(), vec![f2]);
        db.delete_group(g).unwrap();
        assert!(db.get_key(k).unwrap().group_ids.is_empty());
    }

    #[test]
    fn linking_missing_items_is_a_validation_error() {
        let db = Db::open_in_memory().unwrap();
        let err = db.insert_key("k", "p", "h", &[42]).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::Validation);
    }

    #[test]
    fn duplicate_names_and_paths_conflict() {
        let db = Db::open_in_memory().unwrap();
        db.insert_group("a", "", &[], &[]).unwrap();
        assert_eq!(
            db.insert_group("a", "", &[], &[]).unwrap_err().code,
            crate::error::ErrorCode::Conflict
        );
        db.insert_flow(&flow("x")).unwrap();
        assert_eq!(
            db.insert_flow(&flow("x")).unwrap_err().code,
            crate::error::ErrorCode::Conflict
        );
    }

    #[test]
    fn keys_are_found_by_hash_and_can_be_rotated() {
        let db = Db::open_in_memory().unwrap();
        let k = db.insert_key("k", "rha_a", "hash-a", &[]).unwrap();
        assert_eq!(db.find_key_by_hash("hash-a").unwrap().unwrap().id, k);
        db.replace_key_secret(k, "rha_b", "hash-b").unwrap();
        assert!(db.find_key_by_hash("hash-a").unwrap().is_none());
        assert_eq!(db.get_key(k).unwrap().key_prefix, "rha_b");
        db.update_key(k, Some("renamed"), Some(false), None)
            .unwrap();
        let key = db.get_key(k).unwrap();
        assert_eq!(key.name, "renamed");
        assert!(!key.enabled);
        db.delete_key(k).unwrap();
        assert!(db.get_key(k).is_err());
    }

    #[test]
    fn runs_are_saved_listed_and_pruned() {
        let db = Db::open_in_memory().unwrap();
        for i in 0..5 {
            let run = Run {
                id: format!("r{i}"),
                flow_id: None,
                flow_name: "f".into(),
                flow_path: "f".into(),
                source: RunSource::Cli,
                api_key_id: None,
                api_key_name: None,
                client_ip: None,
                status: RunStatus::Success,
                created_at: format!("2026-01-01T00:00:0{i}Z"),
                started_at: None,
                finished_at: None,
                error: None,
                steps: vec![],
            };
            db.save_run(&run).unwrap();
        }
        assert_eq!(db.list_runs(None, 100).unwrap().len(), 5);
        db.prune_runs(2).unwrap();
        let runs = db.list_runs(None, 100).unwrap();
        assert_eq!(
            runs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["r4", "r3"]
        );
    }
}
