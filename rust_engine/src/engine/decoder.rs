use crate::engine::user_db::UserAdaptiveDb;
use crate::keyboard::layout::{KeyMapping, KeyboardLayout, Tone, ZhuyinSymbol};
use crate::keyboard::{create_layout, LayoutType};
use crate::lexicon::{BuiltinLexicon, CustomDictionary, PrefixTrie};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

static BUILTIN_CHINESE_LEXICON: Lazy<Arc<PrefixTrie>> =
    Lazy::new(|| Arc::new(BuiltinLexicon::build_chinese_lexicon()));
static BUILTIN_ENGLISH_LEXICON: Lazy<Arc<PrefixTrie>> =
    Lazy::new(|| Arc::new(BuiltinLexicon::build_english_lexicon()));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateType {
    Custom,
    Chinese,
    English,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub text: String,
    pub score: f64,
    pub candidate_type: CandidateType,
    pub annotation: Option<String>,
    #[serde(default)]
    pub span_start: usize,
    #[serde(default)]
    pub span_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionSnapshot {
    pub preedit: String,
    pub cursor_utf16: usize,
    pub selection_len_utf16: usize,
    pub is_valid_zhuyin: bool,
    pub has_uppercase: bool,
    pub has_resolved_chinese: bool,
    pub should_space_act_as_tone: bool,
    pub segment_count: usize,
    #[serde(default)]
    pub syllable_count: usize,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Clone)]
struct SyllableBucket {
    initial: Option<ZhuyinSymbol>,
    medial: Option<ZhuyinSymbol>,
    final_sym: Option<ZhuyinSymbol>,
    tone: Option<Tone>,
    raw_start: usize,
    raw_end: usize,
    has_explicit_tone: bool,
    is_valid: bool,
}

impl SyllableBucket {
    fn new(raw_start: usize) -> Self {
        Self {
            initial: None,
            medial: None,
            final_sym: None,
            tone: None,
            raw_start,
            raw_end: raw_start,
            has_explicit_tone: false,
            is_valid: true,
        }
    }

    fn invalid_raw(raw_start: usize, raw_end: usize) -> Self {
        Self {
            initial: None,
            medial: None,
            final_sym: None,
            tone: None,
            raw_start,
            raw_end,
            has_explicit_tone: true,
            is_valid: false,
        }
    }

    fn is_empty(&self) -> bool {
        self.initial.is_none()
            && self.medial.is_none()
            && self.final_sym.is_none()
            && self.tone.is_none()
    }

    fn has_phonemes(&self) -> bool {
        self.initial.is_some() || self.medial.is_some() || self.final_sym.is_some()
    }

    fn to_canonical_string(&self) -> String {
        let mut s = String::new();
        if let Some(i) = self.initial {
            s.push(i.as_char());
        }
        if let Some(m) = self.medial {
            s.push(m.as_char());
        }
        if let Some(f) = self.final_sym {
            s.push(f.as_char());
        }
        if let Some(t) = self.tone {
            let tc = match t {
                Tone::First => "",
                Tone::Second => "ˊ",
                Tone::Third => "ˇ",
                Tone::Fourth => "ˋ",
                Tone::Fifth => "˙",
            };
            s.push_str(tc);
        }
        s
    }
}

#[derive(Debug, Clone)]
struct DecodedSegment {
    start_syl: usize,
    end_syl: usize,
    word: String,
    _zhuyin: String,
}

fn can_initial_stand_alone(sym: ZhuyinSymbol) -> bool {
    matches!(
        sym,
        ZhuyinSymbol::Zh
            | ZhuyinSymbol::Ch
            | ZhuyinSymbol::Sh
            | ZhuyinSymbol::R
            | ZhuyinSymbol::Z
            | ZhuyinSymbol::C
            | ZhuyinSymbol::S
    )
}

pub struct DualPathDecoder {
    layout: Box<dyn KeyboardLayout>,
    chinese_lexicon: Arc<PrefixTrie>,
    english_lexicon: Arc<PrefixTrie>,
    pub custom_dict: CustomDictionary,
    pub user_db: UserAdaptiveDb,

