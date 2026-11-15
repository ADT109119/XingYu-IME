//! Keyboard Layout abstraction layer.
//! Supports Standard DaChen (大千式), and allows pluggable Eten / HanYu Pinyin.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    First,  // 陰平（1聲，通常為空白或不標）
    Second, // 陽平（2聲，ˊ）
    Third,  // 上聲（3聲，ˇ）
    Fourth, // 去聲（4聲，ˋ）
    Fifth,  // 輕聲（5聲，˙）
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZhuyinSymbol {
    // 聲母 (21)
    B, P, M, F, D, T, N, L, G, K, H, J, Q, X, Zh, Ch, Sh, R, Z, C, S,
    // 介音 (3)
    I, U, V, // ㄧ, ㄨ, ㄩ
    // 韻母 (13)
    A, O, E, Eh, Ai, Ei, Ao, Ou, An, En, Ang, Eng, Er,
    // 聲調
    Tone(Tone),
}

impl ZhuyinSymbol {
    pub fn is_initial(&self) -> bool {
        matches!(self, 
            Self::B | Self::P | Self::M | Self::F | Self::D | Self::T | Self::N | Self::L |
            Self::G | Self::K | Self::H | Self::J | Self::Q | Self::X | Self::Zh | Self::Ch |
            Self::Sh | Self::R | Self::Z | Self::C | Self::S
        )
    }

    pub fn is_medial(&self) -> bool {
        matches!(self, Self::I | Self::U | Self::V)
    }

    pub fn is_final(&self) -> bool {
        matches!(self,
            Self::A | Self::O | Self::E | Self::Eh | Self::Ai | Self::Ei | Self::Ao | Self::Ou |
            Self::An | Self::En | Self::Ang | Self::Eng | Self::Er
        )
    }

    pub fn is_tone(&self) -> bool {
        matches!(self, Self::Tone(_))
    }

    pub fn as_char(&self) -> char {
        match self {
            Self::B => 'ㄅ', Self::P => 'ㄆ', Self::M => 'ㄇ', Self::F => 'ㄈ',
            Self::D => 'ㄉ', Self::T => 'ㄊ', Self::N => 'ㄋ', Self::L => 'ㄌ',
            Self::G => 'ㄍ', Self::K => 'ㄎ', Self::H => 'ㄏ',
            Self::J => 'ㄐ', Self::Q => 'ㄑ', Self::X => 'ㄒ',
            Self::Zh => 'ㄓ', Self::Ch => 'ㄔ', Self::Sh => 'ㄕ', Self::R => 'ㄖ',
            Self::Z => 'ㄗ', Self::C => 'ㄘ', Self::S => 'ㄙ',
            Self::I => 'ㄧ', Self::U => 'ㄨ', Self::V => 'ㄩ',
            Self::A => 'ㄚ', Self::O => 'ㄛ', Self::E => 'ㄜ', Self::Eh => 'ㄝ',
            Self::Ai => 'ㄞ', Self::Ei => 'ㄟ', Self::Ao => 'ㄠ', Self::Ou => 'ㄡ',
            Self::An => 'ㄢ', Self::En => 'ㄣ', Self::Ang => 'ㄤ', Self::Eng => 'ㄥ',
            Self::Er => 'ㄦ',
            Self::Tone(Tone::First) => ' ',
            Self::Tone(Tone::Second) => 'ˊ',
            Self::Tone(Tone::Third) => 'ˇ',
            Self::Tone(Tone::Fourth) => 'ˋ',
            Self::Tone(Tone::Fifth) => '˙',
        }
    }
}

/// The result of mapping a physical key stroke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyMapping {
    Zhuyin(ZhuyinSymbol),
    Tone(Tone),
    English(char),
    Ignored,
}

pub trait KeyboardLayout: Send + Sync {
    /// 取得鍵盤配置名稱
    fn name(&self) -> &'static str;

    /// 根據實體按鍵（小寫或原始字元）映射出對應的注音符號或聲調
    fn map_key(&self, key: char) -> KeyMapping;

    /// 是否為該配置下的聲調鍵
    fn map_tone(&self, key: char) -> Option<Tone>;
}
