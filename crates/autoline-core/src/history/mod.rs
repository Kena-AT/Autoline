pub mod store;

pub use store::HistoryStore;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryKind {
    Command,
    Prompt,
}

impl HistoryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            HistoryKind::Command => "command",
            HistoryKind::Prompt => "prompt",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "command" => Some(HistoryKind::Command),
            "prompt" => Some(HistoryKind::Prompt),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellKind {
    Zsh,
    Bash,
    PowerShell,
    Cmd,
}

impl ShellKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ShellKind::Zsh => "zsh",
            ShellKind::Bash => "bash",
            ShellKind::PowerShell => "powershell",
            ShellKind::Cmd => "cmd",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "zsh" => Some(ShellKind::Zsh),
            "bash" => Some(ShellKind::Bash),
            "powershell" => Some(ShellKind::PowerShell),
            "cmd" => Some(ShellKind::Cmd),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HistoryRow {
    pub id: i64,
    pub ulid: String,
    pub line: String,
    pub normalized: String,
    pub kind: HistoryKind,
    pub shell: Option<ShellKind>,
    pub tool: Option<String>,
    pub cwd: Option<String>,
    pub project_id: Option<String>,
    pub used_count: u32,
    pub last_used_at: u64,
    pub created_at: u64,
}

pub fn normalize_line(line: &str) -> String {
    line.trim().to_lowercase()
}
