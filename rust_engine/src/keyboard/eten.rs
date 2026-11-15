use super::layout::{KeyMapping, KeyboardLayout, Tone};

/// 倚天 26 鍵 / 41 鍵注音配置擴充介面（保留預備）
pub struct EtenLayout;

impl EtenLayout {
    pub fn new() -> Self {
        Self
    }
}

impl KeyboardLayout for EtenLayout {
    fn name(&self) -> &'static str {
        "Eten 26-Key (Reserved)"
    }

    fn map_key(&self, _key: char) -> KeyMapping {
        // 未來擴充倚天鍵盤映射
        KeyMapping::Ignored
    }

    fn map_tone(&self, _key: char) -> Option<Tone> {
        None
    }
}
