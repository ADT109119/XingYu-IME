pub mod layout;
pub mod dachen;
pub mod eten;
pub mod pinyin;

pub use layout::{KeyboardLayout, KeyMapping, Tone, ZhuyinSymbol};
pub use dachen::DaChenLayout;
pub use eten::EtenLayout;
pub use pinyin::PinyinLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutType {
    DaChen,
    Eten,
    Pinyin,
}

pub fn create_layout(layout_type: LayoutType) -> Box<dyn KeyboardLayout> {
    match layout_type {
        LayoutType::DaChen => Box::new(DaChenLayout::new()),
        LayoutType::Eten => Box::new(EtenLayout::new()),
        LayoutType::Pinyin => Box::new(PinyinLayout::new()),
    }
}
