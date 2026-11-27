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
    #[serde(default)]
    pub preview_start_utf16: Option<usize>,
    #[serde(default)]
    pub preview_len_utf16: Option<usize>,
}

impl Candidate {
    fn new(
        text: String,
        score: f64,
        candidate_type: CandidateType,
        annotation: Option<String>,
        span_start: usize,
        span_end: usize,
    ) -> Self {
        Self {
            text,
            score,
            candidate_type,
            annotation,
            span_start,
            span_end,
            preview_start_utf16: None,
            preview_len_utf16: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionSnapshot {
    pub preedit: String,
    pub cursor_utf16: usize,
    pub selection_len_utf16: usize,
    #[serde(default)]
    pub target_start_utf16: usize,
    #[serde(default)]
    pub target_len_utf16: usize,
    pub is_valid_zhuyin: bool,
    pub has_uppercase: bool,
    pub has_resolved_chinese: bool,
    pub should_space_act_as_tone: bool,
    pub segment_count: usize,
    #[serde(default)]
    pub syllable_count: usize,
    #[serde(default)]
    pub english_ghost: String,
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
    is_punctuation: bool,
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
            is_punctuation: false,
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
            is_punctuation: false,
        }
    }

    fn punctuation(raw_start: usize, raw_end: usize) -> Self {
        Self {
            initial: None,
            medial: None,
            final_sym: None,
            tone: None,
            raw_start,
            raw_end,
            has_explicit_tone: true,
            is_valid: true,
            is_punctuation: true,
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
    zhuyin: String,
}

struct SentenceDecodeResult {
    segments: Vec<DecodedSegment>,
    syllables: Vec<SyllableBucket>,
    resolved_len: usize,
    cursor_syl_idx: Option<usize>,
    is_valid: bool,
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
    /// 暫存區編輯游標（UTF-8 byte 偏移量；None 表示位於緩衝區末尾）
    cursor_byte: Option<usize>,
    /// 是否正在句中游標處鍵入尚未閉合的注音音節
    is_editing_mid_syllable: bool,
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
            cursor_byte: None,
            is_editing_mid_syllable: false,
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
        self.cursor_byte = None;
        self.is_editing_mid_syllable = false;
    }

    /// 取得目前 raw 輸入緩衝區
    pub fn raw_buffer(&self) -> &str {
        &self.raw_buffer
    }

    /// 輸入一個按鍵字元（支援句中游標位置插入）
    pub fn push_char(&mut self, ch: char) {
        if let Some(b) = self.cursor_byte {
            let b = b.min(self.raw_buffer.len());
            let (old_p_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer[..b]);
            let old_p_len = old_p_syls.len();

            self.raw_buffer.insert(b, ch);
            let new_b = b + ch.len_utf8();
            let (new_p_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer[..new_b]);
            let new_p_len = new_p_syls.len();

            if new_p_len > old_p_len {
                let delta = new_p_len - old_p_len;
                for (s, e, _) in &mut self.user_overrides {
                    if *s >= old_p_len {
                        *s += delta;
                        *e += delta;
                    } else if *e > old_p_len {
                        *e += delta;
                    }
                }
            }

            let finished_with_tone = matches!(self.layout.map_key(ch), KeyMapping::Tone(_))
                && new_p_syls.last().map_or(false, |s| s.has_explicit_tone);
            self.is_editing_mid_syllable = !finished_with_tone;
            self.cursor_byte = if new_b >= self.raw_buffer.len() {
                self.is_editing_mid_syllable = false;
                None
            } else {
                Some(new_b)
            };
        } else {
            self.raw_buffer.push(ch);
            self.is_editing_mid_syllable = false;
        }
        let _ = self.custom_dict.reload_if_needed();
    }

    fn delete_syllable_range(
        &mut self,
        del_idx: usize,
        byte_start: usize,
        byte_end: usize,
        preserve_prev_first_tone: bool,
    ) {
        if byte_start >= byte_end || byte_end > self.raw_buffer.len() {
            return;
        }
        self.raw_buffer.drain(byte_start..byte_end);
        self.user_overrides.retain_mut(|(s, e, _)| {
            if *e <= del_idx {
                true
            } else if *s > del_idx {
                *s -= 1;
                *e -= 1;
                true
            } else {
                false
            }
        });
        let at_end_after_drain = self.raw_buffer.is_empty() || byte_start >= self.raw_buffer.len();
        if preserve_prev_first_tone && del_idx > 0 && at_end_after_drain {
            let (rem_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer);
            if let Some(prev_syl) = rem_syls.get(del_idx - 1) {
                if prev_syl.is_valid && prev_syl.has_phonemes() && !prev_syl.has_explicit_tone {
                    self.raw_buffer.push(' ');
                }
            }
        }
        self.is_editing_mid_syllable = false;
        self.cursor_byte = if at_end_after_drain {
            None
        } else {
            Some(byte_start)
        };
    }

    /// 刪除游標左側按鍵或已轉換的整個中文字（Backspace）
    pub fn pop_char(&mut self) -> Option<char> {
        if let Some(b) = self.cursor_byte {
            let b = b.min(self.raw_buffer.len());
            if b == 0 {
                return None;
            }
            let prefix = &self.raw_buffer[..b];
            let (p_syls, _) = self.parse_zhuyin_segments(prefix);
            let last_p = p_syls.last()?.clone();
            let del_idx = p_syls.len() - 1;

            // 若正在句中鍵入尚未組字成中文的按鍵（例如剛打完聲母 'r'），逐鍵退格
            let is_raw_mid_typing = self.is_editing_mid_syllable
                && !last_p.has_explicit_tone
                && !(p_syls.len() >= 2 && self.prefix_ends_with_phrase(&p_syls));

            if is_raw_mid_typing {
                let prev_b = self.raw_buffer[..b]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let popped = self.raw_buffer.remove(prev_b);
                if prev_b <= last_p.raw_start {
                    self.is_editing_mid_syllable = false;
                }
                self.cursor_byte = if self.raw_buffer.is_empty() || prev_b >= self.raw_buffer.len() {
                    None
                } else {
                    Some(prev_b)
                };
                return Some(popped);
            }

            // 否則游標左側已是完整中文字/音節，直接整字刪除！
            let first_ch = self.raw_buffer[last_p.raw_start..last_p.raw_end]
                .chars()
                .next();
            self.delete_syllable_range(del_idx, last_p.raw_start, last_p.raw_end, true);
            return first_ch;
        }

        // 位於緩衝區末尾：若最後已轉換為中文（非未完成按鍵、非純英文），按一次 Backspace 直接刪除整個中文字
        let decoded = self.build_sentence_segments();
        let n = decoded.syllables.len();
        let tail_is_converted_non_ascii = decoded.is_valid
            && n > 0
            && decoded.resolved_len == n
            && decoded
                .segments
                .last()
                .map_or(false, |seg| !seg.word.is_ascii());
        if tail_is_converted_non_ascii {
            let last_syl = &decoded.syllables[n - 1];
            let first_ch = self
                .raw_buffer
                .get(last_syl.raw_start..last_syl.raw_end)
                .and_then(|s| s.chars().next());
            self.delete_syllable_range(n - 1, last_syl.raw_start, last_syl.raw_end, true);
            return first_ch;
        }

        let popped = self.raw_buffer.pop();
        self.is_editing_mid_syllable = false;
        let (syllables, _) = self.parse_zhuyin_segments(&self.raw_buffer);
        let syl_count = syllables.len();
        self.user_overrides
            .retain(|(start, end, word)| *end <= syl_count && *start < *end && !(word.is_ascii() && *end == syl_count));
        popped
    }

    /// 向前刪除游標右側的中文字/音節（Forward Delete / Fn+Backspace）
    pub fn delete_forward(&mut self) -> bool {
        let Some(b) = self.cursor_byte else {
            return false;
        };
        let b = b.min(self.raw_buffer.len());
        if b >= self.raw_buffer.len() {
            self.cursor_byte = None;
            return false;
        }
        let (p_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer[..b]);
        let (s_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer[b..]);
        let Some(first_s) = s_syls.first() else {
            return false;
        };
        let del_idx = p_syls.len();
        self.delete_syllable_range(del_idx, b + first_s.raw_start, b + first_s.raw_end, true);
        true
    }

    /// 檢查是否有輸入暫存
    pub fn is_empty(&self) -> bool {
        self.raw_buffer.is_empty()
    }

    /// 取消編輯游標，返回末尾繼續輸入
    pub fn clear_cursor(&mut self) {
        self.cursor_byte = None;
        self.is_editing_mid_syllable = false;
    }

    /// 檢查目前是否處於句中游標位置（非緩衝區末尾）
    pub fn has_active_cursor(&self) -> bool {
        self.cursor_byte.is_some()
    }

    /// 向左移動一個音節/字元游標
    pub fn move_cursor_left(&mut self) -> bool {
        if self.raw_buffer.is_empty() {
            return false;
        }
        self.is_editing_mid_syllable = false;
        let bound = self.cursor_byte.unwrap_or(self.raw_buffer.len()).min(self.raw_buffer.len());
        if bound == 0 {
            return false;
        }
        let (p_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer[..bound]);
        if let Some(last_p) = p_syls.last() {
            self.cursor_byte = Some(last_p.raw_start);
            true
        } else {
            false
        }
    }

    /// 向右移動一個音節/字元游標
    pub fn move_cursor_right(&mut self) -> bool {
        let Some(b) = self.cursor_byte else {
            return false;
        };
        self.is_editing_mid_syllable = false;
        let b = b.min(self.raw_buffer.len());
        if b >= self.raw_buffer.len() {
            self.cursor_byte = None;
            return false;
        }
        let (s_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer[b..]);
        if let Some(first_s) = s_syls.first() {
            let next_b = b + first_s.raw_end;
            self.cursor_byte = if next_b >= self.raw_buffer.len() {
                None
            } else {
                Some(next_b)
            };
            true
        } else {
            self.cursor_byte = None;
            true
        }
    }

    /// 取得目前游標／目標區段的「上鍵」快速轉換與相似候選清單（注音文 / 英文 / 同音異調 / 近音 / 拆併詞）
    pub fn get_alternatives(&self, limit: usize) -> Vec<Candidate> {
        if self.raw_buffer.is_empty() {
            return Vec::new();
        }
        self.build_alternatives_internal(limit)
    }

    /// 在組字緩衝內選取第 `index` 個候選詞替換當前區段
    pub fn apply_candidate_selection(&mut self, index: usize) -> Option<(Candidate, usize)> {
        let snap = self.get_composition_snapshot(256);
        let chosen = snap.candidates.get(index)?.clone();
        Some(self.apply_chosen_override(chosen, snap.segment_count))
    }

    /// 套用「上鍵」轉換與相似選單中的第 `index` 項替代內容
    pub fn apply_alternative_selection(&mut self, index: usize) -> Option<(Candidate, usize)> {
        let snap = self.get_composition_snapshot(1);
        let alts = self.build_alternatives_internal(256);
        let chosen = alts.get(index)?.clone();
        Some(self.apply_chosen_override(chosen, snap.segment_count))
    }

    fn apply_chosen_override(
        &mut self,
        chosen: Candidate,
        segment_count: usize,
    ) -> (Candidate, usize) {
        if chosen.span_end <= chosen.span_start {
            return (chosen, segment_count);
        }

        // 若選擇了末尾未標調純注音（如剛按 'c' 就透過 ↑ 選「ㄏ」），補上空白鍵明確閉合該音節以免後續打字時失效
        if !chosen.text.is_ascii() {
            let (pre_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer);
            if chosen.span_end == pre_syls.len() {
                if let Some(last_syl) = pre_syls.last() {
                    if last_syl.is_valid && last_syl.has_phonemes() && !last_syl.has_explicit_tone {
                        self.raw_buffer.push(' ');
                    }
                }
            }
        }

        // 移除與此新區段重疊的舊手動鎖定
        self.user_overrides
            .retain(|(s, e, _)| *e <= chosen.span_start || *s >= chosen.span_end);
        self.user_overrides
            .push((chosen.span_start, chosen.span_end, chosen.text.clone()));
        self.is_editing_mid_syllable = false;

        // 若正處於句中游標模式，或選詞後仍有剩餘音節尚未涵蓋，推進游標至下一個區段起點
        let decoded = self.build_sentence_segments();
        if self.cursor_byte.is_some() || chosen.span_end < decoded.syllables.len() {
            if let Some(next_syl) = decoded.syllables.get(chosen.span_end) {
                self.cursor_byte = Some(next_syl.raw_start);
            } else {
                self.cursor_byte = None;
            }
        }

        (chosen, segment_count)
    }

    /// 產生行內預編輯文字（Pre-edit marked text）
    pub fn get_preedit_string(&self) -> String {
        self.get_composition_snapshot(1).preedit
    }

    /// 取得完整組字狀態快照（含斷詞結果、游標位置與候選詞）
    pub fn get_composition_snapshot(&self, limit: usize) -> CompositionSnapshot {
        if self.raw_buffer.is_empty() {
            return CompositionSnapshot {
                preedit: String::new(),
                cursor_utf16: 0,
                selection_len_utf16: 0,
                target_start_utf16: 0,
                target_len_utf16: 0,
                is_valid_zhuyin: false,
                has_uppercase: false,
                has_resolved_chinese: false,
                should_space_act_as_tone: false,
                segment_count: 0,
                syllable_count: 0,
                english_ghost: String::new(),
                candidates: Vec::new(),
            };
        }

        let has_uppercase = self.raw_buffer.chars().any(|c| c.is_uppercase());
        let decoded = self.build_sentence_segments();
        let trailing_raw = if decoded.resolved_len < decoded.syllables.len() {
            let start_byte = decoded.syllables[decoded.resolved_len].raw_start;
            &self.raw_buffer[start_byte..]
        } else if !decoded.is_valid {
            self.raw_buffer.as_str()
        } else {
            ""
        };

        // 決定候選詞目標音節區間 (target_start, target_end)
        let (target_start, target_end) = self.compute_target_span(&decoded);

        // 組合行內顯示字串，並同時計算游標 UTF-16 位置與候選目標的 UTF-16 範圍
        let mut preedit = String::new();
        let mut cursor_utf16 = 0usize;
        let mut found_cursor = false;
        let mut target_start_utf16 = 0usize;
        let mut target_end_utf16 = 0usize;
        let mut found_target_start = false;
        let mut found_target_end = false;

        if decoded.is_valid && !decoded.segments.is_empty() {
            for seg in &decoded.segments {
                let seg_start_utf16 = preedit.encode_utf16().count();

                if !found_cursor {
                    if let Some(c_syl) = decoded.cursor_syl_idx {
                        if c_syl == seg.start_syl {
                            cursor_utf16 = seg_start_utf16;
                            found_cursor = true;
                        } else if c_syl > seg.start_syl && c_syl < seg.end_syl {
                            let char_offset = c_syl - seg.start_syl;
                            let partial_utf16: usize = seg
                                .word
                                .chars()
                                .take(char_offset)
                                .map(|c| c.len_utf16())
                                .sum();
                            cursor_utf16 = seg_start_utf16 + partial_utf16;
                            found_cursor = true;
                        }
                    }
                }

                if !found_target_start && target_start >= seg.start_syl && target_start < seg.end_syl {
                    let char_offset = target_start - seg.start_syl;
                    let partial_utf16: usize = seg
                        .word
                        .chars()
                        .take(char_offset)
                        .map(|c| c.len_utf16())
                        .sum();
                    target_start_utf16 = seg_start_utf16 + partial_utf16;
                    found_target_start = true;
                }

                preedit.push_str(&seg.word);
                let seg_end_utf16 = preedit.encode_utf16().count();

                if !found_target_end && target_end > seg.start_syl && target_end <= seg.end_syl {
                    if target_end == seg.end_syl {
                        target_end_utf16 = seg_end_utf16;
                    } else {
                        let char_offset = target_end - seg.start_syl;
                        let partial_utf16: usize = seg
                            .word
                            .chars()
                            .take(char_offset)
                            .map(|c| c.len_utf16())
                            .sum();
                        target_end_utf16 = seg_start_utf16 + partial_utf16;
                    }
                    found_target_end = true;
                }
            }
            if !trailing_raw.is_empty() {
                let raw_start_utf16 = preedit.encode_utf16().count();
                preedit.push_str(trailing_raw);
                if !found_target_start {
                    target_start_utf16 = raw_start_utf16;
                    target_end_utf16 = preedit.encode_utf16().count();
                }
            }
            if !found_cursor {
                cursor_utf16 = preedit.encode_utf16().count();
            }
        } else {
            preedit = self.raw_buffer.clone();
            cursor_utf16 = if let Some(b) = self.cursor_byte {
                self.raw_buffer[..b.min(self.raw_buffer.len())]
                    .encode_utf16()
                    .count()
            } else {
                preedit.encode_utf16().count()
            };
        }

        let target_len_utf16 = target_end_utf16.saturating_sub(target_start_utf16);

        let candidates = self.build_candidates_internal(
            limit,
            &decoded,
            target_start,
            target_end,
            has_uppercase,
        );

        let top_is_english = candidates
            .first()
            .map_or(false, |c| c.candidate_type == CandidateType::English);

        // 判斷空白鍵是否應作為「一聲（陰平）」：
        // - 位於緩衝區末尾：尾端音節為有效未標調注音且未被手動覆寫
        // - 位於句中編輯新音節時：游標左側剛輸入的音節為有效未標調注音
        let active_syl_for_space = if let Some(c_syl) = decoded.cursor_syl_idx {
            if self.is_editing_mid_syllable && c_syl > 0 {
                let has_ov = self.user_overrides.iter().any(|(_, end, _)| *end == c_syl);
                if has_ov {
                    None
                } else {
                    decoded.syllables.get(c_syl - 1)
                }
            } else {
                None
            }
        } else {
            let has_ov = self
                .user_overrides
                .iter()
                .any(|(_, end, _)| *end == decoded.syllables.len());
            if has_ov {
                None
            } else {
                decoded.syllables.last()
            }
        };

        let should_space_act_as_tone = decoded.is_valid
            && !has_uppercase
            && !top_is_english
            && active_syl_for_space
                .map_or(false, |s| s.is_valid && !s.has_explicit_tone && s.has_phonemes());

        let english_ghost = self
            .compute_english_ghost(&decoded, top_is_english)
            .map(|(_, raw_tok, full_word)| full_word[raw_tok.len()..].to_string())
            .unwrap_or_default();

        CompositionSnapshot {
            preedit,
            cursor_utf16,
            selection_len_utf16: 0,
            target_start_utf16,
            target_len_utf16,
            is_valid_zhuyin: decoded.is_valid,
            has_uppercase,
            has_resolved_chinese: !decoded.segments.is_empty(),
            should_space_act_as_tone,
            segment_count: decoded.segments.len(),
            syllable_count: decoded.syllables.len(),
            english_ghost,
            candidates,
        }
    }

    /// 接受目前的英文行內半透明補字（Tab 鍵補全英文單字）
    pub fn accept_english_ghost(&mut self) -> bool {
        let decoded = self.build_sentence_segments();
        let raw_lower = self.raw_buffer.to_ascii_lowercase();
        let top_is_english = !decoded.is_valid
            || (decoded.segments.is_empty()
                && !self.english_lexicon.find_prefix(&raw_lower, 1).is_empty());

        let Some((token_start, _raw_tok, full_match)) =
            self.compute_english_ghost(&decoded, top_is_english)
        else {
            return false;
        };

        let prefix_syls = if token_start > 0 {
            self.parse_zhuyin_segments(&self.raw_buffer[..token_start]).0.len()
        } else {
            0
        };

        self.raw_buffer.replace_range(token_start.., &full_match);
        self.cursor_byte = None;
        self.is_editing_mid_syllable = false;

        if token_start == 0 {
            self.user_overrides.clear();
        } else {
            let (new_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer);
            let new_end = new_syls.len();
            self.user_overrides.retain(|(_, e, _)| *e <= prefix_syls);
            if new_end > prefix_syls {
                self.user_overrides
                    .push((prefix_syls, new_end, full_match));
            }
        }
        true
    }

    /// 計算游標末尾正在輸入之英文單字的行內半透明補字建議 `(token_byte_start, typed_token, full_match)`
    fn compute_english_ghost(
        &self,
        decoded: &SentenceDecodeResult,
        top_is_english: bool,
    ) -> Option<(usize, String, String)> {
        if self.cursor_byte.is_some() || self.raw_buffer.is_empty() {
            return None;
        }

        let bytes = self.raw_buffer.as_bytes();
        let mut token_start = bytes.len();
        while token_start > 0 && bytes[token_start - 1].is_ascii_alphabetic() {
            token_start -= 1;
        }
        let token = &self.raw_buffer[token_start..];
        if token.len() < 2 {
            return None;
        }

        if token_start == 0 {
            // 純英文或英文優先：避免對單音節未標調中文（如 "ji" -> ji3 我）誤彈補字
            let allow_pure = !decoded.is_valid
                || (top_is_english && decoded.segments.is_empty() && decoded.syllables.len() >= 2);
            if !allow_pure {
                return None;
            }
        } else {
            // 混打句尾英文（例如 "這是app"）：確保 trailing 英文字母不在已組詞的中文區段內，且包含無法組成注音的按鍵
            let tail_has_invalid = decoded.syllables.iter().any(|syl| {
                syl.raw_end > token_start && !syl.is_valid
            });
            let unresolved_tail = decoded
                .syllables
                .get(decoded.resolved_len)
                .map_or(false, |syl| syl.raw_start >= token_start);
            let token_is_inside_override = self.user_overrides.iter().any(|(s, e, _)| {
                let end_b = decoded.syllables.get(e.saturating_sub(1)).map_or(0, |syl| syl.raw_end);
                let start_b = decoded.syllables.get(*s).map_or(0, |syl| syl.raw_start);
                token_start >= start_b && end_b >= self.raw_buffer.len()
            });
            if token_is_inside_override || (!tail_has_invalid && !unresolved_tail) {
                return None;
            }
        }

        let token_lower = token.to_ascii_lowercase();
        let prev_word = self.last_committed_word.as_deref();
        let exact_score = self
            .custom_dict
            .query(&token_lower)
            .into_iter()
            .next()
            .map(|(_, s)| s + 10_000.0)
            .or_else(|| {
                self.english_lexicon
                    .find_exact(&token_lower)
                    .into_iter()
                    .next()
                    .map(|(w, base)| base + self.user_db.get_adaptive_boost(&w, prev_word))
            })
            .unwrap_or(0.0);

        let mut best: Option<(String, f64)> = None;
        for (word, base) in self.english_lexicon.find_prefix(&token_lower, 16) {
            if word.len() <= token.len() || !word.to_ascii_lowercase().starts_with(&token_lower) {
                continue;
            }
            let total = base + self.user_db.get_adaptive_boost(&word, prev_word);
            // 若使用者已打完高分精確單字（例如 "apple" 920 vs "apples" 905），不額外干擾
            if exact_score > 0.0 && total <= exact_score + 10.0 {
                continue;
            }
            if best.as_ref().map_or(true, |(_, bs)| total > *bs) {
                best = Some((word, total));
            }
        }

        let (matched, _) = best?;
        let display_match = if token.chars().next().map_or(false, |c| c.is_uppercase()) {
            Self::to_title_case(&matched)
        } else {
            matched
        };

        if display_match.len() <= token.len() {
            return None;
        }
        Some((token_start, token.to_string(), display_match))
    }

    fn compute_target_span(&self, decoded: &SentenceDecodeResult) -> (usize, usize) {
        if !decoded.is_valid || decoded.syllables.is_empty() {
            return (0, 0);
        }
        if let Some(curr_syl) = decoded.cursor_syl_idx {
            if self.is_editing_mid_syllable && curr_syl > 0 {
                return (curr_syl - 1, curr_syl);
            }
            if let Some(seg) = decoded
                .segments
                .iter()
                .find(|s| curr_syl >= s.start_syl && curr_syl < s.end_syl)
            {
                return (curr_syl.max(seg.start_syl), seg.end_syl);
            }
            if curr_syl > 0 {
                if let Some(last_seg) = decoded.segments.last() {
                    return (last_seg.start_syl, last_seg.end_syl);
                }
            }
            return (0, decoded.syllables.len());
        }
        if !decoded.segments.is_empty() && decoded.resolved_len == decoded.syllables.len() {
            let last_seg = decoded.segments.last().unwrap();
            (last_seg.start_syl, last_seg.end_syl)
        } else {
            (decoded.resolved_len, decoded.syllables.len())
        }
    }

    /// 雙軌解碼：回傳候選詞清單
    pub fn get_candidates(&self, limit: usize) -> Vec<Candidate> {
        self.get_composition_snapshot(limit).candidates
    }

    fn build_candidates_internal(
        &self,
        limit: usize,
        decoded: &SentenceDecodeResult,
        target_start: usize,
        target_end: usize,
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
        if decoded.is_valid && !decoded.syllables.is_empty() {
            let syllables = &decoded.syllables;
            let max_end = syllables.len();

            // 1A. 若為單一詞彙或鎖定目標區段（且非標點符號），優先列出目標區段精確匹配詞條
            let target_all_valid = target_end > target_start
                && syllables[target_start..target_end]
                    .iter()
                    .all(|s| s.is_valid && !s.is_punctuation);
            if target_end == target_start + 1 && syllables[target_start].is_punctuation {
                let raw_p = self
                    .raw_buffer
                    .get(syllables[target_start].raw_start..syllables[target_start].raw_end)
                    .unwrap_or("");
                for (alt_p, tag) in Self::punctuation_variants(raw_p) {
                    candidates.push(Candidate::new(
                        alt_p,
                        5000.0 - candidates.len() as f64,
                        CandidateType::Chinese,
                        Some(tag.to_string()),
                        target_start,
                        target_end,
                    ));
                }
            }
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
                    candidates.push(Candidate::new(
                        override_word.clone(),
                        200_000.0,
                        CandidateType::Chinese,
                        Some(target_zy.clone()),
                        target_start,
                        target_end,
                    ));
                }

                for (word, score) in self.custom_dict.query(&target_zy) {
                    if !candidates.iter().any(|c| c.text == word) {
                        candidates.push(Candidate::new(
                            word,
                            score + 10000.0,
                            CandidateType::Custom,
                            Some("自訂".to_string()),
                            target_start,
                            target_end,
                        ));
                    }
                }

                for (word, base_score) in self.chinese_lexicon.find_exact(&target_zy) {
                    let (uni, bi) = self
                        .user_db
                        .get_adaptive_boost_for_reading(&target_zy, &word, prev_word);
                    if !candidates.iter().any(|c| c.text == word) {
                        candidates.push(Candidate::new(
                            word,
                            base_score + uni + bi + 3000.0,
                            CandidateType::Chinese,
                            Some(target_zy.clone()),
                            target_start,
                            target_end,
                        ));
                    }
                }

                // 1A-2. 台灣「注音文 / 單純注音符號」直出候選：
                // 單音節輸入時提供純注音候選（標註為「注」），單一注音符號（如 ㄅ、ㄏ、ㄓ、ㄧ、ㄚ）確保出現在第 1 頁以利快速選取，同時不覆蓋首選常用漢字
                if target_end - target_start == 1 && !target_zy.is_empty() {
                    let is_single_bopomofo_symbol = target_zy.chars().count() == 1
                        && target_zy
                            .chars()
                            .next()
                            .map(|c| ('\u{3105}'..='\u{3129}').contains(&c))
                            .unwrap_or(false);
                    let (zy_uni, zy_bi) = self
                        .user_db
                        .get_adaptive_boost_for_reading(&target_zy, &target_zy, prev_word);
                    let zy_adaptive = zy_uni + zy_bi;

                    let mut exact_non_zy_scores: Vec<f64> = candidates
                        .iter()
                        .filter(|c| {
                            c.candidate_type == CandidateType::Chinese
                                && c.span_start == target_start
                                && c.span_end == target_end
                                && c.text != target_zy
                        })
                        .map(|c| c.score)
                        .collect();
                    exact_non_zy_scores
                        .sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

                    let zy_score = if exact_non_zy_scores.is_empty() {
                        4260.0 + zy_adaptive
                    } else if is_single_bopomofo_symbol {
                        let anchor_idx = 1.min(exact_non_zy_scores.len() - 1);
                        let anchor = exact_non_zy_scores[anchor_idx];
                        (anchor - 5.0).max(3800.0) + zy_adaptive
                    } else {
                        // 完整音節注音（如 ㄓㄨˋ、ㄏㄠˇ）穩定呈現於第 1 頁第 4 順位（即使每頁 5 字也可直接看到）
                        let anchor_idx = 3.min(exact_non_zy_scores.len() - 1);
                        let anchor = exact_non_zy_scores[anchor_idx];
                        (anchor - 5.0).max(3500.0) + zy_adaptive
                    };

                    if let Some(existing) = candidates.iter_mut().find(|c| c.text == target_zy) {
                        if zy_score > existing.score {
                            existing.score = zy_score;
                        }
                        existing.annotation = Some("注".to_string());
                    } else {
                        candidates.push(Candidate::new(
                            target_zy.clone(),
                            zy_score,
                            CandidateType::Chinese,
                            Some("注".to_string()),
                            target_start,
                            target_end,
                        ));
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
                        let (uni, bi) = self
                            .user_db
                            .get_adaptive_boost_for_reading(&alt_zy, &word, prev_word);
                        if !candidates.iter().any(|c| c.text == word) {
                            candidates.push(Candidate::new(
                                word,
                                base_score + uni + bi + len_bonus + 1500.0,
                                CandidateType::Chinese,
                                Some(alt_zy.clone()),
                                target_start,
                                end_idx,
                            ));
                        }
                    }
                }

                // 1C. 若處於一般輸入（非游標重選），補上以此音節開頭的前綴自動完成詞（例如輸入 "ㄅㄨˋㄓ" 即可提前看到 "不知道" 3 字詞）
                if self.cursor_byte.is_none() {
                    for (word, base_score) in self.chinese_lexicon.find_prefix(&target_zy, limit) {
                        let (uni, bi) = self
                            .user_db
                            .get_adaptive_boost_for_reading(&target_zy, &word, prev_word);
                        if !candidates.iter().any(|c| c.text == word) {
                            candidates.push(Candidate::new(
                                word,
                                base_score + uni + bi - 1000.0,
                                CandidateType::Chinese,
                                Some(target_zy.clone()),
                                target_start,
                                target_end,
                            ));
                        }
                    }
                }
            }
        }

        // 2. 自訂辭典英文鍵值匹配
        for (word, score) in self.custom_dict.query(&raw_lower) {
            if !candidates.iter().any(|c| c.text == word) {
                candidates.push(Candidate::new(
                    word,
                    score + 10000.0,
                    CandidateType::Custom,
                    Some("自訂".to_string()),
                    0,
                    0,
                ));
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

            if !decoded.is_valid {
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
                candidates.push(Candidate::new(
                    display_text,
                    score,
                    CandidateType::English,
                    Some("英".to_string()),
                    0,
                    0,
                ));
            }
        }

        // 4. 保底原始英文字串
        if !candidates.iter().any(|c| c.text == self.raw_buffer) {
            let fallback_score = if has_uppercase {
                5000.0
            } else if !decoded.is_valid {
                800.0
            } else {
                10.0
            };
            candidates.push(Candidate::new(
                self.raw_buffer.clone(),
                fallback_score,
                CandidateType::English,
                Some("英".to_string()),
                0,
                0,
            ));
        }

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(limit);
        candidates
    }

    /// 華碩式「方向鍵上 (↑)」選單：提供注音文、英文原文/相似單字、同音異調、近音詞與拆併詞切換
    fn build_alternatives_internal(&self, limit: usize) -> Vec<Candidate> {
        let decoded = self.build_sentence_segments();
        let syllables = &decoded.syllables;
        let n = syllables.len();
        let mut results: Vec<Candidate> = Vec::new();
        let prev_word = self.last_committed_word.as_deref();

        // 若無有效中文音節（例如純英文單字 "hello" / "test"），提供原文大小寫、英文補全與全形符號等選項
        if !decoded.is_valid || n == 0 {
            let raw = self.raw_buffer.trim_end();
            if raw.is_empty() {
                return results;
            }
            let raw_utf16 = self.raw_buffer.encode_utf16().count();
            let raw_lower = raw.to_ascii_lowercase();
            let mut push_raw = |text: String, tag: &str| {
                if text.is_empty() || text == self.raw_buffer || results.iter().any(|c| c.text == text) {
                    return;
                }
                let mut cand = Candidate::new(
                    text,
                    1000.0 - results.len() as f64,
                    CandidateType::English,
                    Some(tag.to_string()),
                    0,
                    n,
                );
                cand.preview_start_utf16 = Some(0);
                cand.preview_len_utf16 = Some(raw_utf16);
                results.push(cand);
            };

            if raw != self.raw_buffer {
                push_raw(raw.to_string(), "英");
            }
            for (word, _) in self.custom_dict.query(&raw_lower).into_iter().take(3) {
                push_raw(word, "自訂");
            }
            for (word, _) in self.english_lexicon.find_prefix(&raw_lower, 6) {
                push_raw(word, "英");
            }
            if raw.chars().all(|c| c.is_ascii_alphabetic()) {
                let title = Self::to_title_case(raw);
                let upper = raw.to_ascii_uppercase();
                push_raw(raw_lower.clone(), "英");
                push_raw(title, "英");
                push_raw(upper, "英");
            }
            push_raw(self.raw_buffer.clone(), "英");
            results.truncate(limit);
            return results;
        }

        let (target_start, target_end) = self.compute_target_span(&decoded);
        let s = target_start.min(n.saturating_sub(1));
        let e = target_end.max(s + 1).min(n);
        let syl_offsets = self.compute_syllable_utf16_offsets(&decoded);
        if e == s + 1 && syllables[s].is_punctuation {
            let raw_p = self
                .raw_buffer
                .get(syllables[s].raw_start..syllables[s].raw_end)
                .unwrap_or("");
            let p_start = syl_offsets.get(s).copied().unwrap_or(0);
            let p_end = syl_offsets.get(e).copied().unwrap_or(p_start);
            for (alt_p, tag) in Self::punctuation_variants(raw_p) {
                let mut cand = Candidate::new(
                    alt_p,
                    10_000.0 - results.len() as f64,
                    CandidateType::Chinese,
                    Some(tag.to_string()),
                    s,
                    e,
                );
                cand.preview_start_utf16 = Some(p_start);
                cand.preview_len_utf16 = Some(p_end.saturating_sub(p_start));
                results.push(cand);
            }
            results.truncate(limit);
            return results;
        }
        let span_all_valid = syllables[s..e].iter().all(|syl| syl.is_valid && !syl.is_punctuation);
        let target_zy: String = if span_all_valid {
            syllables[s..e]
                .iter()
                .map(|syl| syl.to_canonical_string())
                .collect()
        } else {
            String::new()
        };

        // 找出目前組字在 (s..e) 上顯示的文字，避免把「目前已經顯示的字」當作第一項轉換
        let current_span_text: Option<String> = decoded
            .segments
            .iter()
            .find(|seg| seg.start_syl == s && seg.end_syl == e)
            .map(|seg| seg.word.clone())
            .or_else(|| {
                if e <= decoded.resolved_len {
                    None
                } else {
                    let b0 = syllables[s].raw_start;
                    let b1 = syllables[e - 1].raw_end;
                    self.raw_buffer.get(b0..b1).map(|str_s| str_s.to_string())
                }
            });

        let mut push_alt = |text: String,
                            ctype: CandidateType,
                            tag: &str,
                            span_s: usize,
                            span_e: usize| {
            if text.is_empty() || span_e <= span_s || span_e > n {
                return;
            }
            if span_s == s && span_e == e && current_span_text.as_deref() == Some(text.as_str()) {
                return;
            }
            if results
                .iter()
                .any(|c| c.text == text && c.span_start == span_s && c.span_end == span_e)
            {
                return;
            }
            let p_start = syl_offsets.get(span_s).copied().unwrap_or(0);
            let p_end = syl_offsets.get(span_e).copied().unwrap_or(p_start);
            let mut cand = Candidate::new(
                text,
                10_000.0 - results.len() as f64,
                ctype,
                Some(tag.to_string()),
                span_s,
                span_e,
            );
            cand.preview_start_utf16 = Some(p_start);
            cand.preview_len_utf16 = Some(p_end.saturating_sub(p_start));
            results.push(cand);
        };

        // 1. 注音文（單音節或多音節整段注音，及多字詞尾字/首字快速注音轉換）
        if !target_zy.is_empty() {
            push_alt(target_zy.clone(), CandidateType::Chinese, "注", s, e);
            if e - s > 1 {
                if self.cursor_byte.is_none() {
                    let last_zy = syllables[e - 1].to_canonical_string();
                    push_alt(last_zy, CandidateType::Chinese, "注", e - 1, e);
                }
                let first_zy = syllables[s].to_canonical_string();
                push_alt(first_zy, CandidateType::Chinese, "注", s, s + 1);
            }
        }

        // 2. 英文／原始按鍵與相鄰完整英文單字
        if let Some((ascii_s, ascii_e, ascii_token)) = self.extract_contiguous_ascii_span(&decoded, s) {
            let ascii_lower = ascii_token.to_ascii_lowercase();
            let is_known_eng = !self.english_lexicon.find_exact(&ascii_lower).is_empty();
            if is_known_eng {
                push_alt(ascii_token.clone(), CandidateType::English, "英", ascii_s, ascii_e);
            }
            for (word, _) in self.english_lexicon.find_prefix(&ascii_lower, 4) {
                push_alt(word, CandidateType::English, "英", ascii_s, ascii_e);
            }
            if !is_known_eng {
                push_alt(ascii_token.clone(), CandidateType::English, "英", ascii_s, ascii_e);
            }
            if ascii_token.chars().all(|c| c.is_ascii_alphabetic()) {
                push_alt(
                    Self::to_title_case(&ascii_token),
                    CandidateType::English,
                    "英",
                    ascii_s,
                    ascii_e,
                );
                push_alt(
                    ascii_token.to_ascii_uppercase(),
                    CandidateType::English,
                    "英",
                    ascii_s,
                    ascii_e,
                );
            }
        }

        let seg_raw = self
            .raw_buffer
            .get(syllables[s].raw_start..syllables[e - 1].raw_end)
            .unwrap_or("")
            .trim_end();
        if !seg_raw.is_empty() {
            push_alt(seg_raw.to_string(), CandidateType::English, "英", s, e);
        }

        // 若區段小於整句，額外提供「整串轉原始英文按鍵」，方便忘記切英文時一鍵還原
        if e - s < n {
            let full_raw = self.raw_buffer.trim_end();
            if !full_raw.is_empty() {
                push_alt(full_raw.to_string(), CandidateType::English, "全英", 0, n);
            }
        }

        // 3. 若目前已被覆寫成注音或英文，附上該區段最常用的中文／自訂詞以利切回中文
        if !target_zy.is_empty() {
            for (word, _) in self.custom_dict.query(&target_zy).into_iter().take(2) {
                push_alt(word, CandidateType::Custom, "自訂", s, e);
            }
            if let Some((word, _)) = self.chinese_lexicon.find_exact(&target_zy).into_iter().next() {
                push_alt(word, CandidateType::Chinese, "中", s, e);
            }
        }

        // 4. 相似東西：同音異調 ([調])
        if span_all_valid {
            for word in self.collect_tone_variant_words(&syllables[s..e], prev_word, 6) {
                push_alt(word, CandidateType::Chinese, "調", s, e);
            }
        }

        // 5. 相似東西：常見易混近音 ([近音]：ㄓ/ㄗ、ㄔ/ㄘ、ㄕ/ㄙ、ㄣ/ㄥ、ㄢ/ㄤ、ㄈ/ㄏ、ㄖ/ㄌ)
        if e - s == 1 && syllables[s].is_valid {
            for word in self.collect_confused_sound_words(&syllables[s], prev_word, 5) {
                push_alt(word, CandidateType::Chinese, "近音", s, e);
            }
        }

        // 6. 分詞微調 ([字] 拆詞 / [詞] 併詞)
        if e - s >= 2 && syllables[s].is_valid {
            let first_zy = syllables[s].to_canonical_string();
            if let Some((word, _)) = self.chinese_lexicon.find_exact(&first_zy).into_iter().next() {
                push_alt(word, CandidateType::Chinese, "字", s, s + 1);
            }
        } else if e - s == 1 {
            for end_idx in [s + 2, s + 3] {
                if end_idx <= n && syllables[s..end_idx].iter().all(|syl| syl.is_valid) {
                    let merge_zy: String = syllables[s..end_idx]
                        .iter()
                        .map(|syl| syl.to_canonical_string())
                        .collect();
                    if let Some((word, _)) =
                        self.chinese_lexicon.find_exact(&merge_zy).into_iter().next()
                    {
                        push_alt(word, CandidateType::Chinese, "詞", s, end_idx);
                    }
                }
            }
        }

        results.truncate(limit);
        results
    }

    fn punctuation_variants(p: &str) -> Vec<(String, &'static str)> {
        let items: &[(&str, &str)] = match p {
            "，" => &[(",", "半形"), ("、", "符"), ("；", "符"), ("。", "符")],
            "。" => &[(".", "半形"), ("，", "符"), ("．", "全形")],
            "、" => &[(",", "半形"), ("，", "符"), ("/", "半形"), ("\\", "半形")],
            "？" => &[("?", "半形"), ("！", "符")],
            "！" => &[("!", "半形"), ("？", "符")],
            "：" => &[(":", "半形"), ("；", "符")],
            "；" => &[(";", "半形"), ("：", "符"), ("，", "符")],
            "「" => &[("[", "半形"), ("『", "符"), ("（", "符")],
            "」" => &[("]", "半形"), ("』", "符"), ("）", "符")],
            "『" => &[("{", "半形"), ("「", "符")],
            "』" => &[("}", "半形"), ("」", "符")],
            "（" => &[("(", "半形"), ("「", "符")],
            "）" => &[(")", "半形"), ("」", "符")],
            "’" => &[("'", "半形"), ("”", "符")],
            "”" => &[("\"", "半形"), ("’", "符")],
            "～" => &[("~", "半形")],
            _ => &[],
        };
        items.iter().map(|(s, tag)| ((*s).to_string(), *tag)).collect()
    }

    fn to_title_case(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_ascii_lowercase(),
        }
    }

