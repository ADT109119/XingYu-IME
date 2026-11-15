use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// 單詞學習紀錄項目
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearnedUnigram {
    pub word: String,
    pub count: i64,
    pub last_used: i64,
}

/// 詞彙二元搭配學習紀錄項目
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearnedBigram {
    pub prev_word: String,
    pub next_word: String,
    pub count: i64,
    pub last_used: i64,
}

/// 學習詞袋列表快照
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearningEntriesSnapshot {
    pub unigrams: Vec<LearnedUnigram>,
    pub bigrams: Vec<LearnedBigram>,
}

/// 使用者詞袋與自適應學習資料庫 (SQLite 儲存)
pub struct UserAdaptiveDb {
    conn: Option<Mutex<Connection>>,
}

impl UserAdaptiveDb {
    /// 建立未初始化的資料庫容器
    pub fn new() -> Self {
        Self { conn: None }
    }

    /// 建立內存記憶體資料庫（適合單元測試）
    pub fn in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Some(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    /// 從指定路徑載入或建立本機 SQLite 資料庫
    pub fn open<P: AsRef<Path>>(path: P) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        // 啟用 WAL 模式，提升並發讀寫效能
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        let db = Self {
            conn: Some(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    /// 初始化資料庫表結構
    fn init_schema(&self) -> rusqlite::Result<()> {
        if let Some(ref mutex) = self.conn {
            let conn = mutex.lock().unwrap();
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS user_unigrams (
                    word TEXT PRIMARY KEY,
                    count INTEGER NOT NULL DEFAULT 1,
                    last_used INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS user_bigrams (
                    prev_word TEXT NOT NULL,
                    next_word TEXT NOT NULL,
                    count INTEGER NOT NULL DEFAULT 1,
                    last_used INTEGER NOT NULL,
                    PRIMARY KEY (prev_word, next_word)
                );
                "#,
            )?;
        }
        Ok(())
    }

    /// 當使用者確認輸出文字時（Commit Event），更新詞頻與詞袋搭配關係
    pub fn record_commit(&self, words: &[&str]) {
        self.record_commit_with_prev(None, words);
    }

    /// 記錄提交詞序列，並選擇性將前一次提交的尾詞串接為跨句 Bigram（不重複累加前詞 Unigram）
    pub fn record_commit_with_prev(&self, prev_word: Option<&str>, words: &[&str]) {
        if words.is_empty() {
            return;
        }
        let Some(ref mutex) = self.conn else { return };
        let mut conn = mutex.lock().unwrap();
        let tx = match conn.transaction() {
            Ok(tx) => tx,
            Err(_) => return,
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        for word in words.iter().copied().filter(|w| !w.is_empty()) {
            let _ = tx.execute(
                r#"
                INSERT INTO user_unigrams (word, count, last_used)
                VALUES (?1, 1, ?2)
                ON CONFLICT(word) DO UPDATE SET
                    count = count + 1,
                    last_used = ?2;
                "#,
                params![word, now],
            );
        }

        let insert_bigram = |prev: &str, next: &str| {
            if prev.is_empty() || next.is_empty() {
                return;
            }
            let _ = tx.execute(
                r#"
                INSERT INTO user_bigrams (prev_word, next_word, count, last_used)
                VALUES (?1, ?2, 1, ?3)
                ON CONFLICT(prev_word, next_word) DO UPDATE SET
                    count = count + 1,
                    last_used = ?3;
                "#,
                params![prev, next, now],
            );
        };

        if let (Some(prev), Some(first)) = (prev_word, words.first().copied()) {
            insert_bigram(prev, first);
        }

        // 紀錄句內二元轉移機率 (Bigram)
        for window in words.windows(2) {
            insert_bigram(window[0], window[1]);
        }

        let _ = tx.commit();
    }

    /// 根據使用者的歷史習慣計算詞條額外加權分
    pub fn get_adaptive_boost(&self, word: &str, prev_word: Option<&str>) -> f64 {
        let Some(ref mutex) = self.conn else { return 0.0 };
        let conn = mutex.lock().unwrap();

        let mut boost = 0.0;

        // 查詢 Unigram 詞頻
        if let Ok(count) = conn.query_row(
            "SELECT count FROM user_unigrams WHERE word = ?1",
            params![word],
            |row| row.get::<_, i64>(0),
        ) {
            // 只要選用過一次即給予顯著基礎加權，讓學習效果立即可見並能蓋過常見靜態分數差距
            boost += 180.0 + (count as f64).min(25.0) * 180.0;
        }

        // 查詢上下文 Bigram 轉移加權
        if let Some(prev) = prev_word {
            if let Ok(bcount) = conn.query_row(
                "SELECT count FROM user_bigrams WHERE prev_word = ?1 AND next_word = ?2",
                params![prev, word],
                |row| row.get::<_, i64>(0),
            ) {
                boost += 220.0 + (bcount as f64).min(15.0) * 320.0;
            }
        }

        boost
    }

    /// 取得資料庫學習統計（Unigram 與 Bigram 詞條數量）
    pub fn get_stats(&self) -> (usize, usize) {
        let Some(ref mutex) = self.conn else { return (0, 0) };
        let conn = mutex.lock().unwrap();

        let unigrams: usize = conn
            .query_row("SELECT COUNT(*) FROM user_unigrams", [], |row| row.get(0))
            .unwrap_or(0);
        let bigrams: usize = conn
            .query_row("SELECT COUNT(*) FROM user_bigrams", [], |row| row.get(0))
            .unwrap_or(0);

        (unigrams, bigrams)
    }

    /// 列出目前學習到的單詞（預設依最近使用時間與次數排序）
    pub fn list_unigrams(&self, limit: usize) -> Vec<LearnedUnigram> {
        let Some(ref mutex) = self.conn else { return Vec::new() };
        let conn = mutex.lock().unwrap();
        let row_limit = limit.max(1) as i64;
        let Ok(mut stmt) = conn.prepare(
            "SELECT word, count, last_used
             FROM user_unigrams
             ORDER BY last_used DESC, count DESC, word ASC
             LIMIT ?1",
        ) else {
            return Vec::new();
        };

        let Ok(rows) = stmt.query_map(params![row_limit], |row| {
            Ok(LearnedUnigram {
                word: row.get(0)?,
                count: row.get(1)?,
                last_used: row.get(2)?,
            })
        }) else {
            return Vec::new();
        };

        rows.filter_map(Result::ok).collect()
    }

    /// 列出目前學習到的詞彙二元搭配（預設依最近使用時間與次數排序）
    pub fn list_bigrams(&self, limit: usize) -> Vec<LearnedBigram> {
        let Some(ref mutex) = self.conn else { return Vec::new() };
        let conn = mutex.lock().unwrap();
        let row_limit = limit.max(1) as i64;
        let Ok(mut stmt) = conn.prepare(
            "SELECT prev_word, next_word, count, last_used
             FROM user_bigrams
             ORDER BY last_used DESC, count DESC, prev_word ASC, next_word ASC
             LIMIT ?1",
        ) else {
            return Vec::new();
        };

        let Ok(rows) = stmt.query_map(params![row_limit], |row| {
            Ok(LearnedBigram {
                prev_word: row.get(0)?,
                next_word: row.get(1)?,
                count: row.get(2)?,
                last_used: row.get(3)?,
            })
        }) else {
            return Vec::new();
        };

        rows.filter_map(Result::ok).collect()
    }

    /// 取得學習詞袋完整清單快照
    pub fn get_entries_snapshot(&self, limit: usize) -> LearningEntriesSnapshot {
        LearningEntriesSnapshot {
            unigrams: self.list_unigrams(limit),
            bigrams: self.list_bigrams(limit),
        }
    }

    /// 刪除指定單詞的學習紀錄（並一併清理包含該詞的二元搭配紀錄）
    pub fn delete_unigram(&self, word: &str) -> rusqlite::Result<bool> {
        let Some(ref mutex) = self.conn else { return Ok(false) };
        let mut conn = mutex.lock().unwrap();
        let tx = conn.transaction()?;
        let affected_unigrams = tx.execute(
            "DELETE FROM user_unigrams WHERE word = ?1",
            params![word],
        )?;
        let affected_bigrams = tx.execute(
            "DELETE FROM user_bigrams WHERE prev_word = ?1 OR next_word = ?1",
            params![word],
        )?;
        tx.commit()?;
        Ok(affected_unigrams > 0 || affected_bigrams > 0)
    }

    /// 刪除指定的二元搭配學習紀錄
    pub fn delete_bigram(&self, prev_word: &str, next_word: &str) -> rusqlite::Result<bool> {
        let Some(ref mutex) = self.conn else { return Ok(false) };
        let conn = mutex.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM user_bigrams WHERE prev_word = ?1 AND next_word = ?2",
            params![prev_word, next_word],
        )?;
        Ok(affected > 0)
    }

    /// 清空所有使用者學習詞庫與習慣
    pub fn clear_all(&self) -> rusqlite::Result<()> {
        let Some(ref mutex) = self.conn else { return Ok(()) };
        let conn = mutex.lock().unwrap();
        conn.execute_batch("DELETE FROM user_unigrams; DELETE FROM user_bigrams; VACUUM;")?;
        Ok(())
    }
}
