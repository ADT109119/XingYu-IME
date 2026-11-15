use super::trie::PrefixTrie;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 自訂辭典管理器
/// 具備自訂詞優先（置頂權重加成）與熱重載功能
pub struct CustomDictionary {
    pub file_path: Option<PathBuf>,
    pub trie: PrefixTrie,
    pub last_modified: Option<SystemTime>,
    pub priority_boost: f64,
}

impl Default for CustomDictionary {
    fn default() -> Self {
        Self {
            file_path: None,
            trie: PrefixTrie::new(),
            last_modified: None,
            priority_boost: 10000.0, // 自訂辭典預設獲得極高權重，保證排在第 1 位
        }
    }
}

impl CustomDictionary {
    pub fn new() -> Self {
        Self::default()
    }

    /// 設定辭典檔案路徑並首次載入
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let p = path.as_ref().to_path_buf();
        self.file_path = Some(p.clone());
        self.reload_if_needed()?;
        Ok(())
    }

    /// 檢查檔案修改時間，若有更新則重新解析載入 (Hot-Reload)
    pub fn reload_if_needed(&mut self) -> std::io::Result<bool> {
        let path = match &self.file_path {
            Some(p) => p,
            None => return Ok(false),
        };

        if !path.exists() {
            return Ok(false);
        }

        let metadata = fs::metadata(path)?;
        let modified = metadata.modified()?;

        if self.last_modified.map_or(true, |m| modified > m) {
            let content = fs::read_to_string(path)?;
            self.parse_content(&content);
            self.last_modified = Some(modified);
            return Ok(true);
        }

        Ok(false)
    }

    /// 解析辭典文字內容
    /// 支援格式（Tab 或空格分隔）：
    /// `詞條 \t 注音或英文字串 \t 權重(可選)`
    /// 例如：
    /// `深度學習	ㄕㄣ ㄉㄨˋ ㄒㄩㄝˊ ㄒㄧˊ	2000`
    /// `Jerry	Jerry	2000`
    pub fn parse_content(&mut self, content: &str) {
        let mut new_trie = PrefixTrie::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split(|c| c == '\t' || c == ',').collect();
            if parts.is_empty() {
                continue;
            }

            let word = parts[0].trim().to_string();
            let key = if parts.len() > 1 {
                // 去除注音中間的空格並轉為小寫比對，讓英文字串不分大小寫皆可命中
                parts[1].trim().replace(' ', "").to_lowercase()
            } else {
                word.to_lowercase()
            };

            let score = if parts.len() > 2 {
                parts[2].trim().parse::<f64>().unwrap_or(self.priority_boost)
            } else {
                self.priority_boost
            };

            new_trie.insert(&key, word, score);
        }

        self.trie = new_trie;
    }

    /// 搜尋符合的自訂詞條
    pub fn query(&self, key: &str) -> Vec<(String, f64)> {
        self.trie.find_exact(key)
    }
}
