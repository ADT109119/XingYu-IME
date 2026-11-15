use super::layout::{KeyMapping, KeyboardLayout, Tone, ZhuyinSymbol};

pub struct DaChenLayout;

impl DaChenLayout {
    pub fn new() -> Self {
        Self
    }
}

impl KeyboardLayout for DaChenLayout {
    fn name(&self) -> &'static str {
        "DaChen (Standard)"
    }

    fn map_key(&self, key: char) -> KeyMapping {
        // 若為大寫字母（Shift 組合輸入），明確視為英文字元，方便快速鍵入大寫
        if key.is_ascii_uppercase() {
            return KeyMapping::English(key);
        }

        match key.to_ascii_lowercase() {
            // 聲母 (Initials)
            '1' => KeyMapping::Zhuyin(ZhuyinSymbol::B),
            'q' => KeyMapping::Zhuyin(ZhuyinSymbol::P),
            'a' => KeyMapping::Zhuyin(ZhuyinSymbol::M),
            'z' => KeyMapping::Zhuyin(ZhuyinSymbol::F),
            '2' => KeyMapping::Zhuyin(ZhuyinSymbol::D),
            'w' => KeyMapping::Zhuyin(ZhuyinSymbol::T),
            's' => KeyMapping::Zhuyin(ZhuyinSymbol::N),
            'x' => KeyMapping::Zhuyin(ZhuyinSymbol::L),
            'e' => KeyMapping::Zhuyin(ZhuyinSymbol::G),
            'd' => KeyMapping::Zhuyin(ZhuyinSymbol::K),
            'c' => KeyMapping::Zhuyin(ZhuyinSymbol::H),
            'r' => KeyMapping::Zhuyin(ZhuyinSymbol::J),
            'f' => KeyMapping::Zhuyin(ZhuyinSymbol::Q),
            'v' => KeyMapping::Zhuyin(ZhuyinSymbol::X),
            '5' => KeyMapping::Zhuyin(ZhuyinSymbol::Zh),
            't' => KeyMapping::Zhuyin(ZhuyinSymbol::Ch),
            'g' => KeyMapping::Zhuyin(ZhuyinSymbol::Sh),
            'b' => KeyMapping::Zhuyin(ZhuyinSymbol::R),
            'y' => KeyMapping::Zhuyin(ZhuyinSymbol::Z),
            'h' => KeyMapping::Zhuyin(ZhuyinSymbol::C),
            'n' => KeyMapping::Zhuyin(ZhuyinSymbol::S),

            // 介音 (Medials)
            'u' => KeyMapping::Zhuyin(ZhuyinSymbol::I),
            'j' => KeyMapping::Zhuyin(ZhuyinSymbol::U),
            'm' => KeyMapping::Zhuyin(ZhuyinSymbol::V),

            // 韻母 (Finals)
            '8' => KeyMapping::Zhuyin(ZhuyinSymbol::A),
            'i' => KeyMapping::Zhuyin(ZhuyinSymbol::O),
            'k' => KeyMapping::Zhuyin(ZhuyinSymbol::E),
            ',' => KeyMapping::Zhuyin(ZhuyinSymbol::Eh),
            '9' => KeyMapping::Zhuyin(ZhuyinSymbol::Ai),
            'o' => KeyMapping::Zhuyin(ZhuyinSymbol::Ei),
            'l' => KeyMapping::Zhuyin(ZhuyinSymbol::Ao),
            '.' => KeyMapping::Zhuyin(ZhuyinSymbol::Ou),
            '0' => KeyMapping::Zhuyin(ZhuyinSymbol::An),
            'p' => KeyMapping::Zhuyin(ZhuyinSymbol::En),
            ';' => KeyMapping::Zhuyin(ZhuyinSymbol::Ang),
            '/' => KeyMapping::Zhuyin(ZhuyinSymbol::Eng),
            '-' => KeyMapping::Zhuyin(ZhuyinSymbol::Er),

            // 聲調
            '3' => KeyMapping::Tone(Tone::Third),
            '4' => KeyMapping::Tone(Tone::Fourth),
            '6' => KeyMapping::Tone(Tone::Second),
            '7' => KeyMapping::Tone(Tone::Fifth),
            ' ' => KeyMapping::Tone(Tone::First),

            // 其餘英文字元或符號
            c if c.is_ascii_alphabetic() => KeyMapping::English(c),
            _ => KeyMapping::Ignored,
        }
    }

    fn map_tone(&self, key: char) -> Option<Tone> {
        match key {
            ' ' => Some(Tone::First),
            '6' => Some(Tone::Second),
            '3' => Some(Tone::Third),
            '4' => Some(Tone::Fourth),
            '7' => Some(Tone::Fifth),
            _ => None,
        }
    }
}