    fn compute_syllable_utf16_offsets(&self, decoded: &SentenceDecodeResult) -> Vec<usize> {
        let n = decoded.syllables.len();
        let mut offsets = vec![0usize; n + 1];
        let mut cursor_utf16 = 0usize;

        for seg in &decoded.segments {
            let span_len = seg.end_syl.saturating_sub(seg.start_syl).max(1);
            let chars: Vec<char> = seg.word.chars().collect();
            let mut local_utf16 = 0usize;
            for k in 0..span_len {
                let syl_idx = seg.start_syl + k;
                if syl_idx < n {
                    offsets[syl_idx] = cursor_utf16 + local_utf16;
                }
                if k < chars.len() {
                    local_utf16 += chars[k].len_utf16();
                }
            }
            cursor_utf16 += seg.word.encode_utf16().count();
            if seg.end_syl <= n {
                offsets[seg.end_syl] = cursor_utf16;
            }
        }

        for k in decoded.resolved_len..n {
            offsets[k] = cursor_utf16;
            let s_b = decoded.syllables[k].raw_start;
            let e_b = decoded.syllables[k].raw_end.min(self.raw_buffer.len());
            if let Some(raw_slice) = self.raw_buffer.get(s_b..e_b) {
                cursor_utf16 += raw_slice.encode_utf16().count();
            }
            offsets[k + 1] = cursor_utf16;
        }
        offsets
    }

