use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NgramModel {
    pub unigrams: HashMap<String, u32>,
    pub bigrams: HashMap<String, HashMap<String, u32>>,
    pub trigrams: HashMap<(String, String), HashMap<String, u32>>,
    total_unigrams: u64,
}

impl NgramModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn train(&mut self, tokens: &[&str]) {
        if tokens.is_empty() {
            return;
        }

        for &tok in tokens {
            *self.unigrams.entry(tok.to_string()).or_insert(0) += 1;
            self.total_unigrams += 1;
        }

        for window in tokens.windows(2) {
            let a = window[0].to_string();
            let b = window[1].to_string();
            *self
                .bigrams
                .entry(a)
                .or_default()
                .entry(b)
                .or_insert(0) += 1;
        }

        for window in tokens.windows(3) {
            let a = window[0].to_string();
            let b = window[1].to_string();
            let c = window[2].to_string();
            *self
                .trigrams
                .entry((a, b))
                .or_default()
                .entry(c)
                .or_insert(0) += 1;
        }
    }

    pub fn train_line(&mut self, line: &str) {
        let tokens: Vec<&str> = simple_tokenize(line).collect();
        self.train(&tokens);
    }

    pub fn predict_next(&self, context: &[&str]) -> Option<String> {
        if context.is_empty() {
            return self.most_common_unigram();
        }

        if context.len() >= 2 {
            let tail = &context[context.len() - 2..];
            let key = (tail[0].to_string(), tail[1].to_string());
            if let Some(candidates) = self.trigrams.get(&key) {
                if let Some(best) = highest_count(candidates) {
                    return Some(best);
                }
            }
        }

        let last = context[context.len() - 1].to_string();
        if let Some(candidates) = self.bigrams.get(&last) {
            if let Some(best) = highest_count(candidates) {
                return Some(best);
            }
        }

        self.most_common_unigram()
    }

    pub fn predict_next_k(&self, context: &[&str], k: usize) -> Vec<(String, u32)> {
        let mut scored: Vec<(String, u32)> = Vec::new();

        if context.len() >= 2 {
            let tail = &context[context.len() - 2..];
            let key = (tail[0].to_string(), tail[1].to_string());
            if let Some(cands) = self.trigrams.get(&key) {
                for (w, c) in cands {
                    scored.push((w.clone(), *c * 3));
                }
            }
        }

        if !context.is_empty() {
            let last = context[context.len() - 1].to_string();
            if let Some(cands) = self.bigrams.get(&last) {
                for (w, c) in cands {
                    scored.push((w.clone(), *c * 2));
                }
            }
        }

        for (w, c) in &self.unigrams {
            scored.push((w.clone(), *c));
        }

        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.dedup_by(|a, b| a.0 == b.0);
        scored.truncate(k);
        scored
    }

    fn most_common_unigram(&self) -> Option<String> {
        highest_count(&self.unigrams)
    }

    pub fn unique_tokens(&self) -> usize {
        self.unigrams.len()
    }

    pub fn total_tokens(&self) -> u64 {
        self.total_unigrams
    }

    pub fn is_empty(&self) -> bool {
        self.total_unigrams == 0
    }
}

fn highest_count(map: &HashMap<String, u32>) -> Option<String> {
    map.iter()
        .max_by(|a, b| a.1.cmp(b.1).then_with(|| a.0.cmp(&b.0)))
        .map(|(k, _)| k.clone())
}

