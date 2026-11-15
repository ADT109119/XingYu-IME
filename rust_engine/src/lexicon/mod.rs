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

    /// 載入常用英文詞庫（支援中英混打補全）
    pub fn build_english_lexicon() -> PrefixTrie {
        let mut trie = PrefixTrie::new();

        let common_words: &[(&str, f64)] = &[
            ("the", 1000.0), ("be", 900.0), ("to", 900.0), ("of", 900.0),
            ("and", 900.0), ("a", 1000.0), ("in", 850.0), ("that", 800.0),
            ("have", 800.0), ("i", 900.0), ("it", 850.0), ("for", 800.0),
            ("not", 800.0), ("on", 800.0), ("with", 750.0), ("he", 700.0),
            ("as", 700.0), ("you", 850.0), ("do", 750.0), ("at", 750.0),
            ("this", 750.0), ("but", 700.0), ("his", 700.0), ("by", 700.0),
            ("from", 700.0), ("they", 700.0), ("we", 750.0), ("say", 650.0),
            ("her", 650.0), ("she", 650.0), ("or", 700.0), ("an", 700.0),
            ("will", 700.0), ("my", 700.0), ("one", 700.0), ("all", 700.0),
            ("would", 650.0), ("there", 650.0), ("their", 650.0), ("what", 700.0),
            ("so", 750.0), ("up", 700.0), ("out", 700.0), ("if", 750.0),
            ("about", 750.0), ("who", 700.0), ("get", 750.0), ("which", 700.0),
            ("go", 750.0), ("me", 750.0), ("when", 700.0), ("make", 750.0),
            ("can", 750.0), ("like", 750.0), ("time", 750.0), ("just", 750.0),
            ("know", 750.0), ("take", 700.0), ("people", 700.0), ("into", 700.0),
            ("year", 700.0), ("your", 750.0), ("some", 700.0), ("could", 700.0),
            ("them", 700.0), ("see", 700.0), ("other", 700.0), ("than", 700.0),
            ("then", 700.0), ("now", 750.0), ("look", 700.0), ("only", 700.0),
            ("come", 700.0), ("its", 700.0), ("over", 700.0), ("think", 700.0),
            ("also", 700.0), ("back", 700.0), ("after", 700.0), ("use", 750.0),
            ("two", 700.0), ("how", 750.0), ("our", 700.0), ("work", 750.0),
            ("first", 750.0), ("well", 750.0), ("way", 700.0), ("even", 700.0),
            ("new", 750.0), ("want", 700.0), ("because", 700.0), ("any", 700.0),
            ("these", 700.0), ("give", 700.0), ("day", 750.0), ("most", 700.0),
            ("thank", 800.0), ("thanks", 850.0), ("please", 800.0), ("sorry", 800.0),
            // 開發與現代常用詞
            ("apple", 800.0), ("swift", 850.0), ("swiftui", 800.0), ("rust", 850.0),
            ("cargo", 750.0), ("mac", 800.0), ("macos", 850.0), ("xcode", 800.0),
            ("code", 750.0), ("coding", 700.0), ("git", 800.0), ("github", 850.0),
            ("commit", 750.0), ("push", 750.0), ("pull", 750.0), ("merge", 750.0),
            ("branch", 750.0), ("issue", 750.0), ("fix", 750.0), ("bug", 800.0),
            ("test", 750.0), ("testing", 700.0), ("build", 750.0), ("release", 750.0),
            ("run", 700.0), ("debug", 750.0), ("app", 800.0), ("application", 700.0),
            ("api", 800.0), ("sdk", 750.0), ("cli", 750.0), ("gui", 750.0),
            ("key", 700.0), ("keyboard", 750.0), ("input", 750.0), ("output", 750.0),
            ("method", 700.0), ("ime", 800.0), ("hello", 800.0), ("world", 750.0),
            ("smart", 800.0), ("asus", 800.0), ("system", 750.0), ("model", 750.0),
            ("update", 750.0), ("upgrade", 700.0), ("project", 750.0), ("user", 750.0),
            ("data", 750.0), ("database", 750.0), ("sqlite", 750.0), ("json", 800.0),
            ("file", 750.0), ("folder", 750.0), ("path", 700.0), ("url", 700.0),
            ("http", 750.0), ("https", 750.0), ("server", 750.0), ("client", 750.0),
            ("frontend", 750.0), ("backend", 750.0), ("engine", 750.0), ("config", 750.0),
            ("setting", 700.0), ("settings", 750.0), ("option", 700.0), ("default", 750.0),
            ("error", 750.0), ("warning", 750.0), ("check", 750.0), ("search", 750.0),
            ("google", 800.0), ("gemini", 800.0), ("claude", 800.0), ("openai", 800.0),
            ("python", 800.0), ("typescript", 750.0), ("javascript", 750.0), ("docker", 750.0),
            ("linux", 800.0), ("windows", 750.0), ("iphone", 750.0), ("ipad", 750.0),
            ("email", 750.0), ("password", 750.0), ("login", 750.0), ("token", 750.0),
            ("ok", 850.0), ("yes", 800.0), ("no", 800.0), ("good", 800.0), ("great", 750.0),
        ];

        for (word, score) in common_words {
            trie.insert(word, word.to_string(), *score);
        }

        trie
    }
}