    // 輸入中狀態
    raw_buffer: String,
    last_committed_word: Option<String>,
    /// 使用者於目前組字句中手動鎖定的詞組區段: (start_syl, end_syl, word)
    user_overrides: Vec<(usize, usize, String)>,
    /// 選詞 / 重選時的活躍音節索引
    active_cursor_syl: Option<usize>,
}

impl DualPathDecoder {
    pub fn new() -> Self {
        Self {
            layout: create_layout(LayoutType::DaChen),
            chinese_lexicon: Arc::clone(&BUILTIN_CHINESE_LEXICON),
            english_lexicon: Arc::clone(&BUILTIN_ENGLISH_LEXICON),
            custom_dict: CustomDictionary::new(),
            user_db: UserAdaptiveDb::new(),
            raw_buffer: String::new(),
            last_committed_word: None,
            user_overrides: Vec::new(),
            active_cursor_syl: None,
        }
    }

    /// 設定鍵盤配置
    pub fn set_layout(&mut self, layout_type: LayoutType) {
        self.layout = create_layout(layout_type);
    }

    /// 清空輸入暫存區與選詞狀態
    pub fn clear(&mut self) {
        self.raw_buffer.clear();
        self.user_overrides.clear();
        self.active_cursor_syl = None;
    }

    /// 取得目前 raw 輸入緩衝區
    pub fn raw_buffer(&self) -> &str {
        &self.raw_buffer
    }

    /// 輸入一個按鍵字元
    pub fn push_char(&mut self, ch: char) {
        self.raw_buffer.push(ch);
        self.active_cursor_syl = None;
        let _ = self.custom_dict.reload_if_needed();
    }

    /// 刪除最後一個按鍵（Backspace）
    pub fn pop_char(&mut self) -> Option<char> {
        let popped = self.raw_buffer.pop();
        self.active_cursor_syl = None;
        let (syllables, _) = self.parse_zhuyin_segments(&self.raw_buffer);
        let syl_count = syllables.len();
        self.user_overrides
            .retain(|(start, end, _)| *end <= syl_count && *start < *end);
        popped
    }

    /// 檢查是否有輸入暫存
    pub fn is_empty(&self) -> bool {
        self.raw_buffer.is_empty()
    }

    /// 取消選詞游標，返回末尾繼續輸入
    pub fn clear_cursor(&mut self) {
        self.active_cursor_syl = None;
    }

    /// 檢查目前是否處於句中游標導覽 / 重選狀態
    pub fn has_active_cursor(&self) -> bool {
        self.active_cursor_syl.is_some()
    }

    /// 向左移動選詞區段游標（進行詞彙重選）
    pub fn move_cursor_left(&mut self) -> bool {
        let (segments, _, _, is_valid) = self.build_sentence_segments();
        if !is_valid || segments.is_empty() {
            return false;
        }
        match self.active_cursor_syl {
            None => {
                self.active_cursor_syl = Some(segments.last().unwrap().start_syl);
                true
            }
            Some(curr_syl) => {
                if let Some(pos) = segments
                    .iter()
                    .position(|s| curr_syl >= s.start_syl && curr_syl < s.end_syl)
                {
                    if pos > 0 {
                        self.active_cursor_syl = Some(segments[pos - 1].start_syl);
                        return true;
                    }
                }
                false
            }
        }
    }

    /// 向右移動選詞區段游標
    pub fn move_cursor_right(&mut self) -> bool {
        let (segments, _, _, is_valid) = self.build_sentence_segments();
        if !is_valid || segments.is_empty() {
            return false;
        }
        match self.active_cursor_syl {
            None => false,
            Some(curr_syl) => {
                if let Some(pos) = segments
                    .iter()
                    .position(|s| curr_syl >= s.start_syl && curr_syl < s.end_syl)
                {
                    if pos + 1 < segments.len() {
                        self.active_cursor_syl = Some(segments[pos + 1].start_syl);
                        return true;
                    }
                }
                self.active_cursor_syl = None;
                true
            }
        }
    }

