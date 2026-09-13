/// Optional age encryption support for sync backends.
///
/// History entries can be encrypted before push and decrypted after pull
/// if age encryption is enabled in the backend configuration.
use crate::sync::{SyncedHistoryEntry, LamportClock};
use crate::history::HistoryRow;
use std::collections::HashSet;
use age::r#async::Encryptor;
use age::r#async::Decryptor;
use age::KeyRing;

/// Encrypt a history entry's sensitive fields before sync.
///
/// Currently encrypts the `line` field (the raw command text) while keeping
/// metadata (normlized, used_count, timestamps) in clear text for searchability.
pub fn encrypt_entry(entry: &SyncedHistoryEntry, encryptor: &Encryptor) -> anyhow::Result<SyncedHistoryEntry> {
    let line = &entry.row.line;

    // Encrypt the line content
    let encrypted_line = encryptor.encrypt(line.as_bytes())?;

    Ok(SyncedHistoryEntry {
        row: HistoryRow {
            id: entry.row.id,
            ulid: entry.row.ulid,
            line: String::from_utf_lossy(&encrypted_line), // Store as lossy UTF-8 (binary-safe)
            normalized: entry.row.normalized.clone(), // Keep in clear for search
            kind: entry.row.kind,
            shell: entry.row.shell,
            tool: entry.row.tool,
            cwd: entry.row.cwd,
            project_id: entry.row.project_id,
            used_count: entry.row.used_count,
            last_used_at: entry.row.last_used_at,
            created_at: entry.row.created_at,
        },
        synced_at: entry.synced_at,
        lamport_clock: entry.lamport_clock.clone(),
        source: entry.source.clone(),
    })
}

/// Decrypt a history entry that was previously encrypted.
pub fn decrypt_entry(entry: &SyncedHistoryEntry, decryptor: &Decryptor) -> anyhow::Result<SyncedHistoryEntry> {
    let encrypted_line = entry.row.line.as_bytes();

    // Decrypt the line content
    let decrypted_bytes = decryptor.decrypt(encrypted_line)?;
    let decrypted_line = String::from_utf8(decrypted_bytes)?;

    Ok(SyncedHistoryEntry {
        row: HistoryRow {
            id: entry.row.id,
            ulid: entry.row.ulid,
            line: decrypted_line,
            normalized: entry.row.normalized.clone(),
            kind: entry.row.kind,
            shell: entry.row.shell,
            tool: entry.row.tool,
            cwd: entry.row.cwd,
            project_id: entry.row.project_id,
            used_count: entry.row.used_count,
            last_used_at: entry.row.last_used_at,
            created_at: entry.row.created_at,
        },
        synced_at: entry.synced_at,
        lamport_clock: entry.lamport_clock.clone(),
        source: entry.source.clone(),
    })
}

/// Check if an entry has been encrypted (line content starts with age header).
pub fn is_encrypted(entry: &SyncedHistoryEntry) -> bool {
    // age encryption prefix is typically "age-encryption.org" or similar
    entry.line.starts_with(" age") || entry.line.contains("age-encryption.org")
}

/// Remove encryption from an entry (decrypt if encrypted).
pub fn strip_encryption(entry: SyncedHistoryEntry, decryptor: &Decryptor) -> anyhow::Result<SyncedHistoryEntry> {
    if is_encrypted(&entry) {
        decrypt_entry(&entry, decryptor)
    } else {
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use age::r#async::DefaultEncryptor;
    use age::r#async::DefaultDecryptor;

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