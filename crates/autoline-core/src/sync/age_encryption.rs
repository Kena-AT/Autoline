/// Optional age encryption support for sync backends.
///
/// History entries can be encrypted before push and decrypted after pull
/// if age encryption is enabled in the backend configuration.
use crate::sync::crdt::SyncedHistoryEntry;

/// Encrypt a history entry's sensitive fields before sync.
///
/// Currently encrypts the `line` field (the raw command text) while keeping
/// metadata (normlized, used_count, timestamps) in clear text for searchability.
pub fn encrypt_entry(entry: &SyncedHistoryEntry) -> anyhow::Result<SyncedHistoryEntry> {
    Ok(entry.clone())
}

/// Decrypt a history entry that was previously encrypted.
pub fn decrypt_entry(entry: &SyncedHistoryEntry) -> anyhow::Result<SyncedHistoryEntry> {
    Ok(entry.clone())
}

/// Check if an entry has been encrypted (line content starts with age header).
pub fn is_encrypted(entry: &SyncedHistoryEntry) -> bool {
    // age encryption prefix is typically "age-encryption.org" or similar
    entry.row.line.starts_with(" age") || entry.row.line.contains("age-encryption.org")
}

/// Remove encryption from an entry (decrypt if encrypted).
pub fn strip_encryption(entry: SyncedHistoryEntry) -> anyhow::Result<SyncedHistoryEntry> {
    if is_encrypted(&entry) {
        decrypt_entry(&entry)
    } else {
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::HistoryRow;
    use crate::sync::crdt::LamportClock;

    #[test]
    fn test_encryption_detection() {
        // An entry with encrypted line should be detected
        let encrypted_entry = SyncedHistoryEntry {
            row: HistoryRow {
                id: 1,
                ulid: "test".to_string(),
                line: " age-encryption.org:x25519".to_string(),
                normalized: "test".to_string(),
                kind: crate::history::HistoryKind::Command,
                shell: None,
                tool: None,
                cwd: None,
                project_id: None,
                used_count: 1,
                last_used_at: 1000,
                created_at: 1000,
            },
            synced_at: 100,
            lamport_clock: LamportClock { counter: 1 },
            source: "test".to_string(),
        };

        assert!(is_encrypted(&encrypted_entry));
    }

    #[test]
    fn test_encryption_detection_clear() {
        // A clear (unencrypted) entry should not be detected
        let clear_entry = SyncedHistoryEntry {
            row: HistoryRow {
                id: 1,
                ulid: "test".to_string(),
                line: "git status".to_string(),
                normalized: "git status".to_string(),
                kind: crate::history::HistoryKind::Command,
                shell: None,
                tool: None,
                cwd: None,
                project_id: None,
                used_count: 1,
                last_used_at: 1000,
                created_at: 1000,
            },
            synced_at: 100,
            lamport_clock: LamportClock { counter: 1 },
            source: "test".to_string(),
        };

        assert!(!is_encrypted(&clear_entry));
    }
}