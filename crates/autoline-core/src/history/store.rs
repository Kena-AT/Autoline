use super::{normalize_line, HistoryKind, HistoryRow, ShellKind};
use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

const SCHEMA_VERSION: u32 = 2;

pub struct HistoryStore {
    conn: Connection,
    machine_id: String,
}

impl HistoryStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut store = Self {
            conn,
            machine_id: whoami_hostname(),
        };
        store.apply_migrations()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut store = Self {
            conn,
            machine_id: whoami_hostname(),
        };
        store.apply_migrations()?;
        Ok(store)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn count(&self) -> Result<u64> {
        let count: u64 = self
            .conn
            .query_row("SELECT count(*) FROM history_global", [], |row| row.get(0))?;
        Ok(count)
    }

    fn apply_migrations(&mut self) -> Result<()> {
        let version: u32 = self
            .conn
            .query_row(
                "SELECT user_version FROM pragma_user_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if version < 1 {
            self.migrate_v1()?;
        }
        if version < 2 {
            self.migrate_v2()?;
        }
        self.conn
            .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    }

    fn migrate_v1(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS history_global (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    ulid          TEXT NOT NULL,
    line          TEXT    NOT NULL,
    normalized    TEXT    NOT NULL,
    kind          TEXT    NOT NULL CHECK (kind IN ('command', 'prompt')),
    shell         TEXT,
    tool          TEXT,
    cwd           TEXT,
    project_id    TEXT,
    used_count    INTEGER NOT NULL DEFAULT 1,
    last_used_at  INTEGER NOT NULL,
    created_at    INTEGER NOT NULL,
    lamport_clock INTEGER NOT NULL DEFAULT 0,
    machine_id    TEXT NOT NULL,
    synced_at     INTEGER,
    sync_backend  TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_history_global_ulid ON history_global(ulid);
CREATE INDEX IF NOT EXISTS idx_history_global_normalized ON history_global(normalized);
CREATE INDEX IF NOT EXISTS idx_history_global_kind ON history_global(kind);
CREATE INDEX IF NOT EXISTS idx_history_global_last_used ON history_global(last_used_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_history_global_dedupe ON history_global(normalized, COALESCE(shell, ''));

CREATE TABLE IF NOT EXISTS sync_state (
    backend             TEXT PRIMARY KEY,
    last_synced_at      INTEGER,
    remote_lamport_clock INTEGER DEFAULT 0,
    last_sync_error     TEXT
);
            "#,
        )?;
        Ok(())
    }

    fn migrate_v2(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS projects (
    id   TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS history_project (
    history_id INTEGER NOT NULL REFERENCES history_global(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    PRIMARY KEY (history_id, project_id)
);
            "#,
        )?;
        Ok(())
    }

    pub fn insert(
        &mut self,
        line: &str,
        kind: HistoryKind,
        shell: Option<ShellKind>,
        cwd: Option<&str>,
        tool: Option<&str>,
        project_id: Option<&crate::projects::ProjectId>,
    ) -> Result<i64> {
        let normalized = normalize_line(line);
        let now = unix_now();
        let shell_str = shell.map(|s| s.as_str().to_string());
        let dedupe_key = shell_str.clone().unwrap_or_default();

        let existing: Option<(i64, u32)> = self.conn.query_row(
            "SELECT id, used_count FROM history_global WHERE normalized = ?1 AND COALESCE(shell, '') = ?2",
            params![normalized, dedupe_key],
            |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as u32)),
        ).ok();

        let history_id = if let Some((id, count)) = existing {
            self.conn.execute(
                "UPDATE history_global SET used_count = ?1, last_used_at = ?2 WHERE id = ?3",
                params![(count + 1) as i64, now as i64, id],
            )?;
            id
        } else {
            let new_ulid = generate_ulid();
            self.conn.execute(
                "INSERT INTO history_global (ulid, line, normalized, kind, shell, tool, cwd, used_count, last_used_at, created_at, machine_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8, ?9)",
                params![
                    new_ulid,
                    line,
                    normalized,
                    kind.as_str(),
                    shell_str,
                    tool,
                    cwd,
                    now as i64,
                    self.machine_id,
                ],
            )?;
            self.conn.last_insert_rowid()
        };

        if let Some(pid) = project_id {
            self.conn.execute(
                "INSERT OR IGNORE INTO projects (id, path, name) VALUES (?1, 'unknown', 'unknown')",
                params![pid.0],
            )?;
            self.conn.execute(
                "INSERT OR IGNORE INTO history_project (history_id, project_id) VALUES (?1, ?2)",
                params![history_id, pid.0],
            )?;
        }

        Ok(history_id)
    }

    pub fn prefix_match(
        &self,
        prefix: &str,
        kind: Option<HistoryKind>,
        limit: usize,
    ) -> Result<Vec<HistoryRow>> {
        let normalized_prefix = normalize_line(prefix);
        let like_pattern = format!("{}%", normalized_prefix);
        let sql = "SELECT id, ulid, line, normalized, kind, shell, tool, cwd, project_id, used_count, last_used_at, created_at
                   FROM history_global
                   WHERE normalized LIKE ?1
                     AND (?2 IS NULL OR kind = ?2)
                   ORDER BY (used_count * 1.0 / (1 + (CAST(strftime('%s', 'now') AS INTEGER) - last_used_at) / 86400.0)) DESC
                   LIMIT ?3";
        let kind_str = kind.map(|k| k.as_str().to_string());
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![like_pattern, kind_str, limit as i64], |row| {
            map_history_row(row)
        })?;
        let mut out = Vec::with_capacity(limit);
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn last_n(&self, count: usize) -> Result<Vec<HistoryRow>> {
        let sql = "SELECT id, ulid, line, normalized, kind, shell, tool, cwd, project_id, used_count, last_used_at, created_at
                   FROM history_global ORDER BY last_used_at DESC LIMIT ?1";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![count as i64], |row| map_history_row(row))?;
        let mut out = Vec::with_capacity(count);
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn all(&self, limit: usize) -> Result<Vec<HistoryRow>> {
        self.last_n(limit)
    }

    pub fn clear(&mut self) -> Result<u64> {
        let changes = self.conn.execute("DELETE FROM history_global", [])?;
        Ok(changes as u64)
    }
}

