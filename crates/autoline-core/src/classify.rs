use crate::history::HistoryKind;

static KNOWN_BINARIES: &[&str] = &[
    "git", "cargo", "rustc", "rustup", "docker", "kubectl", "npm", "pnpm", "yarn", "node", "npx",
    "python", "python3", "pip", "pip3", "go", "java", "javac", "mvn", "gradle", "make", "cmake",
    "gcc", "g++", "clang", "ls", "cd", "pwd", "cat", "echo", "mkdir", "rm", "cp", "mv", "find",
    "grep", "sed", "awk", "curl", "wget", "ssh", "scp", "rsync", "tar", "unzip", "zip", "chmod",
    "chown", "sudo", "su", "brew", "apt", "apt-get", "yum", "dnf", "pacman", "zypper", "winget",
    "choco", "scoop", "powershell", "pwsh", "cmd", "zsh", "bash", "fish", "nu", "starship",
    "zoxide", "fzf", "rg", "fd", "exa", "eza", "bat", "jq", "yq", "terraform", "helm", "aws",
    "gcloud", "az", "vercel", "netlify", "gh", "glab", "code", "nvim", "vim", "vi", "nano",
    "claude", "gemini", "ollama", "llm",
];

static NL_KEYWORDS: &[&str] = &[
    "explain",
    "write",
    "refactor",
    "simplify",
    "test",
    "describe",
    "analyze",
    "summarize",
    "review",
    "debug",
    "fix",
    "improve",
    "generate",
    "create",
    "how",
    "what",
    "why",
    "when",
    "where",
    "please",
    "can you",
    "help",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    Command(HistoryKind),
}

impl InputKind {
    pub fn history_kind(self) -> HistoryKind {
        match self {
            InputKind::Command(k) => k,
        }
    }
}

pub fn classify(line: &str) -> InputKind {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return InputKind::Command(HistoryKind::Command);
    }

    let lower = trimmed.to_lowercase();
    let first_token = lower.split_whitespace().next().unwrap_or("");

    if first_token.is_empty() {
        return InputKind::Command(HistoryKind::Command);
    }

    if trimmed.starts_with("./")
        || trimmed.starts_with("../")
        || trimmed.starts_with('/')
        || trimmed.starts_with('~')
        || trimmed.starts_with('.') && first_token.len() > 1 && first_token.contains('.')
    {
        return InputKind::Command(HistoryKind::Command);
    }

    for prefix in ["sudo ", "doas ", "nice ", "time ", "nohup "] {
        if lower.starts_with(prefix) {
            let rest = &lower[prefix.len()..];
            let rest_first = rest.split_whitespace().next().unwrap_or("");
            if KNOWN_BINARIES.contains(&rest_first) {
                return InputKind::Command(HistoryKind::Command);
            }
        }
    }

    let contains_pipe_sem =
        trimmed.contains('|') || trimmed.contains(';') || trimmed.contains("&&") || trimmed.contains(">>") || trimmed.contains('>');
    let has_flag = trimmed.split_whitespace().any(|t| t.starts_with('-'));

    // Check for explicit AI tool prefixes FIRST (before known binaries),
    // so that "claude explain..." and "gemini write..." are recognized as prompts
    let for_ai_tool = lower.starts_with("claude ")
        || lower.starts_with("gemini ")
        || lower.starts_with("ollama run ")
        || lower.starts_with("llm ");
    if for_ai_tool {
        return InputKind::Command(HistoryKind::Prompt);
    }

    if KNOWN_BINARIES.contains(&first_token) {
        return InputKind::Command(HistoryKind::Command);
    }

    if first_token.ends_with(".exe")
        || first_token.ends_with(".bat")
        || first_token.ends_with(".cmd")
        || first_token.ends_with(".ps1")
        || first_token.ends_with(".sh")
    {
        return InputKind::Command(HistoryKind::Command);
    }

    if has_flag || contains_pipe_sem {
        return InputKind::Command(HistoryKind::Command);
    }

    let lower_no_tool = if let Some(rest) = lower.strip_prefix("claude ") {
        rest
    } else if let Some(rest) = lower.strip_prefix("gemini ") {
        rest
    } else {
        lower.as_str()
    };

    for kw in NL_KEYWORDS {
        if lower_no_tool.starts_with(kw) {
            return InputKind::Command(HistoryKind::Prompt);
        }
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    let mut nl_score = 0u32;
    for t in &tokens {
        for kw in NL_KEYWORDS {
            if t == kw {
                nl_score += 2;
            }
        }
        if t.ends_with('?') || t.ends_with('!') {
            nl_score += 1;
        }
    }
    if tokens.len() >= 6 {
        nl_score += 1;
    }

    let command_score = if tokens.len() <= 2 { 2 } else { 0 };

    if nl_score > command_score {
        InputKind::Command(HistoryKind::Prompt)
    } else {
        InputKind::Command(HistoryKind::Command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_binary_is_command() {
        assert_eq!(
            classify("git status").history_kind(),
            HistoryKind::Command
        );
        assert_eq!(
            classify("cargo build --release").history_kind(),
            HistoryKind::Command
        );
        assert_eq!(
            classify("docker run -it ubuntu").history_kind(),
            HistoryKind::Command
        );
    }

    #[test]
    fn relative_executable_is_command() {
        assert_eq!(
            classify("./build.sh").history_kind(),
            HistoryKind::Command
        );
        assert_eq!(
            classify("../run").history_kind(),
            HistoryKind::Command
        );
    }

    #[test]
    fn sudo_prefix_is_command() {
        assert_eq!(
            classify("sudo apt update").history_kind(),
            HistoryKind::Command
        );
        assert_eq!(
            classify("  sudo   rm -rf /tmp/*").history_kind(),
            HistoryKind::Command
        );
    }

    #[test]
    fn claude_prompt_is_prompt() {
        assert_eq!(
            classify("claude explain this function").history_kind(),
            HistoryKind::Prompt
        );
        assert_eq!(
            classify("gemini write tests for the auth module").history_kind(),
            HistoryKind::Prompt
        );
    }

    #[test]
    fn natural_language_keywords_are_prompt() {
        assert_eq!(
            classify("explain the error below").history_kind(),
            HistoryKind::Prompt
        );
        assert_eq!(
            classify("refactor this code").history_kind(),
            HistoryKind::Prompt
        );
        assert_eq!(
            classify("how does the borrow checker work").history_kind(),
            HistoryKind::Prompt
        );
    }

    #[test]
    fn flag_likely_command() {
        assert_eq!(
            classify("mytool --verbose").history_kind(),
            HistoryKind::Command
        );
    }

    #[test]
    fn pipe_likely_command() {
        assert_eq!(
            classify("echo hello | cat").history_kind(),
            HistoryKind::Command
        );
        assert_eq!(
            classify("ls ; echo done").history_kind(),
            HistoryKind::Command
        );
    }

    #[test]
    fn empty_string_is_command() {
        assert_eq!(classify("").history_kind(), HistoryKind::Command);
        assert_eq!(classify("   ").history_kind(), HistoryKind::Command);
    }

    #[test]
    fn extension_based_command() {
        assert_eq!(
            classify("script.bat run").history_kind(),
            HistoryKind::Command
        );
        assert_eq!(
            classify("./deploy.sh prod").history_kind(),
            HistoryKind::Command
        );
    }

    #[test]
    fn long_natural_language_is_prompt() {
        assert_eq!(
            classify(
                "please help me understand why this code compiles but fails at runtime"
            )
            .history_kind(),
            HistoryKind::Prompt
        );
    }
}