    /// 在組字緩衝內選取第 `index` 個候選詞替換當前區段
    /// 若成功更新內部區段且仍有後續詞組可繼續編輯，返回 true；若整句只有此一詞組或應直接送出，可由前端決定提交
    pub fn apply_candidate_selection(&mut self, index: usize) -> Option<(Candidate, usize)> {
        let snap = self.get_composition_snapshot(256);
        let chosen = snap.candidates.get(index)?.clone();

        if chosen.candidate_type == CandidateType::English || chosen.span_end <= chosen.span_start {
            return Some((chosen, snap.segment_count));
        }

        // 移除與此新區段重疊的舊手動鎖定
        self.user_overrides
            .retain(|(s, e, _)| *e <= chosen.span_start || *s >= chosen.span_end);
        self.user_overrides
            .push((chosen.span_start, chosen.span_end, chosen.text.clone()));

        // 若正處於區段重選游標模式，或選詞後仍有剩餘音節尚未涵蓋（例如把 "第一" 改選為單字 "地"），推進游標至下一個區段
        let (segments, syllables, _, _) = self.build_sentence_segments();
        if self.active_cursor_syl.is_some() || chosen.span_end < syllables.len() {
            if let Some(next_seg) = segments.iter().find(|s| s.start_syl >= chosen.span_end) {
                self.active_cursor_syl = Some(next_seg.start_syl);
            } else {
                self.active_cursor_syl = None;
            }
        }

        Some((chosen, snap.segment_count))
    }

    /// 產生行內預編輯文字（Pre-edit marked text）
    pub fn get_preedit_string(&self) -> String {
        self.get_composition_snapshot(1).preedit
    }

    /// 取得完整組字狀態快照（含斷詞結果、游標選取範圍與候選詞）
    pub fn get_composition_snapshot(&self, limit: usize) -> CompositionSnapshot {
        if self.raw_buffer.is_empty() {
            return CompositionSnapshot {
                preedit: String::new(),
                cursor_utf16: 0,
                selection_len_utf16: 0,
                is_valid_zhuyin: false,
                has_uppercase: false,
                has_resolved_chinese: false,
                should_space_act_as_tone: false,
                segment_count: 0,
                syllable_count: 0,
                candidates: Vec::new(),
            };
        }

        let has_uppercase = self.raw_buffer.chars().any(|c| c.is_uppercase());
        let (segments, syllables, resolved_len, is_valid_zhuyin) = self.build_sentence_segments();
        let trailing_raw = if resolved_len < syllables.len() {
            let start_byte = syllables[resolved_len].raw_start;
            &self.raw_buffer[start_byte..]
        } else if !is_valid_zhuyin {
            self.raw_buffer.as_str()
        } else {
            ""
        };

        // 組合行內顯示字串與計算高亮區段 UTF-16 偏移量
        let mut preedit = String::new();
        let mut cursor_utf16 = 0usize;
        let mut selection_len_utf16 = 0usize;

        if is_valid_zhuyin && !segments.is_empty() {
            for seg in &segments {
                let is_active = self
                    .active_cursor_syl
                    .map_or(false, |c| c >= seg.start_syl && c < seg.end_syl);
                if is_active {
                    cursor_utf16 = preedit.encode_utf16().count();
                    selection_len_utf16 = seg.word.encode_utf16().count();
                }
                preedit.push_str(&seg.word);
            }
            if !trailing_raw.is_empty() {
                preedit.push_str(trailing_raw);
            }
            if self.active_cursor_syl.is_none() {
                cursor_utf16 = preedit.encode_utf16().count();
                selection_len_utf16 = 0;
            }
        } else {
            preedit = self.raw_buffer.clone();
            cursor_utf16 = preedit.encode_utf16().count();
        }

        let candidates = self.build_candidates_internal(
            limit,
            &segments,
            &syllables,
            resolved_len,
            is_valid_zhuyin,
            has_uppercase,
        );

        let top_is_english = candidates
            .first()
            .map_or(false, |c| c.candidate_type == CandidateType::English);

        // 判斷空白鍵是否應作為「一聲（陰平）」：
        // 條件：沒有大寫、沒有進入游標重選、目前尾端音節為有效未標調注音且尚未被使用者手動選字覆寫、且英文並非絕對優先
        let has_override_at_end = self
            .user_overrides
            .iter()
            .any(|(_, end, _)| *end == syllables.len());
        let should_space_act_as_tone = is_valid_zhuyin
            && !has_uppercase
            && self.active_cursor_syl.is_none()
            && !top_is_english
            && !has_override_at_end
            && syllables
                .last()
                .map_or(false, |s| s.is_valid && !s.has_explicit_tone && s.has_phonemes());

        CompositionSnapshot {
            preedit,
            cursor_utf16,
            selection_len_utf16,
            is_valid_zhuyin,
            has_uppercase,
            has_resolved_chinese: !segments.is_empty(),
            should_space_act_as_tone,
            segment_count: segments.len(),
            syllable_count: syllables.len(),
            candidates,
        }
    }