pub fn simple_tokenize(line: &str) -> impl Iterator<Item = &str> {
    line.split(|c: char| c.is_whitespace() || matches!(c, '|' | ';' | '&' | '>' | '<' | '(' | ')'))
        .filter(|s| !s.is_empty())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PartitionedNgramModel {
    commands: NgramModel,
    prompts: NgramModel,
}

impl PartitionedNgramModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn commands(&self) -> &NgramModel {
        &self.commands
    }

    pub fn prompts(&self) -> &NgramModel {
        &self.prompts
    }

    pub fn train(&mut self, kind: crate::history::HistoryKind, tokens: &[&str]) {
        match kind {
            crate::history::HistoryKind::Command => self.commands.train(tokens),
            crate::history::HistoryKind::Prompt => self.prompts.train(tokens),
        }
    }

    pub fn train_line(&mut self, kind: crate::history::HistoryKind, line: &str) {
        match kind {
            crate::history::HistoryKind::Command => self.commands.train_line(line),
            crate::history::HistoryKind::Prompt => self.prompts.train_line(line),
        }
    }

    pub fn predict_next(
        &self,
        kind: crate::history::HistoryKind,
        context: &[&str],
    ) -> Option<String> {
        match kind {
            crate::history::HistoryKind::Command => self.commands.predict_next(context),
            crate::history::HistoryKind::Prompt => self.prompts.predict_next(context),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_model_is_empty() {
        let m = NgramModel::new();
        assert!(m.is_empty());
        assert_eq!(m.unique_tokens(), 0);
        assert_eq!(m.total_tokens(), 0);
    }

    #[test]
    fn train_single_word() {
        let mut m = NgramModel::new();
        m.train(&["hello"]);
        assert_eq!(m.total_tokens(), 1);
        assert_eq!(m.unique_tokens(), 1);
        assert_eq!(m.unigrams.get("hello"), Some(&1));
    }

    #[test]
    fn train_builds_bigrams_and_trigrams() {
        let mut m = NgramModel::new();
        m.train(&["a", "b", "c", "d"]);
        assert_eq!(m.bigrams["a"]["b"], 1);
        assert_eq!(m.bigrams["b"]["c"], 1);
        assert_eq!(m.bigrams["c"]["d"], 1);
        assert_eq!(m.trigrams[&("a".into(), "b".into())]["c"], 1);
        assert_eq!(m.trigrams[&("b".into(), "c".into())]["d"], 1);
    }

    #[test]
    fn train_counts_accumulate() {
        let mut m = NgramModel::new();
        m.train(&["git", "status"]);
        m.train(&["git", "status"]);
        m.train(&["git", "commit"]);
        assert_eq!(m.unigrams["git"], 3);
        assert_eq!(m.bigrams["git"]["status"], 2);
        assert_eq!(m.bigrams["git"]["commit"], 1);
    }

    #[test]
    fn predict_next_from_bigram() {
        let mut m = NgramModel::new();
        m.train(&["git", "status"]);
        m.train(&["git", "status"]);
        m.train(&["git", "commit"]);
        assert_eq!(m.predict_next(&["git"]).as_deref(), Some("status"));
    }

    #[test]
    fn predict_next_from_trigram_preferred() {
        let mut m = NgramModel::new();
        m.train(&["git", "commit", "-m"]);
        m.train(&["git", "commit", "-m"]);
        m.train(&["git", "commit", "--amend"]);
        assert_eq!(
            m.predict_next(&["git", "commit"]).as_deref(),
            Some("-m")
        );
    }

    #[test]
    fn predict_next_falls_back_when_trigram_missing() {
        let mut m = NgramModel::new();
        m.train(&["git", "status"]);
        m.train(&["docker", "run"]);
        assert_eq!(m.predict_next(&["missing", "git"]).as_deref(), Some("status"));
    }

    #[test]
    fn predict_next_empty_uses_unigram() {
        let mut m = NgramModel::new();
        m.train(&["git", "status"]);
        m.train(&["git", "commit"]);
        m.train(&["cargo", "build"]);
        assert_eq!(m.predict_next(&[]).as_deref(), Some("git"));
    }

    #[test]
    fn predict_next_empty_model_returns_none() {
        let m = NgramModel::new();
        assert_eq!(m.predict_next(&["anything"]), None);
    }

    #[test]
    fn predict_next_k_returns_weighted_ranking() {
        let mut m = NgramModel::new();
        m.train(&["git", "commit", "-m"]);
        m.train(&["git", "commit", "-m"]);
        m.train(&["git", "status"]);
        let top = m.predict_next_k(&["git", "commit"], 3);
        assert_eq!(top[0].0, "-m");
    }

    #[test]
    fn train_line_tokenizes() {
        let mut m = NgramModel::new();
        m.train_line("git status && echo done");
        assert!(m.bigrams.contains_key("git"));
        assert_eq!(m.bigrams["git"]["status"], 1);
        assert_eq!(m.bigrams["echo"]["done"], 1);
    }

    #[test]
    fn empty_train_is_noop() {
        let mut m = NgramModel::new();
        m.train(&[]);
        m.train_line("");
        assert!(m.is_empty());
    }

    #[test]
    fn partitioned_separates_commands_and_prompts() {
        use crate::history::HistoryKind;
        let mut m = PartitionedNgramModel::new();
        m.train_line(HistoryKind::Command, "git status");
        m.train_line(HistoryKind::Command, "git commit");
        m.train_line(HistoryKind::Prompt, "explain this function");
        m.train_line(HistoryKind::Prompt, "explain the code");

        assert_eq!(
            m.predict_next(HistoryKind::Command, &["git"])
                .as_deref(),
            Some("status")
        );
        assert_eq!(
            m.predict_next(HistoryKind::Prompt, &["explain"])
                .as_deref(),
            Some("this")
        );
    }
}