fn map_history_row(row: &rusqlite::Row) -> rusqlite::Result<HistoryRow> {
    let id: i64 = row.get(0)?;
    let ulid: String = row.get(1)?;
    let line: String = row.get(2)?;
    let normalized: String = row.get(3)?;
    let kind_str: String = row.get(4)?;
    let shell_str: Option<String> = row.get(5)?;
    let tool: Option<String> = row.get(6)?;
    let cwd: Option<String> = row.get(7)?;
    let project_id: Option<String> = row.get(8)?;
    let used_count: i64 = row.get(9)?;
    let last_used_at: i64 = row.get(10)?;
    let created_at: i64 = row.get(11)?;

    Ok(HistoryRow {
        id,
        ulid,
        line,
        normalized,
        kind: HistoryKind::from_str(&kind_str).unwrap_or(HistoryKind::Command),
        shell: shell_str.as_deref().and_then(ShellKind::from_str),
        tool,
        cwd,
        project_id,
        used_count: used_count.max(0) as u32,
        last_used_at: last_used_at.max(0) as u64,
        created_at: created_at.max(0) as u64,
    })
}

fn unix_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn generate_ulid() -> String {
    use ulid::Ulid;
    Ulid::new().to_string().to_lowercase()
}

fn whoami_hostname() -> String {
    hostname()
        .unwrap_or_else(|| "unknown-machine".to_string())
}

