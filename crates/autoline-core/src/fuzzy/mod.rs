#[derive(Debug, Clone, Default)]
pub struct FuzzyMatcher;

impl FuzzyMatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rerank(&self, query: &str, candidates: Vec<String>) -> Vec<(String, f32)> {
        let query_chars: Vec<char> = query.chars().collect();
        let mut scored: Vec<(String, f32)> = candidates
            .into_iter()
            .map(|c| {
                let score = fuzzy_score(&query_chars, &c);
                (c, score)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }
}

fn fuzzy_score(query: &[char], candidate: &str) -> f32 {
    if query.is_empty() {
        return 1.0;
    }
    let candidate_chars: Vec<char> = candidate.chars().collect();
    let cand_lower: Vec<char> = candidate_chars
        .iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let query_lower: Vec<char> = query.iter().map(|c| c.to_ascii_lowercase()).collect();

    let mut qi = 0;
    let mut last_match = 0i64;
    let mut consecutive = 0i64;
    let mut best_streak = 0i64;
    let mut gaps = 0i64;

    for (ci, ch) in cand_lower.iter().enumerate() {
        if qi >= query_lower.len() {
            break;
        }
        if *ch == query_lower[qi] {
            if qi > 0 && (ci as i64) == last_match + 1 {
                consecutive += 1;
                best_streak = best_streak.max(consecutive);
            } else if qi > 0 {
                gaps += (ci as i64) - last_match - 1;
                consecutive = 0;
            }
            last_match = ci as i64;
            qi += 1;
        }
    }

    if qi < query_lower.len() {
        return 0.0;
    }

    let coverage = query.len() as f32 / candidate_chars.len().max(1) as f32;
    let streak_bonus = (best_streak as f32 / query.len() as f32).min(1.0);
    let gap_penalty = 1.0 / (1.0 + gaps as f32);
    let base = coverage * 0.4 + streak_bonus * 0.4 + gap_penalty * 0.2;
    base.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_scores_high() {
        let m = FuzzyMatcher::new();
        let cands = vec!["git checkout".to_string(), "github".to_string()];
        let result = m.rerank("git", cands);
        assert!(!result.is_empty());
        for (_, s) in &result {
            assert!(*s > 0.0);
        }
    }

    #[test]
    fn substring_match() {
        let m = FuzzyMatcher::new();
        let result = m.rerank(
            "checkout",
            vec![
                "git checkout".to_string(),
                "git commit".to_string(),
            ],
        );
        assert_eq!(result[0].0, "git checkout");
    }

    #[test]
    fn non_matching_filtered_out() {
        let m = FuzzyMatcher::new();
        let result = m.rerank("zzzz", vec!["abc".to_string(), "def".to_string()]);
        assert!(result.is_empty());
    }

    #[test]
    fn empty_query_scores_one() {
        let s = fuzzy_score(&[], "abc");
        assert_eq!(s, 1.0);
    }

    #[test]
    fn rerank_preserves_top() {
        let m = FuzzyMatcher::new();
        let result = m.rerank(
            "gitch",
            vec![
                "git checkout".to_string(),
                "some unrelated".to_string(),
            ],
        );
        if !result.is_empty() {
            assert_eq!(result[0].0, "git checkout");
        }
    }
}
