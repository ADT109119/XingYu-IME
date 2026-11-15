use super::layout::{KeyMapping, KeyboardLayout, Tone};

/// 漢語拼音配置擴充介面（保留預備）
pub struct PinyinLayout;

impl PinyinLayout {
    pub fn new() -> Self {
        Self
    }
}

impl KeyboardLayout for PinyinLayout {
    fn name(&self) -> &'static str {
        "HanYu Pinyin (Reserved)"
    }

    fn map_key(&self, key: char) -> KeyMapping {
        // 未來擴充拼音映射：例如直接傳遞英文小寫字母與數字聲調 1-5
        if key.is_ascii_alphabetic() {
            KeyMapping::English(key.to_ascii_lowercase())
        } else {
            KeyMapping::Ignored
        }
    }

    fn map_tone(&self, key: char) -> Option<Tone> {
        match key {
            '1' => Some(Tone::First),
            '2' => Some(Tone::Second),
            '3' => Some(Tone::Third),
            '4' => Some(Tone::Fourth),
            '5' => Some(Tone::Fifth),
            _ => None,
        }
    }
}
