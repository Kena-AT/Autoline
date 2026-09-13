use crate::classify::{classify, InputKind};
use crate::history::{HistoryKind, HistoryStore};
use crate::ngram::PartitionedNgramModel;
use crate::protocol::{SuggestRequest, SuggestResponse, SuggestionSource};
use crate::projects;
use crate::trie::Trie;

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub text: String,
    pub source: SuggestionSource,
    pub confidence: f32,
}

pub struct SuggestionCascade {
    command_trie: Trie,
    prompt_trie: Trie,
    history: Option<HistoryStore>,
    ngram: PartitionedNgramModel,
}

impl SuggestionCascade {
    /// Compute project boost factor based on current cwd
    fn project_boost(cwd: &str) -> f32 {
        let pid = projects::detect_project(cwd);
        match pid {
            Some(_) => 2.0,     // 2x boost for same project
            None => 1.0,        // no project context
        }
    }

    pub fn new() -> Self {
        Self {
            command_trie: Trie::new(),
            prompt_trie: Trie::new(),
            history: None,
            ngram: PartitionedNgramModel::new(),
        }
    }

    pub fn with_history(self, history: HistoryStore) -> Self {
        Self {
            history: Some(history),
            ..self
        }
    }

    pub fn with_project_boost(self, _boost: f32) -> Self {
        Self {
            ..self
        }
    }

    pub fn insert_trie(&mut self, kind: HistoryKind, key: &str, value: String, weight: f32) {
        match kind {
            HistoryKind::Command => self.command_trie.insert(key, value, weight),
            HistoryKind::Prompt => self.prompt_trie.insert(key, value, weight),
        }
    }

    pub fn train_ngram(&mut self, kind: HistoryKind, line: &str) {
        self.ngram.train_line(kind, line);
    }

    pub fn suggest(&self, line: &str, cwd: &str) -> SuggestResponse {
        let InputKind::Command(kind) = classify(line);
        let trimmed = line.trim_end_matches(|c: char| c.is_whitespace());

        let pb = Self::project_boost(cwd);

        if !trimmed.is_empty() {
            let trie = match kind {
                HistoryKind::Command => &self.command_trie,
                HistoryKind::Prompt => &self.prompt_trie,
            };

            if let Some((suffix, weight)) = trie.lookup_prefix(trimmed) {
                if let Some(rest) = suffix.strip_prefix(trimmed) {
                    let trimmed_rest = rest.trim_start_matches(|c: char| c.is_whitespace());
                    if !trimmed_rest.is_empty() {
                        return SuggestResponse {
                            suggestion: Some(rest.to_string()),
                            source: SuggestionSource::Trie,
                            confidence: clamp(weight * pb),
                        };
                    }
                }
            }
        }

        if let Some(store) = &self.history {
            if let Ok(matches) = store.prefix_match(trimmed, Some(kind), 5) {
                if let Some(best) = matches.first() {
                    let history_confidence = (best.used_count.min(10) as f32 / 10.0) * 0.9 * pb;
                    if let Some(rest) = best.line.strip_prefix(line) {
                        return SuggestResponse {
                            suggestion: Some(rest.to_string()),
                            source: SuggestionSource::History,
                            confidence: clamp(history_confidence),
                        };
                    }
                    if let Some(rest) = best.line.strip_prefix(trimmed) {
                        if line.len() >= trimmed.len() {
                            let pad = " ".repeat(line.len() - trimmed.len());
                            let history_confidence = (best.used_count.min(10) as f32 / 10.0) * 0.9 * pb;
                            return SuggestResponse {
                                suggestion: Some(format!("{}{}", pad, rest)),
                                source: SuggestionSource::History,
                                confidence: clamp(history_confidence),
                            };
                        }
                    }
                }
            }
        }

        let tokens: Vec<&str> = trimmed
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .collect();
        if !tokens.is_empty() {
            if let Some(predicted) = self.ngram.predict_next(kind, &tokens) {
                let last_char = line.chars().last();
                let with_space = last_char.map_or(true, |c| c.is_whitespace());
                let text = if with_space {
                    format!(" {}", predicted)
                } else {
                    format!(" {}", predicted)
                };
                return SuggestResponse {
                    suggestion: Some(text),
                    source: SuggestionSource::Ngram,
                    confidence: 0.4,
                };
            }
        }

        SuggestResponse::default()
    }

    pub fn suggest_from_request(&self, req: &SuggestRequest) -> SuggestResponse {
        self.suggest(&req.line, &req.cwd)
    }
}

impl Default for SuggestionCascade {
    fn default() -> Self {
        Self::new()
    }
}

fn clamp(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cascade_returns_none() {
        let c = SuggestionCascade::new();
        let r = c.suggest("anything", "/");
        assert!(r.suggestion.is_none());
        assert_eq!(r.confidence, 0.0);
    }

    #[test]
    fn trie_suggests_continuation() {
        let mut c = SuggestionCascade::new();
        c.insert_trie(
            HistoryKind::Command,
            "git checkout",
            "git checkout".to_string(),
            0.9,
        );
        let r = c.suggest("git ch", "/");
        assert!(r.suggestion.is_some());
        assert_eq!(r.source, SuggestionSource::Trie);
        assert!(r.confidence > 0.0);
    }

    #[test]
    fn history_prefix_match_suggests() {
        let store = HistoryStore::in_memory().unwrap();
        let mut c = SuggestionCascade::new().with_history(store);
        if let Some(store) = &mut c.history {
            store
                .insert(
                    "git checkout main",
                    HistoryKind::Command,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
        }
        let r = c.suggest("git checkout ", "/");
        assert!(r.suggestion.is_some());
        assert_eq!(r.source, SuggestionSource::History);
    }

    #[test]
    fn ngram_predicts_next_word() {
        let mut c = SuggestionCascade::new();
        c.train_ngram(HistoryKind::Command, "cargo build --release");
        c.train_ngram(HistoryKind::Command, "cargo build --release");
        c.train_ngram(HistoryKind::Command, "cargo test");
        let r = c.suggest("cargo ", "/");
        assert!(r.suggestion.is_some());
        assert_eq!(r.source, SuggestionSource::Ngram);
        let text = r.suggestion.unwrap();
        assert!(text.contains("build") || text.contains("test"));
    }

    #[test]
    fn classifies_prompts_and_checks_prompt_trie() {
        let mut c = SuggestionCascade::new();
        c.insert_trie(
            HistoryKind::Prompt,
            "explain this function",
            "explain this function".to_string(),
            1.0,
        );
        let r = c.suggest("explain this ", "/");
        assert!(r.suggestion.is_some());
        assert_eq!(r.source, SuggestionSource::Trie);
    }

    #[test]
    fn suggest_from_request_delegates() {
        let c = SuggestionCascade::new();
        let req = SuggestRequest {
            session_id: "s".into(),
            shell: crate::history::ShellKind::Zsh,
            line: "foo".into(),
            cwd: "/".into(),
        };
        let r = c.suggest_from_request(&req);
        assert!(r.suggestion.is_none());
    }
}
