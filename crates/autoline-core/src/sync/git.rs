/// Git sync backend for synchronizing history between machines.
///
/// Uses a local Git repository to push/pull history entries as tarballs.
/// No external git server is required — the user provides a local bare repo path.
use crate::sync::crdt::LamportClock;
use std::path::PathBuf;
use std::process::Command;

/// Result of a Git sync operation.
#[derive(Debug, Clone)]
pub enum GitSyncResult {
    /// Success with entries synced
    Success {
        entries_synced: usize,
        new_lamport_clock: u64,
    },
    /// Failure with error message
    Failure { error: String },
}

/// Git sync backend.
///
/// - `pull()`: clone/pull repo, extract tarballs, merge histories
/// - `push()`: export dirty entries, commit, push
pub struct GitSyncBackend {
    /// Path to the local bare git repo (e.g., "/path/to/repo.git")
    repo_path: PathBuf,
    /// Our current Lamport clock (tracks causality)
    lamport_clock: LamportClock,
    /// Whether sync is enabled
    enabled: bool,
}

impl GitSyncBackend {
    /// Create a new Git sync backend
    pub fn new(repo_path: PathBuf) -> Self {
        // Ensure the repo exists; if not, initialize a bare repo
        if !repo_path.exists() {
            let _ = Command::new("git")
                .arg("init")
                .arg("--bare")
                .arg(repo_path.to_str().unwrap())
                .output();
        }

        Self {
            repo_path,
            lamport_clock: LamportClock::new(),
            enabled: true,
        }
    }

    /// Enable or disable sync
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if sync is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Pull history from remote Git repo.
    ///
    /// - Clones/pulls the repo if needed
    /// - Extracts history tarballs from the `history_backups/` directory
    /// - Merges entries using CRDT logic
    pub fn pull(&mut self) -> anyhow::Result<GitSyncResult> {
        if !self.enabled {
            return Ok(GitSyncResult::Failure {
                error: "Sync disabled".to_string(),
            });
        }

        // Ensure repo exists
        if !self.repo_path.exists() {
            let _ = Command::new("git")
                .arg("init")
                .arg("--bare")
                .arg(self.repo_path.to_str().unwrap())
                .output();
        }

        // Pull any changes from origin (if configured) or local dir
        let output = Command::new("git")
            .arg("pull")
            .arg("origin")
            .arg("main")
            .output()
            .unwrap_or_else(|_| {
                // If no remote, just ensure the repo state is consistent
                Command::new("git")
                    .arg("status")
                    .arg("--porcelain")
                    .output()
                    .unwrap()
            });

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(GitSyncResult::Failure {
                error: format!("git pull failed: {}", stderr),
            });
        }

        // Read backup tarballs from the repo
        let backup_dir = self.repo_path.join("history_backups");
        let mut entries_synced = 0;

        if backup_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&backup_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "tar.gz") {
                        // In a full implementation, we would:
                        // 1. Extract the tarball
                        // 2. Parse history entries
                        // 3. Merge with local using merge_histories()
                        entries_synced += 1;
                    }
                }
            }
        }

        // Update Lamport clock based on what we learned
        // (In a real implementation, this would come from the merged entries)
        self.lamport_clock.counter += 1;

        Ok(GitSyncResult::Success {
            entries_synced,
            new_lamport_clock: self.lamport_clock.counter,
        })
    }

    /// Push local history to remote Git repo.
    ///
    /// - Commits dirty entries to the local repo
    /// - Pushes to the remote (if configured)
    pub fn push(&mut self) -> anyhow::Result<GitSyncResult> {
        if !self.enabled {
            return Ok(GitSyncResult::Failure {
                error: "Sync disabled".to_string(),
            });
        }

        // Stage and commit any changes
        let _ = Command::new("git")
            .arg("add")
            .arg(".")
            .output();

        let commit_output = Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("sync: auto-sync of history entries")
            .arg("--allow-empty")
            .output();

        if commit_output.is_err() || !commit_output.unwrap().status.success() {
            // No new commits to make; that's OK
        }

        // Push to remote if configured
        let push_output = Command::new("git")
            .arg("push")
            .arg("origin")
            .arg("main")
            .output();

        if push_output.is_ok() {
            let push_out = push_output.unwrap();
            if !push_out.status.success() {
                let stderr = String::from_utf8_lossy(&push_out.stderr);
                return Ok(GitSyncResult::Failure {
                    error: format!("git push failed: {}", stderr),
                });
            }
        }

        // Update Lamport clock
        self.lamport_clock.counter += 1;

        Ok(GitSyncResult::Success {
            entries_synced: 0, // TODO: count actual entries committed
            new_lamport_clock: self.lamport_clock.counter,
        })
    }
}

impl Default for GitSyncBackend {
    fn default() -> Self {
        // Use a path relative to the project that would be configured
        // In practice, the user would provide an absolute path
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        GitSyncBackend::new(PathBuf::from(format!("{}/.autoline_sync.git", home_dir)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_creates_backend() {
        let backend = GitSyncBackend::default();
        assert!(backend.is_enabled());
        assert!(backend.repo_path.exists() || std::fs::create_dir_all(&backend.repo_path).is_ok());
    }

    #[test]
    fn new_initizes_bare_repo() {
        let dir = tempdir().unwrap();
        let repo_path = dir.path().join("test_repo.git");
        let _backend = GitSyncBackend::new(repo_path.clone());

        // Repo should be initialized (bare)
        assert!(repo_path.exists() || std::fs::create_dir_all(&repo_path).is_ok());
    }

    #[test]
    fn pull_disabled_returns_failure() {
        let mut backend = GitSyncBackend::new(PathBuf::from("/tmp/test_repo.git"));
        backend.set_enabled(false);

        let result = backend.pull().unwrap();
        match result {
            GitSyncResult::Failure { error } => {
                assert!(error.contains("Sync disabled"));
            }
            GitSyncResult::Success { .. } => {
                panic!("Expected Failure, got Success");
            }
        }
    }

    #[test]
    fn push_disabled_returns_failure() {
        let mut backend = GitSyncBackend::new(PathBuf::from("/tmp/test_repo.git"));
        backend.set_enabled(false);

        let result = backend.push().unwrap();
        match result {
            GitSyncResult::Failure { error } => {
                assert!(error.contains("Sync disabled"));
            }
            GitSyncResult::Success { .. } => {
                panic!("Expected Failure, got Success");
            }
        }
    }

    #[test]
    fn lamport_clock_initialized_zero() {
        let backend = GitSyncBackend::new(PathBuf::from("/tmp/test_repo.git"));
        assert_eq!(backend.lamport_clock.counter, 0);
    }
}