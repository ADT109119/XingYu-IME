pub mod engine;
pub mod ffi;
pub mod keyboard;
pub mod lexicon;
pub mod phonetics;

pub use engine::{Candidate, CandidateType, DualPathDecoder, UserAdaptiveDb};
pub use keyboard::{DaChenLayout, KeyboardLayout, LayoutType};
pub use lexicon::CustomDictionary;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dachen_layout_mapping() {
        let layout = DaChenLayout::new();
        assert_eq!(layout.name(), "DaChen (Standard)");
        match layout.map_key('a') {
            keyboard::KeyMapping::Zhuyin(s) => assert_eq!(s.as_char(), 'ㄇ'),
            _ => panic!("'a' should map to Zhuyin ㄇ"),
        }
    }

    #[test]
    fn test_mixed_typing_apple_vs_chinese() {
        let mut decoder = DualPathDecoder::new();

        // 模擬輸入 'apple'
        for ch in "apple".chars() {
            decoder.push_char(ch);
        }

        let candidates = decoder.get_candidates(5);
        assert!(!candidates.is_empty());
        // 因為 apple 鍵位為 ㄇㄣㄣㄠㄍ（連續兩個韻母 ㄣㄣ 違反音律），所以英文 'apple' 必定排在最前！
        assert_eq!(candidates[0].text, "apple");
        assert_eq!(candidates[0].candidate_type, CandidateType::English);
    }

    #[test]
    fn test_chinese_typing_hello() {
        let mut decoder = DualPathDecoder::new();

        // 模擬輸入 '你好' (ㄋㄧˇㄏㄠˇ -> 's' 'u' '3' 'c' 'l' '3')
        for ch in "su3cl3".chars() {
            decoder.push_char(ch);
        }

        let candidates = decoder.get_candidates(5);
        assert!(!candidates.is_empty());
        // 應命中中文 "你好"
        assert_eq!(candidates[0].text, "你好");
        assert_eq!(candidates[0].candidate_type, CandidateType::Chinese);
    }

    #[test]
    fn test_shift_uppercase_english() {
        let mut decoder = DualPathDecoder::new();

        // 模擬 Shift + A + p + p + l + e
        for ch in "Apple".chars() {
            decoder.push_char(ch);
        }

        // 行內預編輯應為 Apple
        assert_eq!(decoder.get_preedit_string(), "Apple");

        let candidates = decoder.get_candidates(5);
        assert!(!candidates.is_empty());
        assert_eq!(candidates[0].text, "Apple");
        assert_eq!(candidates[0].candidate_type, CandidateType::English);
    }

    #[test]
    fn test_daily_conversational_phrases() {
        let mut d1 = DualPathDecoder::new();
        // 測試 "辛苦了" (ㄒㄧㄣㄎㄨˇㄌㄜ˙ -> vupdj3xk7)
        for ch in "vupdj3xk7".chars() {
            d1.push_char(ch);
        }
        let c1 = d1.get_candidates(5);
        assert!(!c1.is_empty());
        assert_eq!(c1[0].text, "辛苦了");

        let mut d2 = DualPathDecoder::new();
        // 測試 "今天" (ㄐㄧㄣㄊㄧㄢ -> rupwu0)
        for ch in "rupwu0".chars() {
            d2.push_char(ch);
        }
        let c2 = d2.get_candidates(5);
        assert!(!c2.is_empty());
        assert_eq!(c2[0].text, "今天");
    }

    #[test]
    fn test_out_of_order_typing_tolerance() {
        // 1. 測試介音先於聲母：'uf4' (ㄧ ㄑ ˋ) -> 應自動歸位為 'ㄑㄧˋ' (氣)
        let mut d1 = DualPathDecoder::new();
        for ch in "uf4".chars() {
            d1.push_char(ch);
        }
        let c1 = d1.get_candidates(5);
        assert!(!c1.is_empty(), "uf4 candidates should not be empty");
        assert_eq!(c1[0].text, "氣");

        // 2. 測試韻母先於聲母：'8w' (ㄚ ㄊ) -> 應自動歸位為 'ㄊㄚ' (他)
        let mut d2 = DualPathDecoder::new();
        for ch in "8w".chars() {
            d2.push_char(ch);
        }
        let c2 = d2.get_candidates(5);
        assert!(!c2.is_empty(), "8w candidates should not be empty");
        assert_eq!(c2[0].text, "他");

        // 3. 測試多音節顛倒輸入：'us3lc3' (ㄧ ㄋ ˇ  ㄠ ㄏ ˇ) -> 應自動歸位為 'ㄋㄧˇㄏㄠˇ' (你好)
        let mut d3 = DualPathDecoder::new();
        for ch in "us3lc3".chars() {
            d3.push_char(ch);
        }
        let c3 = d3.get_candidates(5);
        assert!(!c3.is_empty(), "us3lc3 candidates should not be empty");
        assert_eq!(c3[0].text, "你好");
    }

    #[test]
    fn test_custom_dictionary_priority() {
        let mut decoder = DualPathDecoder::new();

        // 加入自訂詞庫內容
        let custom_tsv = "人工智慧\tㄖㄣˊ ㄍㄨㄥ ㄓˋ ㄏㄨㄟˋ\t5000\nJerry\tJerry\t5000\n";
        decoder.custom_dict.parse_content(custom_tsv);

        // 測試輸入 "jerry"
        for ch in "jerry".chars() {
            decoder.push_char(ch);
        }

        let candidates = decoder.get_candidates(5);
        assert!(!candidates.is_empty());
        // 自訂詞條 "Jerry" 享有額外 10000 權重，必排在第 1 位
        assert_eq!(candidates[0].text, "Jerry");
        assert_eq!(candidates[0].candidate_type, CandidateType::Custom);
    }

    #[test]
    fn test_user_adaptive_learning() {
        let mut decoder = DualPathDecoder::new();
        decoder.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 模擬使用者反覆選用並輸出 "智慧"
        for _ in 0..5 {
            decoder.commit_text("智慧");
        }

        let boost = decoder.user_db.get_adaptive_boost("智慧", None);
        assert!(boost > 0.0);

        let (unigrams, _) = decoder.user_db.get_stats();
        assert!(unigrams >= 1);

        assert!(decoder.user_db.clear_all().is_ok());
        let (unigrams_after, _) = decoder.user_db.get_stats();
        assert_eq!(unigrams_after, 0);
    }

    #[test]
    fn test_continuous_sentence_and_three_char_words() {
        // 1. 連續整句輸入免停頓選字：我今天很好 (ji3rupwu0cp3cl3)
        let mut d1 = DualPathDecoder::new();
        for ch in "ji3rupwu0cp3cl3".chars() {
            d1.push_char(ch);
        }
        assert_eq!(d1.get_preedit_string(), "我今天很好");

        // 2. 3 字詞與連續長句：我不知道 (ji31j452l4)
        let mut d2 = DualPathDecoder::new();
        for ch in "ji31j452l4".chars() {
            d2.push_char(ch);
        }
        assert_eq!(d2.get_preedit_string(), "我不知道");

        // 3. 3 字詞：輸入法 (gjbj4z83 -> ㄕㄨㄖㄨˋㄈㄚˇ)
        let mut d3 = DualPathDecoder::new();
        for ch in "gjbj4z83".chars() {
            d3.push_char(ch);
        }
        assert_eq!(d3.get_preedit_string(), "輸入法");
    }

    #[test]
    fn test_segment_cursor_and_reselection() {
        let mut decoder = DualPathDecoder::new();
        // 輸入 "我今天"
        for ch in "ji3rupwu0".chars() {
            decoder.push_char(ch);
        }
        assert_eq!(decoder.get_preedit_string(), "我今天");

        // 向左移動游標選詞 -> 鎖定 "今天"，再向左移動 -> 鎖定 "我"
        assert!(decoder.move_cursor_left());
        let snap1 = decoder.get_composition_snapshot(9);
        assert_eq!(snap1.cursor_utf16, 1); // "今天" 起始位置
        assert_eq!(snap1.selection_len_utf16, 2);

        assert!(decoder.move_cursor_left());
        let snap2 = decoder.get_composition_snapshot(9);
        assert_eq!(snap2.cursor_utf16, 0); // "我" 起始位置
        assert_eq!(snap2.selection_len_utf16, 1);
        assert!(snap2.candidates.len() >= 2);

        // 重選第 2 候選字替換 "我"（例如 "偶" 或同音字）
        let chosen = snap2.candidates[1].text.clone();
        decoder.apply_candidate_selection(1);
        assert_eq!(decoder.get_preedit_string(), format!("{}今天", chosen));
    }

    #[test]
    fn test_english_words_and_space_tone_behavior() {
        // 英文單字 hello, code, test, swift 應維持英文
        for word in ["hello", "code", "test", "swift", "apple"] {
            let mut d = DualPathDecoder::new();
            for ch in word.chars() {
                d.push_char(ch);
            }
            let snap = d.get_composition_snapshot(5);
            assert_eq!(snap.preedit, word, "preedit failed for {}", word);
            assert!(!snap.should_space_act_as_tone, "space should commit english for {}", word);
        }

        // 未標調中文音節 w8 (ㄊㄚ) 按空白鍵應作為一聲
        let mut d_zh = DualPathDecoder::new();
        for ch in "w8".chars() {
            d_zh.push_char(ch);
        }
        let snap_before = d_zh.get_composition_snapshot(5);
        assert_eq!(snap_before.preedit, "w8");
        assert!(snap_before.should_space_act_as_tone);

        d_zh.push_char(' ');
        let snap_after = d_zh.get_composition_snapshot(5);
        assert_eq!(snap_after.preedit, "他");
        assert!(!snap_after.should_space_act_as_tone);
    }

    #[test]
    fn test_multi_syllable_space_first_tone_and_pagination_selection() {
        // 1. Bug fix check: typing "2u4u" then Space (' ') for 1st tone ("第一")
        let mut d = DualPathDecoder::new();
        for ch in "2u4u".chars() {
            d.push_char(ch);
        }
        let snap_before_space = d.get_composition_snapshot(200);
        assert_eq!(snap_before_space.preedit, "第一");
        assert!(
            snap_before_space.should_space_act_as_tone,
            "trailing un-toned 'u' in 2u4u must treat Space as 1st tone instead of premature commit"
        );

        d.push_char(' ');
        let snap_after_space = d.get_composition_snapshot(200);
        assert_eq!(snap_after_space.preedit, "第一");
        assert!(!snap_after_space.should_space_act_as_tone);
        assert_eq!(snap_after_space.syllable_count, 2);
        // Candidates should include exact matches AND single-char split candidates (e.g. 地/第) past top 4
        assert!(snap_after_space.candidates.len() > 9);
        assert!(snap_after_space.candidates.iter().any(|c| c.text == "地衣"));
        let di_idx = snap_after_space
            .candidates
            .iter()
            .position(|c| c.text == "地")
            .expect("'地' should be selectable as first character");
        assert!(d.apply_candidate_selection(di_idx).is_some());
        assert!(d.get_preedit_string().starts_with('地'));
        assert!(d.has_active_cursor(), "remaining syllable should remain editable after shortening phrase");

        // Selecting the remaining syllable ("衣") should finish the composition and clear active cursor
        let snap_rem = d.get_composition_snapshot(200);
        let yi_idx = snap_rem
            .candidates
            .iter()
            .position(|c| c.text == "衣")
            .expect("'衣' should be selectable for remaining syllable");
        assert!(d.apply_candidate_selection(yi_idx).is_some());
        assert_eq!(d.get_preedit_string(), "地衣");
        assert!(!d.has_active_cursor(), "active cursor should clear after selecting final segment");

        // 2. Pagination check: homophones > 9 (e.g. u4 = ㄧˋ has dozens of chars)
        let mut d_page = DualPathDecoder::new();
        for ch in "u4".chars() {
            d_page.push_char(ch);
        }
        let snap_page = d_page.get_composition_snapshot(200);
        assert!(
            snap_page.candidates.len() > 20,
            "expected > 20 homophones for u4, got {}",
            snap_page.candidates.len()
        );
        let cand_12 = snap_page.candidates[12].text.clone();
        assert!(d_page.apply_candidate_selection(12).is_some());
        assert_eq!(d_page.get_preedit_string(), cand_12);

        // 3. Expanded lexicon check (newly merged McBopomofo phrases: 捉迷藏 = ㄓㄨㄛㄇㄧˊㄘㄤˊ -> 5ji au6h;6)
        let mut d_lex = DualPathDecoder::new();
        for ch in "5ji au6h;6".chars() {
            d_lex.push_char(ch);
        }
        assert_eq!(d_lex.get_preedit_string(), "捉迷藏");
    }

    #[test]
    fn test_long_sentence_typo_preserves_resolved_chinese_prefix() {
        // 1. 長句尾端多按聲調鍵（例如 "我今天4" -> ji3rupwu0 4）不可讓前面的「我今天」變成英文
        let mut d1 = DualPathDecoder::new();
        for ch in "ji3rupwu0 4".chars() {
            d1.push_char(ch);
        }
        let snap1 = d1.get_composition_snapshot(20);
        assert!(snap1.is_valid_zhuyin);
        assert_eq!(snap1.preedit, "我今天4");

        // 2. 長句尾端打錯無效注音（例如 "我今天q3" -> ji3rupwu0q3）應保留「我今天」
        let mut d2 = DualPathDecoder::new();
        for ch in "ji3rupwu0q3".chars() {
            d2.push_char(ch);
        }
        let snap2 = d2.get_composition_snapshot(20);
        assert!(snap2.is_valid_zhuyin);
        assert_eq!(snap2.preedit, "我今天q3");

        // 3. 句中打錯一鍵後繼續輸入（例如 "我今天" + "q" + "很好" -> ji3rupwu0qcp3cl3）前後中文皆保留
        let mut d3 = DualPathDecoder::new();
        for ch in "ji3rupwu0qcp3cl3".chars() {
            d3.push_char(ch);
        }
        let snap3 = d3.get_composition_snapshot(20);
        assert!(snap3.is_valid_zhuyin);
        assert_eq!(snap3.preedit, "我今天q很好");

        // 4. 純英文單字仍應正確走英文路徑，不誤判為中文
        for eng in ["hello", "test", "keyboard", "apple", "a1b2"] {
            let mut d_eng = DualPathDecoder::new();
            for ch in eng.chars() {
                d_eng.push_char(ch);
            }
            let snap_eng = d_eng.get_composition_snapshot(10);
            assert!(!snap_eng.is_valid_zhuyin, "expected English mode for {}", eng);
            assert_eq!(snap_eng.preedit, eng);
        }
    }

    #[test]
    fn test_polyphonic_defaults_phrase_scoring_and_sentence_learning() {
        let mut dec = DualPathDecoder::new();
        dec.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 1. "m4" (ㄩˋ) 不應預設為「與」(ㄩˇ 為主讀音)
        for ch in "m4".chars() {
            dec.push_char(ch);
        }
        let snap_yu4 = dec.get_composition_snapshot(10);
        assert!(snap_yu4.is_valid_zhuyin);
        assert_ne!(
            snap_yu4.preedit, "與",
            "m4 (ㄩˋ) should not default to 與, got {}",
            snap_yu4.preedit
        );
        dec.clear();

        // 2. 常見雙字詞應優先於兩個高頻單字拆開（修正 著因/鬼一/與社）
        for (keys, expected) in [("5j4up", "注音"), ("ejo3u4", "詭異"), ("m4gk4", "預設")] {
            let mut d = DualPathDecoder::new();
            d.user_db = UserAdaptiveDb::in_memory().unwrap();
            for ch in keys.chars() {
                d.push_char(ch);
            }
            let snap = d.get_composition_snapshot(10);
            assert_eq!(
                snap.preedit, expected,
                "expected '{}' for keys '{}', got '{}'",
                expected, keys, snap.preedit
            );
        }

        // 3. 整句提交應同時寫入個別分詞 Unigram 與句內 Bigram，且選字後立刻生效
        for ch in "5j4upm4gk4".chars() {
            dec.push_char(ch);
        }
        let sentence = dec.get_composition_snapshot(10).preedit;
        assert_eq!(sentence, "注音預設");
        dec.commit_text(&sentence);
        let (unigrams, bigrams) = dec.user_db.get_stats();
        assert!(unigrams >= 2, "expected >=2 unigrams, got {}", unigrams);
        assert!(bigrams >= 1, "expected >=1 bigrams recorded, got {}", bigrams);

        // 4. 手動改選「育」(m4 -> ㄩˋ) 並提交後，下次輸入 m4 應立即優先出現「育」
        for ch in "m4".chars() {
            dec.push_char(ch);
        }
        let yu_idx = dec
            .get_composition_snapshot(20)
            .candidates
            .iter()
            .position(|c| c.text == "育")
            .expect("育 should be a candidate for m4");
        dec.apply_candidate_selection(yu_idx);
        let committed_yu = dec.get_composition_snapshot(10).preedit;
        assert_eq!(committed_yu, "育");
        dec.commit_text(&committed_yu);

        for ch in "m4".chars() {
            dec.push_char(ch);
        }
        assert_eq!(
            dec.get_composition_snapshot(10).preedit,
            "育",
            "learned character 育 should become default after manual selection"
        );
    }
}
