use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
pub struct TrieNode {
    pub children: HashMap<char, TrieNode>,
    /// 存放候選項目 (候選詞, 基礎權重)
    pub candidates: Vec<(String, f64)>,
    pub is_terminal: bool,
}

#[derive(Default, Debug, Clone)]
pub struct PrefixTrie {
    root: TrieNode,
}

impl PrefixTrie {
    pub fn new() -> Self {
        Self {
            root: TrieNode::default(),
        }
    }

    /// 插入一個項目
    pub fn insert(&mut self, key: &str, candidate: String, score: f64) {
        let mut current = &mut self.root;
        for ch in key.chars() {
            current = current.children.entry(ch).or_default();
        }
        current.is_terminal = true;
        // 避免重複加入完全相同的候選詞
        if let Some(existing) = current.candidates.iter_mut().find(|(c, _)| c == &candidate) {
            if score > existing.1 {
                existing.1 = score;
            }
        } else {
            current.candidates.push((candidate, score));
            // 權重高的排在前面
            current.candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    /// 精確匹配節點
    pub fn find_exact(&self, key: &str) -> Vec<(String, f64)> {
        let mut current = &self.root;
        for ch in key.chars() {
            match current.children.get(&ch) {
                Some(next) => current = next,
                None => return Vec::new(),
            }
        }
        current.candidates.clone()
    }

    /// 前綴搜尋所有候選項目（最多返回 limit 筆）
    pub fn find_prefix(&self, prefix: &str, limit: usize) -> Vec<(String, f64)> {
        let mut current = &self.root;
        for ch in prefix.chars() {
            match current.children.get(&ch) {
                Some(next) => current = next,
                None => return Vec::new(),
            }
        }

        let mut results = Vec::new();
        let mut stack = vec![current];

        while let Some(node) = stack.pop() {
            for cand in &node.candidates {
                results.push(cand.clone());
            }
            for child in node.children.values() {
                stack.push(child);
            }
        }

        // 依權重由高到低排序並限制數量
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }
}
