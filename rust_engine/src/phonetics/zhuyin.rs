use crate::keyboard::layout::{Tone, ZhuyinSymbol};

/// 代表一個完整的注音音節：[聲母] + [介音] + [韻母] + 聲調
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ZhuyinSyllable {
    pub initial: Option<ZhuyinSymbol>,
    pub medial: Option<ZhuyinSymbol>,
    pub final_symbol: Option<ZhuyinSymbol>,
    pub tone: Option<Tone>,
}

impl ZhuyinSyllable {
    pub fn new() -> Self {
        Self::default()
    }

    /// 檢查是否完全為空
    pub fn is_empty(&self) -> bool {
        self.initial.is_none()
            && self.medial.is_none()
            && self.final_symbol.is_none()
            && self.tone.is_none()
    }

    /// 嘗試推入一個注音符號或聲調。
    /// 遵循漢語音律學規則 (Phonotactics)：
    /// 1. 聲母只能有一個，且必須在最前面。
    /// 2. 介音只能有一個，且介音不可重複。
    /// 3. 韻母只能有一個。
    /// 4. 某些特定組合在國語語音中為不可能的音節（例如 ㄅ + ㄩ，ㄓ + ㄧ 等）。
    /// 如果符合語音規則，推入並返回 true；若非法則返回 false。
    pub fn try_push(&mut self, symbol: ZhuyinSymbol) -> bool {
        if symbol.is_initial() {
            if self.initial.is_none() && self.medial.is_none() && self.final_symbol.is_none() {
                self.initial = Some(symbol);
                return true;
            }
            return false;
        }

        if symbol.is_medial() {
            if self.medial.is_none() && self.final_symbol.is_none() {
                // 檢查聲母與介音是否衝突（例如 ㄅ/ㄆ/ㄇ/ㄈ 不能接 ㄩ）
                if let Some(init) = self.initial {
                    if symbol == ZhuyinSymbol::V {
                        // 雙唇音/唇齒音不與撮口呼(ㄩ)相拼
                        if matches!(init, ZhuyinSymbol::B | ZhuyinSymbol::P | ZhuyinSymbol::M | ZhuyinSymbol::F) {
                            return false;
                        }
                        // 舌尖後音(ㄓㄔㄕㄖ)、舌尖前音(ㄗㄘㄙ)不與 ㄩ 相拼 (需為 ㄐㄑㄒ)
                        if matches!(init, ZhuyinSymbol::Zh | ZhuyinSymbol::Ch | ZhuyinSymbol::Sh | ZhuyinSymbol::R
                            | ZhuyinSymbol::Z | ZhuyinSymbol::C | ZhuyinSymbol::S
                            | ZhuyinSymbol::G | ZhuyinSymbol::K | ZhuyinSymbol::H
                            | ZhuyinSymbol::D | ZhuyinSymbol::T) {
                            return false;
                        }
                    }
                }
                self.medial = Some(symbol);
                return true;
            }
            return false;
        }

        if symbol.is_final() {
            if self.final_symbol.is_none() {
                // 介音與韻母組合合理性檢查
                if let Some(med) = self.medial {
                    if med == ZhuyinSymbol::I && symbol == ZhuyinSymbol::Er {
                        return false; // ㄧㄦ 不存在
                    }
                }
                self.final_symbol = Some(symbol);
                return true;
            }
            return false;
        }

        if let ZhuyinSymbol::Tone(t) = symbol {
            // 聲調必須在有任何音素存在時才可以標記
            if !self.is_empty() && self.tone.is_none() {
                self.tone = Some(t);
                return true;
            }
            return false;
        }

        false
    }

    /// 設定聲調
    pub fn set_tone(&mut self, tone: Tone) -> bool {
        if !self.is_empty() {
            self.tone = Some(tone);
            true
        } else {
            false
        }
    }

    /// 轉為可視注音字串（例如 "ㄋㄧˇ"）
    pub fn to_zhuyin_string(&self) -> String {
        let mut s = String::new();
        if let Some(i) = self.initial {
            s.push(i.as_char());
        }
        if let Some(m) = self.medial {
            s.push(m.as_char());
        }
        if let Some(f) = self.final_symbol {
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
