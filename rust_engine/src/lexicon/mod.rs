pub mod custom_dict;
pub mod trie;

pub use custom_dict::CustomDictionary;
pub use trie::PrefixTrie;

/// 內建基礎詞庫建構器
pub struct BuiltinLexicon;

impl BuiltinLexicon {
    /// 載入常用中文詞條（注音 -> 中文詞條，包含單字與常見詞組）
    pub fn build_chinese_lexicon() -> PrefixTrie {
        let mut trie = PrefixTrie::new();

        let raw = include_str!("../../data/lexicon.txt");
        for line in raw.lines() {
            let mut parts = line.split('\t');
            if let (Some(zhuyin), Some(word), Some(score_str)) = (parts.next(), parts.next(), parts.next()) {
                if let Ok(score) = score_str.parse::<f64>() {
                    trie.insert(zhuyin, word.to_string(), score);
                }
            }
        }

        trie
    }

    /// 載入常用英文詞庫（支援中英混打補全與行內半透明 Tab 自動補字）
    pub fn build_english_lexicon() -> PrefixTrie {
        let mut trie = PrefixTrie::new();

        let raw = include_str!("../../data/english_lexicon.txt");
        for line in raw.lines() {
            let mut parts = line.split('\t');
            if let (Some(word), Some(score_str)) = (parts.next(), parts.next()) {
                if let Ok(score) = score_str.parse::<f64>() {
                    trie.insert(word, word.to_string(), score);
                }
            }
        }

        trie
    }
}
