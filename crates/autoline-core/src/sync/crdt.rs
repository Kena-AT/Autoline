/// CRDT merge logic for synchronizing history between machines.
///
/// Uses a Lamport clock for causality tracking and ensures deterministic,
/// commutative merge of history entries.
use crate::history::HistoryRow;
use std::collections::HashMap;

/// A Lamport timestamp for tracking causality in sync operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LamportClock {
    /// The clock value
    pub counter: u64,
}

impl LamportClock {
    /// Create a new Lamport clock at 0
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Increment the clock (simulates an event on this node)
    pub fn increment(&mut self) {
        self.counter += 1;
    }

    /// Merge two clocks, taking the maximum at each position
    pub fn merge(&mut self, other: &Self) {
        if other.counter > self.counter {
            self.counter = other.counter;
        }
    }
}

/// A history entry with sync metadata.
#[derive(Debug, Clone)]
pub struct SyncedHistoryEntry {
    /// The original history row
    pub row: HistoryRow,
    /// When this entry was last synced
    pub synced_at: u64,
    /// The Lamport clock at sync time
    pub lamport_clock: LamportClock,
    /// Which sync backend this came from
    pub source: String,
}

/// Merge local and remote history entries.
///
/// For entries with the same normalized line, the one with the higher
/// lamport clock wins (more recent causal history). If clocks are equal,
/// the local entry takes precedence (commutative: either choice gives
/// deterministic results over multiple merges).
pub fn merge_histories(
    local: &mut Vec<SyncedHistoryEntry>,
    remote: Vec<SyncedHistoryEntry>,
) {
    let mut remote_map: HashMap<String, SyncedHistoryEntry> = HashMap::new();

    for entry in remote {
        let key = entry.row.normalized.clone();
        remote_map.insert(key, entry);
    }

    for local_entry in local.iter_mut() {
        if let Some(remote_entry) = remote_map.get(&local_entry.row.normalized) {
            // Merge clocks: take the maximum
            local_entry.lamport_clock.merge(&remote_entry.lamport_clock);
            // If remote has a higher lamport clock, use remote's entry
            if remote_entry.lamport_clock.counter > local_entry.lamport_clock.counter
            {
                *local_entry = remote_entry.clone();
            }
            // If equal, keep local (commutative choice)
        }
        // If entry only in local, keep it
    }

    // Add remote-only entries that aren't in local
    for (_, remote_entry) in &remote_map {
        let key = remote_entry.row.normalized.clone();
        if !local.iter().any(|e| e.row.normalized == key) {
            local.push(remote_entry.clone());
        }
    }
}

/// Extract the normalized line from a history row for use as a sync key.
/// This is the field used for deduplication and sync matching.
pub fn sync_key_from_row(row: &HistoryRow) -> String {
    row.normalized.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::HistoryKind;

    #[test]
    fn merge_same_entry_remote_wins() {
        let mut local = vec![SyncedHistoryEntry {
            row: HistoryRow {
                id: 1,
                ulid: "test".to_string(),
                line: "git status".to_string(),
                normalized: "git status".to_string(),
                kind: HistoryKind::Command,
                shell: None,
                tool: None,
                cwd: Some("/repo".to_string()),
                project_id: None,
                used_count: 1,
                last_used_at: 1000,
                created_at: 1000,
            },
            synced_at: 100,
            lamport_clock: LamportClock { counter: 1 },
            source: "local".to_string(),
        }];

        let remote = vec![SyncedHistoryEntry {
            row: HistoryRow {
                id: 2,
                ulid: "test2".to_string(),
                line: "git status".to_string(),
                normalized: "git status".to_string(),
                kind: HistoryKind::Command,
                shell: None,
                tool: None,
                cwd: Some("/other".to_string()),
                project_id: None,
                used_count: 5,
                last_used_at: 2000,
                created_at: 2000,
            },
            synced_at: 200,
            lamport_clock: LamportClock { counter: 5 },
            source: "remote".to_string(),
        }];

        merge_histories(&mut local, remote);

        // Remote should win because lamport_clock.counter 5 > 1
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].row.used_count, 5);
        assert_eq!(local[0].source, "remote");
    }

    #[test]
    fn merge_same_entry_local_wins() {
        let mut local = vec![SyncedHistoryEntry {
            row: HistoryRow {
                id: 1,
                ulid: "test".to_string(),
                line: "git status".to_string(),
                normalized: "git status".to_string(),
                kind: HistoryKind::Command,
                shell: None,
                tool: None,
                cwd: Some("/repo".to_string()),
                project_id: None,
                used_count: 1,
                last_used_at: 1000,
                created_at: 1000,
            },
            synced_at: 100,
            lamport_clock: LamportClock { counter: 5 },
            source: "local".to_string(),
        }];

        let remote = vec![SyncedHistoryEntry {
            row: HistoryRow {
                id: 2,
                ulid: "test2".to_string(),
                line: "git status".to_string(),
                normalized: "git status".to_string(),
                kind: HistoryKind::Command,
                shell: None,
                tool: None,
                cwd: Some("/other".to_string()),
                project_id: None,
                used_count: 5,
                last_used_at: 2000,
                created_at: 2000,
            },
            synced_at: 200,
            lamport_clock: LamportClock { counter: 1 },
            source: "remote".to_string(),
        }];

        merge_histories(&mut local, remote);

        // Local should win because lamport_clock.counter 5 > 1
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].row.used_count, 1);
        assert_eq!(local[0].source, "local");
    }

    #[test]
    fn merge_unique_entries_preserved() {
        let mut local = Vec::new();

        let remote = vec![SyncedHistoryEntry {
            row: HistoryRow {
                id: 1,
                ulid: "remote1".to_string(),
                line: "cargo build".to_string(),
                normalized: "cargo build".to_string(),
                kind: HistoryKind::Command,
                shell: None,
                tool: None,
                cwd: Some("/proj".to_string()),
                project_id: None,
                used_count: 1,
                last_used_at: 1000,
                created_at: 1000,
            },
            synced_at: 100,
            lamport_clock: LamportClock { counter: 1 },
            source: "remote".to_string(),
        }];

        merge_histories(&mut local, remote);

        // Remote-only entry should be added
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].row.line, "cargo build");
    }

    #[test]
    fn merge_empty_remote() {
        let mut local = vec![SyncedHistoryEntry {
            row: HistoryRow {
                id: 1,
                ulid: "local1".to_string(),
                line: "git status".to_string(),
                normalized: "git status".to_string(),
                kind: HistoryKind::Command,
                shell: None,
                tool: None,
                cwd: Some("/repo".to_string()),
                project_id: None,
                used_count: 1,
                last_used_at: 1000,
                created_at: 1000,
            },
            synced_at: 100,
            lamport_clock: LamportClock { counter: 1 },
            source: "local".to_string(),
        }];

        let remote: Vec<SyncedHistoryEntry> = Vec::new();

        merge_histories(&mut local, remote);

        // Local unchanged when remote is empty
        assert_eq!(local.len(), 1);
    }
}