    /// 雙軌解碼：回傳候選詞清單
    pub fn get_candidates(&self, limit: usize) -> Vec<Candidate> {
        self.get_composition_snapshot(limit).candidates
    }

    fn build_candidates_internal(
        &self,
        limit: usize,
        segments: &[DecodedSegment],
        syllables: &[SyllableBucket],
        resolved_len: usize,
        is_valid_zhuyin: bool,
        has_uppercase: bool,
    ) -> Vec<Candidate> {
        let mut candidates: Vec<Candidate> = Vec::new();
        let prev_word = self.last_committed_word.as_deref();
        let raw_lower = self.raw_buffer.to_ascii_lowercase();
        let starts_with_uppercase = self
            .raw_buffer
            .chars()
            .next()
            .map_or(false, |c| c.is_uppercase());

        // 1. 若處於中文句子編輯/選詞模式且已有有效注音音節
        if is_valid_zhuyin && !syllables.is_empty() {
            // 決定要列舉候選詞的區段目標 (span_start, span_end)
            let (target_start, target_end) = if let Some(curr_syl) = self.active_cursor_syl {
                if let Some(seg) = segments
                    .iter()
                    .find(|s| curr_syl >= s.start_syl && curr_syl < s.end_syl)
                {
                    (seg.start_syl, seg.end_syl)
                } else {
                    (0, syllables.len())
                }
            } else if !segments.is_empty() && resolved_len == syllables.len() {
                let last_seg = segments.last().unwrap();
                (last_seg.start_syl, last_seg.end_syl)
            } else {
                // 有未結算尾端音節（例如 "fu" 或 "你c" 或 "w8"）
                (resolved_len, syllables.len())
            };

            let max_end = syllables.len();

            // 1A. 若為單一詞彙或鎖定目標區段，優先列出目標區段精確匹配詞條
            let target_all_valid = target_end > target_start
                && syllables[target_start..target_end]
                    .iter()
                    .all(|s| s.is_valid);
            if target_all_valid {
                let target_zy: String = syllables[target_start..target_end]
                    .iter()
                    .map(|s| s.to_canonical_string())
                    .collect();

                // 若此區段已有使用者手動選字覆寫，將該選字置於候選第 1 位與組字畫面一致
                if let Some((_, _, override_word)) = self
                    .user_overrides
                    .iter()
                    .find(|(s, e, _)| *s == target_start && *e == target_end)
                {
                    candidates.push(Candidate {
                        text: override_word.clone(),
                        score: 200_000.0,
                        candidate_type: CandidateType::Chinese,
                        annotation: Some(target_zy.clone()),
                        span_start: target_start,
                        span_end: target_end,
                    });
                }

                for (word, score) in self.custom_dict.query(&target_zy) {
                    if !candidates.iter().any(|c| c.text == word) {
                        candidates.push(Candidate {
                            text: word,
                            score: score + 10000.0,
                            candidate_type: CandidateType::Custom,
                            annotation: Some("自訂".to_string()),
                            span_start: target_start,
                            span_end: target_end,
                        });
                    }
                }

                for (word, base_score) in self.chinese_lexicon.find_exact(&target_zy) {
                    let adaptive_boost = self.user_db.get_adaptive_boost(&word, prev_word);
                    if !candidates.iter().any(|c| c.text == word) {
                        candidates.push(Candidate {
                            text: word,
                            score: base_score + adaptive_boost + 3000.0,
                            candidate_type: CandidateType::Chinese,
                            annotation: Some(target_zy.clone()),
                            span_start: target_start,
                            span_end: target_end,
                        });
                    }
                }

                // 1B. 若在選詞游標模式下（或有多音節），也同時提供更長詞彙（如 3 字詞）或縮短為 1 字詞的選項，方便使用者自由分詞/併詞重選！
                let alt_max = (target_start + 5).min(max_end);
                for end_idx in (target_start + 1..=alt_max).rev() {
                    if end_idx == target_end
                        || syllables[target_start..end_idx].iter().any(|s| !s.is_valid)
                    {
                        continue;
                    }
                    let alt_zy: String = syllables[target_start..end_idx]
                        .iter()
                        .map(|s| s.to_canonical_string())
                        .collect();
                    let len_bonus = if end_idx - target_start >= 2 {
                        (end_idx - target_start - 1) as f64 * 400.0
                    } else {
                        -200.0
                    };
                    for (word, base_score) in self.chinese_lexicon.find_exact(&alt_zy).into_iter().take(30) {
                        let adaptive_boost = self.user_db.get_adaptive_boost(&word, prev_word);
                        if !candidates.iter().any(|c| c.text == word) {
                            candidates.push(Candidate {
                                text: word,
                                score: base_score + adaptive_boost + len_bonus + 1500.0,
                                candidate_type: CandidateType::Chinese,
                                annotation: Some(alt_zy.clone()),
                                span_start: target_start,
                                span_end: end_idx,
                            });
                        }
                    }
                }

                // 1C. 若處於一般輸入（非游標重選），補上以此音節開頭的前綴自動完成詞（例如輸入 "ㄅㄨˋㄓ" 即可提前看到 "不知道" 3 字詞）
                if self.active_cursor_syl.is_none() {
                    for (word, base_score) in self.chinese_lexicon.find_prefix(&target_zy, limit) {
                        let adaptive_boost = self.user_db.get_adaptive_boost(&word, prev_word);
                        if !candidates.iter().any(|c| c.text == word) {
                            candidates.push(Candidate {
                                text: word,
                                score: base_score + adaptive_boost - 1000.0,
                                candidate_type: CandidateType::Chinese,
                                annotation: Some(target_zy.clone()),
                                span_start: target_start,
                                span_end: target_end,
                            });
                        }
                    }
                }
            }
        }

        // 2. 自訂辭典英文鍵值匹配
        for (word, score) in self.custom_dict.query(&raw_lower) {
            if !candidates.iter().any(|c| c.text == word) {
                candidates.push(Candidate {
                    text: word,
                    score: score + 10000.0,
                    candidate_type: CandidateType::Custom,
                    annotation: Some("自訂".to_string()),
                    span_start: 0,
                    span_end: 0,
                });
            }
        }

        // 3. 英文路徑解碼（中英混打核心）
        let english_matches = self.english_lexicon.find_prefix(&raw_lower, limit);
        for (word, base_score) in english_matches {
            let mut score = base_score;

            if has_uppercase {
                score += 4000.0;
            } else if starts_with_uppercase {
                score += 3000.0;
            }

            if !is_valid_zhuyin {
                score += 3000.0;
            }

            if word == raw_lower {
                score += 1200.0;
            }

            let adaptive_boost = self.user_db.get_adaptive_boost(&word, prev_word);
            score += adaptive_boost;

            let display_text = if starts_with_uppercase {
                let mut c = word.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            } else {
                word
            };

            if !candidates.iter().any(|c| c.text == display_text) {
                candidates.push(Candidate {
                    text: display_text,
                    score,
                    candidate_type: CandidateType::English,
                    annotation: Some("英".to_string()),
                    span_start: 0,
                    span_end: 0,
                });
            }
        }

        // 4. 保底原始英文字串
        if !candidates.iter().any(|c| c.text == self.raw_buffer) {
            let fallback_score = if has_uppercase {
                5000.0
            } else if !is_valid_zhuyin {
                800.0
            } else {
                10.0
            };
            candidates.push(Candidate {
                text: self.raw_buffer.clone(),
                score: fallback_score,
                candidate_type: CandidateType::English,
                annotation: Some("英".to_string()),
                span_start: 0,
                span_end: 0,
            });
        }

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(limit);
        candidates
    }