fn hostname() -> Option<String> {
    use std::process::Command;
    if cfg!(windows) {
        Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    } else {
        Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_store() -> HistoryStore {
        HistoryStore::in_memory().expect("in-memory store")
    }

    #[test]
    fn open_in_memory() {
        let store = make_store();
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn open_file_based() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.db");
        {
            let mut store = HistoryStore::open(&path).unwrap();
            store
                .insert("git status", HistoryKind::Command, Some(ShellKind::Zsh), Some("/home/user"), None, None)
                .unwrap();
        }
        let store = HistoryStore::open(&path).unwrap();
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn insert_new_entry() {
        let mut store = make_store();
        let id = store
            .insert("git commit -m \"hi\"", HistoryKind::Command, Some(ShellKind::Bash), Some("/repo"), None, None)
            .unwrap();
        assert!(id > 0);
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn insert_duplicate_increments_count() {
        let mut store = make_store();
        let line = "git status";
        let id1 = store.insert(line, HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
        let id2 = store.insert(line, HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
        assert_eq!(id1, id2);
        assert_eq!(store.count().unwrap(), 1);

        let rows = store.last_n(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].used_count, 2);
    }

    #[test]
    fn insert_case_insensitive_dedupe() {
        let mut store = make_store();
        store.insert("Git Status", HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
        store.insert("  git status  ", HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
        assert_eq!(store.count().unwrap(), 1);
        assert_eq!(store.last_n(1).unwrap()[0].used_count, 2);
    }

    #[test]
    fn insert_different_shells_are_separate() {
        let mut store = make_store();
        store.insert("git status", HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
        store.insert("git status", HistoryKind::Command, Some(ShellKind::Bash), None, None, None).unwrap();
        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn prefix_match_finds_entries() {
        let mut store = make_store();
        store.insert("git commit -m a", HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
        store.insert("git checkout main", HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
        store.insert("cargo build", HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();

        let matches = store.prefix_match("git co", Some(HistoryKind::Command), 10).unwrap();
        assert_eq!(matches.len(), 1);
        for m in &matches {
            assert!(m.normalized.starts_with("git co"));
        }
    }

    #[test]
    fn prefix_match_case_insensitive() {
        let mut store = make_store();
        store.insert("Git Status", HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
        let matches = store.prefix_match("git st", Some(HistoryKind::Command), 5).unwrap();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn prefix_match_filters_by_kind() {
        let mut store = make_store();
        store.insert("git status", HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
        store.insert("git explain the log output", HistoryKind::Prompt, Some(ShellKind::Zsh), None, Some("claude"), None).unwrap();

        let commands = store.prefix_match("git ", Some(HistoryKind::Command), 10).unwrap();
        let prompts = store.prefix_match("git ", Some(HistoryKind::Prompt), 10).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(prompts.len(), 1);
    }

    #[test]
    fn last_n_returns_recent() {
        let mut store = make_store();
        for i in 0..5 {
            store.insert(&format!("cmd {}", i), HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let rows = store.last_n(3).unwrap();
        assert_eq!(rows.len(), 3);
        let ids: Vec<String> = rows.iter().map(|r| r.line.clone()).collect();
        assert_eq!(ids, vec!["cmd 4", "cmd 3", "cmd 2"]);
    }

    #[test]
    fn clear_deletes_all() {
        let mut store = make_store();
        for i in 0..10 {
            store.insert(&format!("cmd {}", i), HistoryKind::Command, Some(ShellKind::Zsh), None, None, None).unwrap();
        }
        let deleted = store.clear().unwrap();
        assert_eq!(deleted, 10);
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn kind_round_trip() {
        assert_eq!(HistoryKind::from_str(HistoryKind::Command.as_str()), Some(HistoryKind::Command));
        assert_eq!(HistoryKind::from_str(HistoryKind::Prompt.as_str()), Some(HistoryKind::Prompt));
        assert_eq!(HistoryKind::from_str("junk"), None);
    }

    #[test]
    fn shell_kind_round_trip() {
        for s in [ShellKind::Zsh, ShellKind::Bash, ShellKind::PowerShell, ShellKind::Cmd] {
            assert_eq!(ShellKind::from_str(s.as_str()), Some(s));
        }
        assert_eq!(ShellKind::from_str("nonsense"), None);
    }
}