    fn extract_contiguous_ascii_span(
        &self,
        decoded: &SentenceDecodeResult,
        anchor_syl: usize,
    ) -> Option<(usize, usize, String)> {
        let syl = decoded.syllables.get(anchor_syl)?;
        let bytes = self.raw_buffer.as_bytes();
        let mut left = syl.raw_start.min(bytes.len());
        let mut right = syl.raw_end.min(bytes.len());

        // 若目標音節自身並非英文字母（例如純數字），不強行展開
        if !bytes[left..right].iter().any(|b| b.is_ascii_alphabetic()) {
            return None;
        }
        while left > 0 && bytes[left - 1].is_ascii_alphabetic() {
            left -= 1;
        }
        while right < bytes.len() && bytes[right].is_ascii_alphabetic() {
            right += 1;
        }

        let start_syl = decoded
            .syllables
            .iter()
            .position(|s| s.raw_end > left)
            .unwrap_or(anchor_syl);
        let end_syl = decoded
            .syllables
            .iter()
            .rposition(|s| s.raw_start < right)
            .map(|i| i + 1)
            .unwrap_or(anchor_syl + 1);

        let token = self.raw_buffer.get(left..right)?.to_string();
        if token.is_empty() || end_syl <= start_syl {
            None
        } else {
            Some((start_syl, end_syl, token))
        }
    }

