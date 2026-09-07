pub mod node;

pub use node::TrieNode;

#[derive(Debug, Clone, Default)]
pub struct Trie {
    root: TrieNode,
    len: usize,
}

impl Trie {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn insert(&mut self, key: &str, value: String, weight: f32) {
        let mut current = &mut self.root;
        for ch in key.chars() {
            current = current.children.entry(ch).or_insert_with(TrieNode::new);
        }
        if !current.is_terminal {
            self.len += 1;
        }
        current.is_terminal = true;
        current.full_text = Some(value);
        current.weight = weight;
    }

    fn walk_prefix(&self, prefix: &str) -> Option<&TrieNode> {
        let mut current = &self.root;
        for ch in prefix.chars() {
            current = current.children.get(&ch)?;
        }
        Some(current)
    }

    fn walk_prefix_mut(&mut self, prefix: &str) -> Option<&mut TrieNode> {
        let mut current = &mut self.root;
        for ch in prefix.chars() {
            current = current.children.get_mut(&ch)?;
        }
        Some(current)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.walk_prefix(key).map(|n| n.is_terminal).unwrap_or(false)
    }

    pub fn get(&self, key: &str) -> Option<(&str, f32)> {
        let node = self.walk_prefix(key)?;
        if node.is_terminal {
            Some((node.full_text.as_deref()?, node.weight))
        } else {
            None
        }
    }

    pub fn lookup_prefix(&self, prefix: &str) -> Option<(String, f32)> {
        let node = self.walk_prefix(prefix)?;
        if node.is_terminal && !node.children.is_empty() {
            return Some((node.full_text.clone()?, node.weight));
        }

        let mut current = node;
        let mut suffix = String::new();
        loop {
            if current.children.len() == 1 {
                let (ch, next) = current.children.iter().next().unwrap();
                suffix.push(*ch);
                current = next;
                if current.is_terminal {
                    let suggestion = format!("{}{}", prefix, suffix);
                    return Some((suggestion, current.weight));
                }
            } else if current.children.is_empty() {
                if current.is_terminal {
                    let suggestion = format!("{}{}", prefix, suffix);
                    return Some((suggestion, current.weight));
                }
                return None;
            } else {
                if current.is_terminal {
                    let suggestion = format!("{}{}", prefix, suffix);
                    return Some((suggestion, current.weight));
                }
                return None;
            }
        }
    }

    pub fn suggestions(&self, prefix: &str, limit: usize) -> Vec<(String, f32)> {
        let Some(node) = self.walk_prefix(prefix) else {
            return Vec::new();
        };
        let mut results = Vec::new();
        collect_below(node, prefix.to_string(), &mut results, limit);
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }

    pub fn remove(&mut self, key: &str) -> bool {
        let removed = {
            let Some(node) = self.walk_prefix_mut(key) else {
                return false;
            };
            if !node.is_terminal {
                return false;
            }
            node.is_terminal = false;
            node.full_text = None;
            node.weight = 0.0;
            true
        };
        if removed {
            self.len -= 1;
            self.prune(key);
        }
        removed
    }

    fn prune(&mut self, key: &str) {
        let chars: Vec<char> = key.chars().collect();
        Self::prune_rec(&mut self.root, &chars, 0);
    }

    fn prune_rec(node: &mut TrieNode, chars: &[char], depth: usize) -> bool {
        if depth == chars.len() {
            return !node.is_terminal && node.children.is_empty();
        }
        let ch = chars[depth];
        let should_remove = match node.children.get_mut(&ch) {
            Some(child) => Self::prune_rec(child, chars, depth + 1),
            None => return false,
        };
        if should_remove {
            node.children.remove(&ch);
        }
        !node.is_terminal && node.children.is_empty()
    }
}