    /// 記錄使用者提交結果並重置輸入緩衝
    pub fn commit_text(&mut self, text: &str) {
        self.last_committed_word = Some(text.to_string());
        self.user_db.record_commit(&[text]);
        self.clear();
    }

    /// 以 Viterbi DP 演算法對音節序列進行最佳斷詞與組字
    fn build_sentence_segments(&self) -> (Vec<DecodedSegment>, Vec<SyllableBucket>, usize, bool) {
        let (syllables, is_valid) = self.parse_zhuyin_segments(&self.raw_buffer);
        let n = syllables.len();
        if !is_valid || n == 0 {
            return (Vec::new(), syllables, 0, is_valid);
        }

        // 若輸入為 >= 3 字元的純英文字母且無任何聲調、無多音節中文詞組，且符合英文單字詞庫，判定為英文優先
        if self.raw_buffer.len() >= 3 && self.raw_buffer.chars().all(|c| c.is_ascii_alphabetic()) {
            let raw_lower = self.raw_buffer.to_ascii_lowercase();
            let has_any_multi_phrase = (0..n).any(|i| {
                ((i + 2)..=n.min(i + 8)).any(|j| {
                    let zy: String = syllables[i..j]
                        .iter()
                        .map(|s| s.to_canonical_string())
                        .collect();
                    !self.chinese_lexicon.find_exact(&zy).is_empty()
                        || !self.custom_dict.query(&zy).is_empty()
                })
            });
            if !has_any_multi_phrase && !self.english_lexicon.find_exact(&raw_lower).is_empty() {
                return (Vec::new(), syllables, 0, false);
            }
        }

        // 計算應轉換為中文的已結算音節長度 resolved_len：
        // 1. 最後一個音節有明確聲調 (3/4/6/7/空白) 或本身為錯字孤立區段，則整串皆已結算 (resolved_len = n)
        // 2. 否則若有長度 >= 2 的有效多音節詞組剛好收尾於 n（例如 "今天" ㄐㄧㄣㄊㄧㄢ），也視為已結算 (resolved_len = n)
        // 3. 若尾端音節已被使用者手動鎖定覆寫，也視為已結算 (resolved_len = n)
        // 4. 否則前面已閉合的音節 (0..n-1) 已結算，僅保留最後一個正在輸入中的音節為英文原字
        let last_has_tone = syllables[n - 1].has_explicit_tone;
        let last_is_invalid = !syllables[n - 1].is_valid;
        let has_multi_phrase_ending = if n >= 2 {
            let start_min = n.saturating_sub(8);
            (start_min..n - 1).any(|start| {
                if syllables[start..n].iter().any(|s| !s.is_valid) {
                    return false;
                }
                let zy: String = syllables[start..n]
                    .iter()
                    .map(|s| s.to_canonical_string())
                    .collect();
                !self.chinese_lexicon.find_exact(&zy).is_empty()
                    || !self.custom_dict.query(&zy).is_empty()
            })
        } else {
            false
        };

        let has_override_at_end = self.user_overrides.iter().any(|(_, end, _)| *end == n);

        let resolved_len = if last_has_tone || last_is_invalid || has_multi_phrase_ending || has_override_at_end {
            n
        } else {
            n.saturating_sub(1)
        };

        if resolved_len == 0 {
            return (Vec::new(), syllables, 0, is_valid);
        }

        // Viterbi DP: 尋找 0..=resolved_len 最高分斷詞路徑（錯字區段保持原按鍵並隔離不跨越合併）
        let mut dp_score = vec![f64::NEG_INFINITY; resolved_len + 1];
        let mut dp_prev: Vec<Option<(usize, String, String)>> = vec![None; resolved_len + 1];
        dp_score[0] = 0.0;

        for j in 1..=resolved_len {
            if !syllables[j - 1].is_valid {
                if dp_score[j - 1] != f64::NEG_INFINITY {
                    let raw_slice =
                        self.raw_buffer[syllables[j - 1].raw_start..syllables[j - 1].raw_end].to_string();
                    dp_score[j] = dp_score[j - 1];
                    dp_prev[j] = Some((j - 1, raw_slice, String::new()));
                }
                continue;
            }
            let i_min = j.saturating_sub(8);
            for i in (i_min..j).rev() {
                if !syllables[i].is_valid {
                    break;
                }
                if dp_score[i] == f64::NEG_INFINITY {
                    continue;
                }
                let span_zy: String = syllables[i..j]
                    .iter()
                    .map(|s| s.to_canonical_string())
                    .collect();

                if let Some((word, edge_score)) = self.best_word_for_span(i, j, &span_zy) {
                    let cand_score = dp_score[i] + edge_score;
                    if cand_score > dp_score[j] {
                        dp_score[j] = cand_score;
                        dp_prev[j] = Some((i, word, span_zy));
                    }
                }
            }
        }

        // 回溯還原斷詞區段
        let mut segments = Vec::new();
        let mut curr = resolved_len;
        while curr > 0 {
            if let Some((prev_idx, ref word, ref zy)) = dp_prev[curr] {
                segments.push(DecodedSegment {
                    start_syl: prev_idx,
                    end_syl: curr,
                    word: word.clone(),
                    _zhuyin: zy.clone(),
                });
                curr = prev_idx;
            } else {
                // 若某單音節無精確匹配，以原音節文字保底
                let fallback_zy = syllables[curr - 1].to_canonical_string();
                segments.push(DecodedSegment {
                    start_syl: curr - 1,
                    end_syl: curr,
                    word: fallback_zy.clone(),
                    _zhuyin: fallback_zy,
                });
                curr -= 1;
            }
        }
        segments.reverse();

        (segments, syllables, resolved_len, is_valid)
    }

