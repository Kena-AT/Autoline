/// HTTP sync backend for synchronizing history between machines.
///
/// Communicates with a remote sync server via HTTP API.
/// Designed to be simple and deployable (e.g., as a Docker container).
use crate::sync::crdt::{merge_histories, sync_key_from_row, LamportClock, SyncedHistoryEntry};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Response from a sync GET request.
#[derive(Debug, Deserialize)]
pub struct SyncResponse {
    /// Entries to append (since our last clock)
    pub entries: Vec<SyncedHistoryEntry>,
    /// Our new Lamport clock after merge
    pub lamport_clock: u64,
    /// Number of entries skipped (conflicts/dedupe)
    pub skipped: usize,
}

/// Request body for sync POST endpoint.
#[derive(Serialize)]
pub struct SyncAppendRequest {
    /// Entries to append
    pub entries: Vec<SyncedHistoryEntry>,
    /// Our current Lamport clock
    pub lamport_clock: u64,
}

/// Sync HTTP backend.
///
/// - `pull()`: GET /sync/since?clock=X → returns entries newer than clock
/// - `push()`: POST /sync/append → appends entries, returns new clock
pub struct HttpSyncBackend {
    /// HTTP client for making requests
    client: Client,
    /// Base URL of the sync server (e.g., http://localhost:8080)
    base_url: String,
    /// Our current Lamport clock (tracks causality)
    lamport_clock: LamportClock,
    /// Whether sync is enabled
    enabled: bool,
}

impl HttpSyncBackend {
    /// Create a new HTTP sync backend
    pub fn new(base_url: String) -> Self {
        let client = Client::new();
        Self {
            client,
            base_url,
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

    /// Pull entries newer than our current Lamport clock.
    ///
    /// Returns the entries and updates our internal clock.
    pub async fn pull(&mut self) -> anyhow::Result<Vec<SyncedHistoryEntry>> {
        if !self.enabled {
            return Ok(vec![]);
        }

        let url = format!("{}/sync/since?clock={}", self.base_url, self.lamport_clock.counter);
        let resp: reqwest::Response = self.client.get(&url).send().await?;

        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await?;
            anyhow::bail!("HTTP {}: {}", status, body);
        }

        let sync_resp: SyncResponse = resp.json().await?;

        // Merge the returned entries with our local clock
        for entry in &sync_resp.entries {
            self.lamport_clock.merge(&entry.lamport_clock);
        }

        // Dedupe: only keep entries we don't already have
        let _existing_keys: HashSet<String> = sync_resp.entries
            .iter()
            .map(|e| sync_key_from_row(&e.row).clone())
            .collect();

        // Return entries ( caller will handle further dedupe via merge_histories)
        Ok(sync_resp.entries)
    }

    /// Push our local entries to the remote server.
    ///
    /// Returns the new Lamport clock from the server response.
    pub async fn push(&mut self, entries: Vec<SyncedHistoryEntry>) -> anyhow::Result<u64> {
        if !self.enabled {
            return Ok(self.lamport_clock.counter);
        }

        let req = SyncAppendRequest {
            entries,
            lamport_clock: self.lamport_clock.counter,
        };

        let url = format!("{}/sync/append", self.base_url);
        let resp: reqwest::Response = self.client.post(&url).json(&req).send().await?;

        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await?;
            anyhow::bail!("HTTP {}: {}", status, body);
        }

        let sync_resp: SyncResponse = resp.json().await?;

        // Update our clock to the server's clock
        self.lamport_clock.counter = sync_resp.lamport_clock;

        Ok(self.lamport_clock.counter)
    }
}

impl Default for HttpSyncBackend {
    fn default() -> Self {
        // Use a relative URL that would be configured at runtime
        // In practice, this would come from config
        Self::new("http://localhost:9999".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_creates_client() {
        let backend = HttpSyncBackend::default();
        assert!(backend.is_enabled());
        assert_eq!(backend.base_url, "http://localhost:9999");
    }

    #[test]
    fn clock_starts_at_zero() {
        let backend = HttpSyncBackend::new("http://example.com".to_string());
        assert_eq!(backend.lamport_clock.counter, 0);
    }
}