fn collect_below(
    node: &TrieNode,
    current: String,
    results: &mut Vec<(String, f32)>,
    limit: usize,
) {
    if results.len() >= limit {
        return;
    }
    if node.is_terminal {
        if let Some(text) = &node.full_text {
            results.push((text.clone(), node.weight));
        }
    }
    let mut entries: Vec<(&char, &TrieNode)> = node.children.iter().collect();
    entries.sort_by(|a, b| b.1.weight.partial_cmp(&a.1.weight).unwrap_or(std::cmp::Ordering::Equal));
    for (ch, child) in entries {
        let mut next = current.clone();
        next.push(*ch);
        collect_below(child, next, results, limit);
        if results.len() >= limit {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_contains() {
        let mut t = Trie::new();
        assert!(t.is_empty());
        t.insert("git", "git".to_string(), 1.0);
        assert_eq!(t.len(), 1);
        assert!(!t.is_empty());
        assert!(t.contains("git"));
        assert!(!t.contains("gi"));
        assert!(!t.contains("gitt"));
    }

    #[test]
    fn insert_multiple_no_conflict() {
        let mut t = Trie::new();
        t.insert("git", "git".to_string(), 1.0);
        t.insert("github", "github".to_string(), 0.8);
        t.insert("gitea", "gitea".to_string(), 0.6);
        assert_eq!(t.len(), 3);
        assert!(t.contains("git"));
        assert!(t.contains("github"));
        assert!(t.contains("gitea"));
    }

    #[test]
    fn lookup_prefix_exact_terminal() {
        let mut t = Trie::new();
        t.insert("git", "git".to_string(), 1.0);
        assert_eq!(t.lookup_prefix("git"), Some(("git".to_string(), 1.0)));
    }

    #[test]
    fn lookup_prefix_unambiguous_continuation() {
        let mut t = Trie::new();
        t.insert("checkout", "checkout".to_string(), 1.0);
        assert_eq!(
            t.lookup_prefix("ch"),
            Some(("checkout".to_string(), 1.0))
        );
        assert_eq!(
            t.lookup_prefix("chec"),
            Some(("checkout".to_string(), 1.0))
        );
    }

    #[test]
    fn lookup_prefix_ambiguous_returns_none() {
        let mut t = Trie::new();
        t.insert("checkout", "checkout".to_string(), 1.0);
        t.insert("cherry", "cherry-pick".to_string(), 0.8);
        assert_eq!(t.lookup_prefix("ch"), None);
        assert_eq!(
            t.lookup_prefix("check"),
            Some(("checkout".to_string(), 1.0))
        );
    }

    #[test]
    fn lookup_prefix_terminal_with_children() {
        let mut t = Trie::new();
        t.insert("git", "git".to_string(), 1.0);
        t.insert("github", "github".to_string(), 0.9);
        let (s, w) = t.lookup_prefix("git").unwrap();
        assert_eq!(s, "git");
        assert_eq!(w, 1.0);
    }

    #[test]
    fn lookup_prefix_missing_returns_none() {
        let t = Trie::new();
        assert_eq!(t.lookup_prefix("nothing"), None);
    }

    #[test]
    fn suggestions_limited() {
        let mut t = Trie::new();
        t.insert("git", "git".to_string(), 1.0);
        t.insert("github", "github".to_string(), 0.9);
        t.insert("gitea", "gitea".to_string(), 0.8);
        t.insert("gitlab", "gitlab".to_string(), 0.7);
        let results = t.suggestions("git", 3);
        assert_eq!(results.len(), 3);
        assert!(results[0].1 >= results[1].1);
        assert!(results[1].1 >= results[2].1);
    }

    #[test]
    fn remove_existing() {
        let mut t = Trie::new();
        t.insert("git", "git".to_string(), 1.0);
        t.insert("github", "github".to_string(), 0.9);
        assert!(t.remove("git"));
        assert!(!t.contains("git"));
        assert!(t.contains("github"));
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut t = Trie::new();
        assert!(!t.remove("nope"));
    }

    #[test]
    fn empty_string_handling() {
        let mut t = Trie::new();
        t.insert("", "root".to_string(), 0.5);
        assert!(t.contains(""));
        assert_eq!(t.lookup_prefix(""), Some(("".to_string(), 0.5)));
    }

    #[test]
    fn unicode_characters() {
        let mut t = Trie::new();
        t.insert("café", "café".to_string(), 1.0);
        assert!(t.contains("café"));
        assert_eq!(t.lookup_prefix("caf"), Some(("café".to_string(), 1.0)));
    }

    #[test]
    fn get_value_and_weight() {
        let mut t = Trie::new();
        t.insert("git commit", "git commit -m".to_string(), 0.95);
        let (val, w) = t.get("git commit").unwrap();
        assert_eq!(val, "git commit -m");
        assert_eq!(w, 0.95);
    }
}