    fn best_word_for_span(&self, start: usize, end: usize, zhuyin: &str) -> Option<(String, f64)> {
        // 1. 使用者手動選詞覆寫（最高優先級）
        if let Some((_, _, word)) = self
            .user_overrides
            .iter()
            .find(|(s, e, _)| *s == start && *e == end)
        {
            return Some((word.clone(), 200_000.0));
        }

        // 2. 自訂辭典匹配
        if let Some((word, score)) = self.custom_dict.query(zhuyin).into_iter().next() {
            return Some((word, score + 10_000.0));
        }

        let prev_word = self.last_committed_word.as_deref();
        let span_len = end - start;
        let phrase_bonus = if span_len >= 2 {
            (span_len - 1) as f64 * 1200.0
        } else {
            0.0
        };

        // 3. 內建大詞庫精確匹配
        let exact = self.chinese_lexicon.find_exact(zhuyin);
        if let Some((best_w, best_s)) = exact
            .into_iter()
            .map(|(w, base)| {
                let adaptive = self.user_db.get_adaptive_boost(&w, prev_word);
                (w, base + adaptive + phrase_bonus)
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            return Some((best_w, best_s));
        }

        // 4. 單音節輕聲或未標調的容錯保底（例如 "ㄉㄜ" 對應 "的"）
        if span_len == 1 {
            if let Some((w, base)) = self.chinese_lexicon.find_prefix(zhuyin, 1).into_iter().next() {
                if w.chars().count() == 1 {
                    return Some((w, base - 300.0));
                }
            }
        }

        None
    }

    /// 智慧注音解析與華語音韻合法性檢查（支援單音節內順序容錯、英數排除、以及長句後段錯字隔離）
    fn parse_zhuyin_segments(&self, input: &str) -> (Vec<SyllableBucket>, bool) {
        if input.is_empty() || input.chars().next().map_or(false, |c| c.is_uppercase()) {
            return (Vec::new(), false);
        }

        let mut syllables: Vec<SyllableBucket> = Vec::new();
        let mut curr: Option<SyllableBucket> = None;

        for (idx, ch) in input.char_indices() {
            let next_idx = idx + ch.len_utf8();
            match self.layout.map_key(ch) {
                KeyMapping::Zhuyin(sym) => {
                    if sym.is_initial() {
                        if let Some(ref b) = curr {
                            if b.initial.is_some() {
                                syllables.push(curr.take().unwrap());
                            }
                        }
                        let bucket = curr.get_or_insert_with(|| SyllableBucket::new(idx));
                        bucket.initial = Some(sym);
                        bucket.raw_end = next_idx;
                    } else if sym.is_medial() {
                        if let Some(ref b) = curr {
                            // 若該音節已有介音，或同時已具備聲母與韻母（如 w8u = 他一），開新音節
                            if b.medial.is_some() || (b.initial.is_some() && b.final_sym.is_some()) {
                                syllables.push(curr.take().unwrap());
                            }
                        }
                        let bucket = curr.get_or_insert_with(|| SyllableBucket::new(idx));
                        bucket.medial = Some(sym);
                        bucket.raw_end = next_idx;
                    } else if sym.is_final() {
                        if let Some(ref b) = curr {
                            if b.final_sym.is_some() {
                                syllables.push(curr.take().unwrap());
                            }
                        }
                        let bucket = curr.get_or_insert_with(|| SyllableBucket::new(idx));
                        bucket.final_sym = Some(sym);
                        bucket.raw_end = next_idx;
                    }
                }
                KeyMapping::Tone(tone) => {
                    if let Some(mut b) = curr.take() {
                        if b.has_phonemes() {
                            b.tone = Some(tone);
                            b.has_explicit_tone = true;
                            b.raw_end = next_idx;
                            syllables.push(b);
                        } else {
                            let start = b.raw_start;
                            syllables.push(SyllableBucket::invalid_raw(start, next_idx));
                        }
                    } else {
                        syllables.push(SyllableBucket::invalid_raw(idx, next_idx));
                    }
                }
                KeyMapping::English(_) | KeyMapping::Ignored => {
                    if let Some(b) = curr.take() {
                        if !b.is_empty() {
                            syllables.push(b);
                        }
                    }
                    syllables.push(SyllableBucket::invalid_raw(idx, next_idx));
                }
            }
        }

        if let Some(b) = curr {
            if !b.is_empty() {
                syllables.push(b);
            }
        }

        if syllables.is_empty() {
            return (syllables, false);
        }

        // 對「已閉合的音節」（0..len-1 或帶有明確聲調者）進行華語音韻合法性檢查
        let n = syllables.len();
        for i in 0..n {
            if !syllables[i].is_valid {
                continue;
            }
            let is_closed = i < n - 1 || syllables[i].has_explicit_tone;
            if is_closed {
                if syllables[i].medial.is_none() && syllables[i].final_sym.is_none() {
                    if !syllables[i].initial.map_or(false, can_initial_stand_alone) {
                        syllables[i].is_valid = false;
                        continue;
                    }
                }
                let canon = syllables[i].to_canonical_string();
                if self.chinese_lexicon.find_prefix(&canon, 1).is_empty()
                    && self.custom_dict.query(&canon).is_empty()
                {
                    syllables[i].is_valid = false;
                }
            }
        }

        // 若存在無效音節/按鍵：
        // - 若前面已有明確解出的中文音節或詞組（帶明確聲調或已命中多音節詞），保留中文模式並僅將錯字部分維持原樣；
        // - 若開頭即無有效中文基礎（例如輸入英文單字 hello, test, keyboard），則整體判定為英文路徑。
        let is_valid = if let Some(first_bad) = syllables.iter().position(|s| !s.is_valid) {
            let has_explicit_tone_prefix = syllables[..first_bad].iter().any(|s| s.has_explicit_tone);
            let is_exact_english_word = input.chars().all(|c| c.is_ascii_alphabetic())
                && !self
                    .english_lexicon
                    .find_exact(&input.to_ascii_lowercase())
                    .is_empty();
            let has_multi_phrase_prefix = !is_exact_english_word
                && first_bad >= 2
                && (0..first_bad).any(|i| {
                    ((i + 2)..=first_bad.min(i + 8)).any(|j| {
                        let zy: String = syllables[i..j]
                            .iter()
                            .map(|s| s.to_canonical_string())
                            .collect();
                        !self.chinese_lexicon.find_exact(&zy).is_empty()
                            || !self.custom_dict.query(&zy).is_empty()
                    })
                });
            has_explicit_tone_prefix || has_multi_phrase_prefix
        } else {
            true
        };

        (syllables, is_valid)
    }
}
