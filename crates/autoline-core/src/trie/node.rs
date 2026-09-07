#[derive(Debug, Clone)]
pub struct TrieNode {
    pub children: std::collections::HashMap<char, TrieNode>,
    pub is_terminal: bool,
    pub full_text: Option<String>,
    pub weight: f32,
}

impl TrieNode {
    pub fn new() -> Self {
        Self {
            children: std::collections::HashMap::new(),
            is_terminal: false,
            full_text: None,
            weight: 0.0,
        }
    }
}

impl Default for TrieNode {
    fn default() -> Self {
        Self::new()
    }
}