    fn collect_tone_variant_words(
        &self,
        span: &[SyllableBucket],
        prev_word: Option<&str>,
        max_out: usize,
    ) -> Vec<String> {
        const TONES: [Tone; 5] = [
            Tone::First,
            Tone::Second,
            Tone::Third,
            Tone::Fourth,
            Tone::Fifth,
        ];
        let mut scored: Vec<(String, f64)> = Vec::new();

        if span.len() == 1 {
            let orig_tone = span[0].tone.unwrap_or(Tone::First);
            for tone in TONES {
                if tone == orig_tone {
                    continue;
                }
                let mut alt = span[0].clone();
                alt.tone = Some(tone);
                let zy = alt.to_canonical_string();
                for (w, base) in self.chinese_lexicon.find_exact(&zy).into_iter().take(2) {
                    let (uni, bi) = self.user_db.get_adaptive_boost_for_reading(&zy, &w, prev_word);
                    scored.push((w, base + uni + bi));
                }
            }
        } else if span.len() <= 3 {
            for pos in 0..span.len() {
                let orig_tone = span[pos].tone.unwrap_or(Tone::First);
                for tone in [Tone::First, Tone::Second, Tone::Third, Tone::Fourth] {
                    if tone == orig_tone {
                        continue;
                    }
                    let zy: String = span
                        .iter()
                        .enumerate()
                        .map(|(idx, syl)| {
                            if idx == pos {
                                let mut c = syl.clone();
                                c.tone = Some(tone);
                                c.to_canonical_string()
                            } else {
                                syl.to_canonical_string()
                            }
                        })
                        .collect();
                    for (w, base) in self.chinese_lexicon.find_exact(&zy).into_iter().take(2) {
                        let (uni, bi) =
                            self.user_db.get_adaptive_boost_for_reading(&zy, &w, prev_word);
                        scored.push((w, base + uni + bi));
                    }
                }
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut out = Vec::new();
        for (w, _) in scored {
            if !out.contains(&w) {
                out.push(w);
                if out.len() >= max_out {
                    break;
                }
            }
        }
        out
    }

    fn collect_confused_sound_words(
        &self,
        syl: &SyllableBucket,
        prev_word: Option<&str>,
        max_out: usize,
    ) -> Vec<String> {
        let mut neighbor_syllables: Vec<SyllableBucket> = Vec::new();

        if let Some(init) = syl.initial {
            let alt_init = match init {
                ZhuyinSymbol::Zh => Some(ZhuyinSymbol::Z),
                ZhuyinSymbol::Z => Some(ZhuyinSymbol::Zh),
                ZhuyinSymbol::Ch => Some(ZhuyinSymbol::C),
                ZhuyinSymbol::C => Some(ZhuyinSymbol::Ch),
                ZhuyinSymbol::Sh => Some(ZhuyinSymbol::S),
                ZhuyinSymbol::S => Some(ZhuyinSymbol::Sh),
                ZhuyinSymbol::F => Some(ZhuyinSymbol::H),
                ZhuyinSymbol::H => Some(ZhuyinSymbol::F),
                ZhuyinSymbol::N => Some(ZhuyinSymbol::L),
                ZhuyinSymbol::L => Some(ZhuyinSymbol::N),
                ZhuyinSymbol::R => Some(ZhuyinSymbol::L),
                _ => None,
            };
            if let Some(ni) = alt_init {
                let mut c = syl.clone();
                c.initial = Some(ni);
                neighbor_syllables.push(c);
            }
        }

        if let Some(fin) = syl.final_sym {
            let alt_fin = match fin {
                ZhuyinSymbol::En => Some(ZhuyinSymbol::Eng),
                ZhuyinSymbol::Eng => Some(ZhuyinSymbol::En),
                ZhuyinSymbol::An => Some(ZhuyinSymbol::Ang),
                ZhuyinSymbol::Ang => Some(ZhuyinSymbol::An),
                _ => None,
            };
            if let Some(nf) = alt_fin {
                let mut c = syl.clone();
                c.final_sym = Some(nf);
                neighbor_syllables.push(c);
            }
        }

        let mut scored: Vec<(String, f64)> = Vec::new();
        for nb in neighbor_syllables {
            let zy = nb.to_canonical_string();
            for (w, base) in self.chinese_lexicon.find_exact(&zy).into_iter().take(2) {
                let (uni, bi) = self.user_db.get_adaptive_boost_for_reading(&zy, &w, prev_word);
                scored.push((w, base + uni + bi));
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut out = Vec::new();
        for (w, _) in scored {
            if !out.contains(&w) {
                out.push(w);
                if out.len() >= max_out {
                    break;
                }
            }
        }
        out
    }

    /// 記錄使用者提交結果並重置輸入緩衝
    pub fn commit_text(&mut self, text: &str) {
        if text.is_empty() {
            self.clear();
            return;
        }

        let decoded = self.build_sentence_segments();
        let prev_last = self.last_committed_word.clone();
        let valid_segments: Vec<&DecodedSegment> = if decoded.is_valid {
            decoded
                .segments
                .iter()
                .filter(|seg| {
                    !seg.word.is_empty()
                        && !seg.word.is_ascii()
                        && !seg.zhuyin.is_empty()
                        && (seg.start_syl..seg.end_syl).all(|idx| {
                            decoded.syllables.get(idx).map(|s| s.is_valid).unwrap_or(false)
                        })
                })
                .collect()
        } else {
            Vec::new()
        };

        let joined_valid: String = valid_segments.iter().map(|s| s.word.as_str()).collect();
        if !valid_segments.is_empty()
            && (joined_valid == text
                || decoded
                    .segments
                    .iter()
                    .map(|s| s.word.as_str())
                    .collect::<String>()
                    == text)
        {
            // 以標點符號為子句邊界分段學習，避免跨過逗號／句號學到錯誤的二元詞組搭配
            let mut clause_items: Vec<(&str, &str)> = Vec::new();
            let mut clause_prev = prev_last.as_deref();
            for seg in &decoded.segments {
                let is_valid_zh = !seg.word.is_empty()
                    && !seg.word.is_ascii()
                    && !seg.zhuyin.is_empty()
                    && (seg.start_syl..seg.end_syl).all(|idx| {
                        decoded
                            .syllables
                            .get(idx)
                            .map(|s| s.is_valid && !s.is_punctuation)
                            .unwrap_or(false)
                    });
                if is_valid_zh {
                    clause_items.push((seg.zhuyin.as_str(), seg.word.as_str()));
                } else if !clause_items.is_empty() {
                    self.user_db
                        .record_commit_with_readings(clause_prev, &clause_items);
                    clause_items.clear();
                    clause_prev = None;
                }
            }
            if !clause_items.is_empty() {
                self.user_db
                    .record_commit_with_readings(clause_prev, &clause_items);
            }

            // 使用者手動選過字的區段額外強化一次讀音 Unigram 權重，確保下次直接偏好該字詞
            for seg in &valid_segments {
                if self
                    .user_overrides
                    .iter()
                    .any(|(s, e, _)| *s == seg.start_syl && *e == seg.end_syl)
                {
                    self.user_db
                        .record_commit_with_readings(None, &[(seg.zhuyin.as_str(), seg.word.as_str())]);
                }
            }
            self.last_committed_word = if decoded.syllables.last().map_or(false, |s| s.is_punctuation) {
                None
            } else {
                valid_segments.last().map(|s| s.word.clone())
            };
        } else if decoded.is_valid && valid_segments.len() == 1 && !text.is_ascii() {
            let zy = valid_segments[0].zhuyin.as_str();
            self.user_db
                .record_commit_with_readings(prev_last.as_deref(), &[(zy, text)]);
            if valid_segments[0].word != text {
                self.user_db.record_commit_with_readings(None, &[(zy, text)]);
            }
            self.last_committed_word = Some(text.to_string());
        } else {
            self.user_db
                .record_commit_with_prev(prev_last.as_deref(), &[text]);
            self.last_committed_word = Some(text.to_string());
        }

        self.clear();
    }

    fn prefix_ends_with_phrase(&self, syls: &[SyllableBucket]) -> bool {
        let n = syls.len();
        if n < 2 {
            return false;
        }
        let start_min = n.saturating_sub(8);
        (start_min..n - 1).any(|start| {
            if syls[start..n].iter().any(|s| !s.is_valid) {
                return false;
            }
            let zy: String = syls[start..n]
                .iter()
                .map(|s| s.to_canonical_string())
                .collect();
            !self.chinese_lexicon.find_exact(&zy).is_empty()
                || !self.custom_dict.query(&zy).is_empty()
        })
    }

    fn parse_zhuyin_with_cursor(&self) -> (Vec<SyllableBucket>, Option<usize>, bool) {
        let Some(b) = self.cursor_byte else {
            let (syllables, is_valid) = self.parse_zhuyin_segments(&self.raw_buffer);
            return (syllables, None, is_valid);
        };

        if self.raw_buffer.is_empty()
            || self
                .raw_buffer
                .chars()
                .next()
                .map_or(false, |c| c.is_uppercase())
        {
            return (Vec::new(), Some(0), false);
        }

        let b = b.min(self.raw_buffer.len());
        let (p_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer[..b]);
        let (mut s_syls, _) = self.parse_zhuyin_segments(&self.raw_buffer[b..]);
        for s in &mut s_syls {
            s.raw_start += b;
            s.raw_end += b;
        }
        let cursor_syl_idx = p_syls.len();
        let mut all = p_syls;
        all.extend(s_syls);

        if all.is_empty() {
            return (all, Some(0), false);
        }

        let is_valid = if let Some(first_bad) = all.iter().position(|s| !s.is_valid) {
            let has_explicit_tone = all.iter().any(|s| s.is_valid && s.has_explicit_tone);
            let is_exact_english = self.raw_buffer.chars().all(|c| c.is_ascii_alphabetic())
                && !self
                    .english_lexicon
                    .find_exact(&self.raw_buffer.to_ascii_lowercase())
                    .is_empty();
            let has_multi_phrase = !is_exact_english
                && (0..all.len()).any(|i| {
                    ((i + 2)..=all.len().min(i + 8)).any(|j| {
                        if all[i..j].iter().any(|s| !s.is_valid) {
                            return false;
                        }
                        let zy: String = all[i..j].iter().map(|s| s.to_canonical_string()).collect();
                        !self.chinese_lexicon.find_exact(&zy).is_empty()
                            || !self.custom_dict.query(&zy).is_empty()
                    })
                });
            let _ = first_bad;
            has_explicit_tone || has_multi_phrase
        } else {
            true
        };

        (all, Some(cursor_syl_idx), is_valid)
    }

    /// 以 Viterbi DP 演算法對音節序列進行最佳斷詞與組字
    fn build_sentence_segments(&self) -> SentenceDecodeResult {
        let (syllables, cursor_syl_idx, parsed_valid) = self.parse_zhuyin_with_cursor();
        let n = syllables.len();
        let is_valid = parsed_valid || (!self.user_overrides.is_empty() && n > 0);
        if !is_valid || n == 0 {
            return SentenceDecodeResult {
                segments: Vec::new(),
                syllables,
                resolved_len: 0,
                cursor_syl_idx,
                is_valid,
            };
        }

        // 若輸入為 >= 3 字元的純英文字母且無任何聲調、無多音節中文詞組，且未手動覆寫區段，判定為英文優先
        if self.user_overrides.is_empty()
            && self.raw_buffer.len() >= 3
            && self.raw_buffer.chars().all(|c| c.is_ascii_alphabetic())
        {
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
            let is_english = !self.english_lexicon.find_exact(&raw_lower).is_empty()
                || (n >= 2 && !self.english_lexicon.find_prefix(&raw_lower, 1).is_empty());
            if !has_any_multi_phrase && is_english {
                return SentenceDecodeResult {
                    segments: Vec::new(),
                    syllables,
                    resolved_len: 0,
                    cursor_syl_idx,
                    is_valid: false,
                };
            }
        }

        // 若游標正在句中鍵入尚未閉合的新音節，暫將該音節保留為原按鍵文字而不強行轉中文
        let unclosed_mid_syl = if let Some(c_syl) = cursor_syl_idx {
            if self.is_editing_mid_syllable && c_syl > 0 && c_syl <= n {
                let p_slice = &syllables[..c_syl];
                let p_last = &p_slice[c_syl - 1];
                let has_ov = self.user_overrides.iter().any(|(_, end, _)| *end == c_syl);
                if p_last.has_explicit_tone
                    || !p_last.is_valid
                    || has_ov
                    || self.prefix_ends_with_phrase(p_slice)
                {
                    None
                } else {
                    Some(c_syl - 1)
                }
            } else {
                None
            }
        } else {
            None
        };

        // 檢查是否處於「中英混打句尾英文」：最後一個明確聲調/覆寫之後是 >= 3 字元的純英文字母、無任何多音節中文詞組且命中英文詞庫
        let mixed_english_tail_syl = if cursor_syl_idx.is_none() && n >= 2 {
            let last_boundary = (0..n)
                .rev()
                .find(|&idx| {
                    syllables[idx].has_explicit_tone
                        || self.user_overrides.iter().any(|(_, e, _)| *e == idx + 1)
                })
                .map(|idx| idx + 1)
                .unwrap_or(0);
            if last_boundary > 0 && last_boundary < n {
                let tail_start_byte = syllables[last_boundary].raw_start;
                let tail_raw = &self.raw_buffer[tail_start_byte..];
                let has_ov_in_tail = self.user_overrides.iter().any(|(_, e, _)| *e > last_boundary);
                if !has_ov_in_tail
                    && tail_raw.len() >= 3
                    && tail_raw.chars().all(|c| c.is_ascii_alphabetic())
                {
                    let tail_has_multi_phrase = (last_boundary..n).any(|i| {
                        ((i + 2)..=n.min(i + 8)).any(|j| {
                            syllables[i..j].iter().all(|s| s.is_valid) && {
                                let zy: String = syllables[i..j]
                                    .iter()
                                    .map(|s| s.to_canonical_string())
                                    .collect();
                                !self.chinese_lexicon.find_exact(&zy).is_empty()
                                    || !self.custom_dict.query(&zy).is_empty()
                            }
                        })
                    });
                    let tail_lower = tail_raw.to_ascii_lowercase();
                    let tail_syl_count = n - last_boundary;
                    let tail_is_english = !self.english_lexicon.find_exact(&tail_lower).is_empty()
                        || (tail_syl_count >= 2
                            && !self.english_lexicon.find_prefix(&tail_lower, 1).is_empty());
                    if !tail_has_multi_phrase && tail_is_english {
                        Some(last_boundary)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // 計算應轉換為中文的已結算音節長度 resolved_len：
        let last_has_tone = syllables[n - 1].has_explicit_tone;
        let last_is_invalid = !syllables[n - 1].is_valid;
        let has_multi_phrase_ending = self.prefix_ends_with_phrase(&syllables);
        let has_override_at_end = self.user_overrides.iter().any(|(_, end, _)| *end == n);

        let max_override_end = self
            .user_overrides
            .iter()
            .map(|(_, end, _)| *end)
            .max()
            .unwrap_or(0);
        let resolved_len = if let Some(tail_syl) = mixed_english_tail_syl {
            tail_syl
        } else if last_has_tone
            || last_is_invalid
            || has_multi_phrase_ending
            || has_override_at_end
            || (cursor_syl_idx.is_some() && unclosed_mid_syl != Some(n - 1))
        {
            n
        } else {
            n.saturating_sub(1).max(max_override_end).min(n)
        };

        if resolved_len == 0 {
            return SentenceDecodeResult {
                segments: Vec::new(),
                syllables,
                resolved_len: 0,
                cursor_syl_idx,
                is_valid,
            };
        }

        // Viterbi DP: 尋找 0..=resolved_len 最高分斷詞路徑（錯字區段與句中未閉合輸入保持原按鍵並隔離不跨越合併；手動覆寫可跨越英文或錯字區段）
        let mut dp_score = vec![f64::NEG_INFINITY; resolved_len + 1];
        let mut dp_prev: Vec<Option<(usize, String, String)>> = vec![None; resolved_len + 1];
        dp_score[0] = 0.0;

        for j in 1..=resolved_len {
            // 先檢查是否有使用者明確鎖定的區段剛好結束於 j（允許覆寫英文單字或無效音節區段）
            for (ov_s, ov_e, ov_word) in &self.user_overrides {
                if *ov_e == j && *ov_s < j && dp_score[*ov_s] != f64::NEG_INFINITY {
                    let cand_score = dp_score[*ov_s] + 200_000.0;
                    if cand_score > dp_score[j] {
                        let span_zy: String = syllables[*ov_s..j]
                            .iter()
                            .filter(|s| s.is_valid)
                            .map(|s| s.to_canonical_string())
                            .collect();
                        dp_score[j] = cand_score;
                        dp_prev[j] = Some((*ov_s, ov_word.clone(), span_zy));
                    }
                }
            }

            if !syllables[j - 1].is_valid
                || syllables[j - 1].is_punctuation
                || unclosed_mid_syl == Some(j - 1)
            {
                if dp_score[j - 1] != f64::NEG_INFINITY && dp_score[j] == f64::NEG_INFINITY {
                    let raw_slice =
                        self.raw_buffer[syllables[j - 1].raw_start..syllables[j - 1].raw_end].to_string();
                    dp_score[j] = dp_score[j - 1];
                    dp_prev[j] = Some((j - 1, raw_slice, String::new()));
                }
                continue;
            }
            let i_min = j.saturating_sub(8);
            for i in (i_min..j).rev() {
                if !syllables[i].is_valid || syllables[i].is_punctuation || unclosed_mid_syl == Some(i) {
                    break;
                }
                if dp_score[i] == f64::NEG_INFINITY {
                    continue;
                }
                let span_zy: String = syllables[i..j]
                    .iter()
                    .map(|s| s.to_canonical_string())
                    .collect();
                let dp_prev_word = if i > 0 {
                    dp_prev[i].as_ref().map(|(_, w, _)| w.as_str())
                } else {
                    None
                };

                if let Some((word, edge_score)) =
                    self.best_word_for_span(i, j, &span_zy, dp_prev_word)
                {
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
                    zhuyin: zy.clone(),
                });
                curr = prev_idx;
            } else {
                // 若某單音節無精確匹配，以原音節文字保底
                let fallback_zy = syllables[curr - 1].to_canonical_string();
                segments.push(DecodedSegment {
                    start_syl: curr - 1,
                    end_syl: curr,
                    word: fallback_zy.clone(),
                    zhuyin: fallback_zy,
                });
                curr -= 1;
            }
        }
        segments.reverse();

        SentenceDecodeResult {
            segments,
            syllables,
            resolved_len,
            cursor_syl_idx,
            is_valid,
        }
    }

    fn best_word_for_span(
        &self,
        start: usize,
        end: usize,
        zhuyin: &str,
        prev_word_override: Option<&str>,
    ) -> Option<(String, f64)> {
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

        let prev_word = prev_word_override.or(self.last_committed_word.as_deref());
        let span_len = end - start;
        let phrase_bonus = if span_len >= 2 {
            (span_len - 1) as f64 * 2300.0
        } else {
            0.0
        };

        // 3. 內建大詞庫精確匹配：
        // 同區段同音字競爭使用完整讀音自適應加權；跨區段 DP 邊權則限制單字 Unigram 加權，避免高頻單字（如「一」「個」「的」）拆散常見詞組
        let exact = self.chinese_lexicon.find_exact(zhuyin);
        if let Some((best_w, _sel_score, edge_score)) = exact
            .into_iter()
            .map(|(w, base)| {
                let (uni, bi) = self
                    .user_db
                    .get_adaptive_boost_for_reading(zhuyin, &w, prev_word);
                let selection_score = base + uni + bi;
                let viterbi_uni = if span_len == 1 {
                    uni.min(150.0)
                } else {
                    uni.min(600.0 * span_len as f64)
                };
                let dp_edge = base + phrase_bonus + bi + viterbi_uni;
                (w, selection_score, dp_edge)
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            return Some((best_w, edge_score));
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
                    if !ch.is_ascii() && !ch.is_whitespace() {
                        syllables.push(SyllableBucket::punctuation(idx, next_idx));
                    } else {
                        syllables.push(SyllableBucket::invalid_raw(idx, next_idx));
                    }
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
            if !syllables[i].is_valid || syllables[i].is_punctuation {
                continue;
            }
            let is_closed = i < n - 1 || syllables[i].has_explicit_tone;
            if is_closed {
                let canon = syllables[i].to_canonical_string();
                if syllables[i].medial.is_none() && syllables[i].final_sym.is_none() {
                    let is_first_tone_bopomofo = syllables[i].has_explicit_tone
                        && syllables[i].tone == Some(Tone::First)
                        && !self.chinese_lexicon.find_exact(&canon).is_empty();
                    if !is_first_tone_bopomofo
                        && !syllables[i].initial.map_or(false, can_initial_stand_alone)
                    {
                        syllables[i].is_valid = false;
                        continue;
                    }
                }
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
            let has_explicit_tone_anywhere = syllables
                .iter()
                .any(|s| s.is_valid && !s.is_punctuation && s.has_explicit_tone);
            let is_exact_english_word = input.chars().all(|c| c.is_ascii_alphabetic())
                && !self
                    .english_lexicon
                    .find_exact(&input.to_ascii_lowercase())
                    .is_empty();
            let has_multi_phrase_prefix = !is_exact_english_word
                && (0..syllables.len()).any(|i| {
                    ((i + 2)..=syllables.len().min(i + 8)).any(|j| {
                        syllables[i..j]
                            .iter()
                            .all(|s| s.is_valid && !s.is_punctuation)
                            && {
                                let zy: String = syllables[i..j]
                                    .iter()
                                    .map(|s| s.to_canonical_string())
                                    .collect();
                                !self.chinese_lexicon.find_exact(&zy).is_empty()
                                    || !self.custom_dict.query(&zy).is_empty()
                            }
                    })
                });
            let _ = first_bad;
            has_explicit_tone_anywhere || has_multi_phrase_prefix
        } else {
            true
        };

        (syllables, is_valid)
    }
}
