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
    fn test_list_and_delete_learned_entries() {
        let db = UserAdaptiveDb::in_memory().unwrap();
        db.record_commit(&["行魚", "輸入法"]);
        db.record_commit(&["行魚", "好用"]);

        let snap = db.get_entries_snapshot(50);
        assert_eq!(snap.unigrams.len(), 3);
        assert_eq!(snap.bigrams.len(), 2);
        assert_eq!(snap.unigrams.iter().find(|u| u.word == "行魚").map(|u| u.count), Some(2));

        // 刪除特定 bigram 不影響單詞記憶
        assert!(db.delete_bigram("行魚", "好用").unwrap());
        let snap_after_bigram = db.get_entries_snapshot(50);
        assert_eq!(snap_after_bigram.bigrams.len(), 1);
        assert_eq!(snap_after_bigram.unigrams.len(), 3);

        // 刪除單詞時一併清理關聯的 bigram
        assert!(db.delete_unigram("行魚").unwrap());
        let snap_after_unigram = db.get_entries_snapshot(50);
        assert_eq!(snap_after_unigram.bigrams.len(), 0);
        assert_eq!(snap_after_unigram.unigrams.len(), 2);
        assert!(snap_after_unigram.unigrams.iter().all(|u| u.word != "行魚"));
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

        // 向左移動游標逐字前進：結尾 -> 第2字("天") -> 第1字("今") -> 第0字("我")
        assert!(decoder.move_cursor_left());
        let snap1 = decoder.get_composition_snapshot(9);
        assert_eq!(snap1.cursor_utf16, 2); // "我今|天"
        assert_eq!(snap1.selection_len_utf16, 0);
        assert_eq!(snap1.target_start_utf16, 2);

        assert!(decoder.move_cursor_left());
        let snap2 = decoder.get_composition_snapshot(9);
        assert_eq!(snap2.cursor_utf16, 1); // "我|今天"
        assert_eq!(snap2.selection_len_utf16, 0);
        assert_eq!(snap2.target_start_utf16, 1);

        assert!(decoder.move_cursor_left());
        let snap3 = decoder.get_composition_snapshot(9);
        assert_eq!(snap3.cursor_utf16, 0); // "|我今天"
        assert_eq!(snap3.selection_len_utf16, 0);
        assert_eq!(snap3.target_start_utf16, 0);
        assert!(snap3.candidates.len() >= 2);

        // 重選第 2 候選字替換 "我"
        let chosen = snap3.candidates[1].text.clone();
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

    #[test]
    fn test_literal_bopomofo_symbols_on_first_page_and_reselection() {
        let mut dec = DualPathDecoder::new();
        dec.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 1. 注音文：輸入單一注音符號（如 ㄓ 5 、ㄧ u 、ㄨ j 、ㄚ 8 ）時，
        //    首選仍為正常漢字，但純注音符號本身應出現在第 1 頁前 9 名內（標記為「注」）
        for (keys, expected_char, expected_zy) in [
            ("5 ", "之", "ㄓ"),
            ("u ", "一", "ㄧ"),
            ("j ", "屋", "ㄨ"),
            ("8 ", "阿", "ㄚ"),
        ] {
            dec.clear();
            for ch in keys.chars() {
                dec.push_char(ch);
            }
            let snap = dec.get_composition_snapshot(20);
            assert_eq!(snap.preedit, expected_char);
            let page1_pos = snap
                .candidates
                .iter()
                .take(9)
                .position(|c| c.text == expected_zy && c.annotation.as_deref() == Some("注"));
            assert!(
                page1_pos.is_some(),
                "expected '{}' [注] on page 1 for '{}', got {:?}",
                expected_zy,
                keys,
                snap.candidates
                    .iter()
                    .take(9)
                    .map(|c| (&c.text, &c.annotation))
                    .collect::<Vec<_>>()
            );
        }

        // 2. 完整音節如 5j4 亦提供純注音候選 ㄓㄨˋ [注]，且可在緩衝區內重複改選不同候選字
        dec.clear();
        for ch in "5j4".chars() {
            dec.push_char(ch);
        }
        let snap1 = dec.get_composition_snapshot(30);
        let zhu4_idx = snap1
            .candidates
            .iter()
            .position(|c| c.text == "ㄓㄨˋ" && c.annotation.as_deref() == Some("注"))
            .expect("expected ㄓㄨˋ [注] candidate for 5j4");
        dec.apply_candidate_selection(zhu4_idx);
        assert_eq!(dec.get_composition_snapshot(10).preedit, "ㄓㄨˋ");

        // 在同一緩衝區內再次重選為「住」
        let snap2 = dec.get_composition_snapshot(30);
        let zhu_live_idx = snap2
            .candidates
            .iter()
            .position(|c| c.text == "住")
            .expect("住 should still be available for reselection");
        dec.apply_candidate_selection(zhu_live_idx);
        assert_eq!(dec.get_composition_snapshot(10).preedit, "住");
    }

    #[test]
    fn test_preedit_caret_backspace_forward_delete_and_mid_insertion() {
        let mut dec = DualPathDecoder::new();
        dec.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 輸入 "我今天"
        for ch in "ji3rupwu0".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "我今天");

        // 向左移一字到 "我今|天"，按 Backspace 應直接刪除整個已轉換的「今」字
        assert!(dec.move_cursor_left());
        assert_eq!(dec.get_composition_snapshot(9).cursor_utf16, 2);
        assert!(dec.pop_char().is_some());
        assert_eq!(dec.get_preedit_string(), "我天");
        assert_eq!(dec.get_composition_snapshot(9).cursor_utf16, 1);

        // 在 "我|天" 中間輸入注音 "1u4" -> "必" (含數字鍵 1 與 4)，若半音節時按 Backspace 應先刪未組完按鍵
        dec.push_char('1');
        dec.push_char('8');
        let mid_snap = dec.get_composition_snapshot(9);
        assert_eq!(mid_snap.preedit, "我18天");
        assert_eq!(mid_snap.cursor_utf16, 3);
        assert_eq!(mid_snap.selection_len_utf16, 0);
        assert_eq!(dec.pop_char(), Some('8')); // 刪掉 '8'，剩 "1"
        assert_eq!(dec.get_composition_snapshot(9).preedit, "我1天");
        dec.push_char('j');
        dec.push_char('4'); // 完成 "ㄅㄨˋ" -> "不"
        assert_eq!(dec.get_preedit_string(), "我不天");

        // 此時游標位於 "我不|天"，按 Forward Delete 應刪除游標右側的「天」字
        assert!(dec.delete_forward());
        assert_eq!(dec.get_preedit_string(), "我不");

        // 游標右移/左移檢查邊界
        assert!(!dec.move_cursor_right());
        assert!(dec.move_cursor_left());
        assert_eq!(dec.get_composition_snapshot(9).cursor_utf16, 1);
        assert!(dec.move_cursor_right());
        assert_eq!(dec.get_composition_snapshot(9).cursor_utf16, 2);
    }

    #[test]
    fn test_reading_isolated_learning_you4_and_yu4() {
        let mut dec = DualPathDecoder::new();
        dec.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 1. 模擬舊版未區分讀音的學習紀錄污染，並頻繁上屏「有」(ㄧㄡˇ) 與「與」(ㄩˇ)
        for _ in 0..10 {
            dec.user_db.record_commit(&["有"]);
            dec.user_db.record_commit(&["與"]);
            dec.user_db.record_commit_with_readings(None, &[("ㄧㄡˇ", "有")]);
            dec.user_db.record_commit_with_readings(None, &[("ㄩˇ", "與")]);
        }

        // 2. 輸入 u.4 (ㄧㄡˋ)：「有」絕不應因 ㄧㄡˇ 學習而出現於第一頁（或排第一），首選應為「又」
        dec.clear();
        for ch in "u.4".chars() {
            dec.push_char(ch);
        }
        let snap_you4 = dec.get_composition_snapshot(20);
        assert_eq!(snap_you4.preedit, "又");
        let page1_you4: Vec<&str> = snap_you4
            .candidates
            .iter()
            .take(9)
            .map(|c| c.text.as_str())
            .collect();
        assert!(
            !page1_you4.contains(&"有"),
            "u.4 page 1 should not contain '有', got {:?}",
            page1_you4
        );

        // 3. 輸入 m4 (ㄩˋ)：「與」絕不應因 ㄩˇ 學習而出現於第一頁（或排第一），首選應為「育」
        dec.clear();
        for ch in "m4".chars() {
            dec.push_char(ch);
        }
        let snap_yu4 = dec.get_composition_snapshot(20);
        assert_eq!(snap_yu4.preedit, "育");
        let page1_yu4: Vec<&str> = snap_yu4
            .candidates
            .iter()
            .take(9)
            .map(|c| c.text.as_str())
            .collect();
        assert!(
            !page1_yu4.contains(&"與"),
            "m4 page 1 should not contain '與', got {:?}",
            page1_yu4
        );

        // 4. 在 u.4 主動學習「右」後，只應影響 u.4，不應干擾 u.3 (ㄧㄡˇ)
        dec.clear();
        for ch in "u.4".chars() {
            dec.push_char(ch);
        }
        let right_idx = dec
            .get_composition_snapshot(20)
            .candidates
            .iter()
            .position(|c| c.text == "右")
            .expect("expected '右' in u.4");
        dec.apply_candidate_selection(right_idx);
        dec.commit_text("右");

        dec.clear();
        for ch in "u.4".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "右");

        dec.clear();
        for ch in "u.3".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "有");
    }

    #[test]
    fn test_phrase_preservation_and_taiwan_variants() {
        let mut dec = DualPathDecoder::new();
        dec.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 大量學習高頻單字，驗證不會反過來把多字詞拆成單字
        for (zy, w) in [
            ("ㄧ", "一"),
            ("ㄍㄜˋ", "個"),
            ("ㄉㄜ˙", "的"),
            ("ㄕˋ", "是"),
            ("ㄗㄞˋ", "在"),
            ("ㄆㄞˊ", "排"),
            ("ㄗㄠˇ", "早"),
        ] {
            for _ in 0..20 {
                dec.user_db.record_commit_with_readings(None, &[(zy, w)]);
            }
        }

        for (keys, expected) in [
            ("u ek4", "一個"),
            ("u6ek4", "一個"),
            ("1j4g4", "不是"),
            ("1j6g4", "不是"),
            ("5k4ek4", "這個"),
            ("5k4ek7", "這個"),
            ("2u4u ek4", "第一個"),
            ("q96y942u4u ek4", "排在第一個"),
            ("vu3yl3", "洗澡"),
            ("yl3y94", "早在"),
            ("y42j/4vm03y4", "自動選字"),
            ("z06wu35j/ jp6", "繁體中文"),
            ("t;6ru04h6", "常見詞"),
            ("y04hjp6fm ", "暫存區"),
            ("ao6ej0 vu ", "沒關係"),
            ("u/3qu0 ", "影片"),
            ("vu84y94", "下載"),
            ("gp6ak ", "什麼"),
            ("ji3ap ", "我們"),
            ("xu03gj ", "臉書"),
            ("g60 ", "食安"),
            ("wu6el4", "提告"),
            ("ru0454", "建置"),
            ("qu0 vu; ", "偏鄉"),
            ("ru, u.3", "街友"),
            ("dk4e; ", "課綱"),
            ("cjo4d0 ", "會勘"),
        ] {
            dec.clear();
            for ch in keys.chars() {
                dec.push_char(ch);
            }
            assert_eq!(
                dec.get_preedit_string(),
                expected,
                "failed decoding '{}' -> got '{}'",
                keys,
                dec.get_preedit_string()
            );
        }
    }

    #[test]
    fn test_up_arrow_alternatives_zhuyin_english_and_similar() {
        let mut dec = DualPathDecoder::new();
        dec.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 1. 輸入 "我今天"，游標左移一次停在「天」前面 ("我今|天")，模擬按 ↑ 叫出轉換選單
        for ch in "ji3rupwu0".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "我今天");
        assert!(dec.move_cursor_left());

        let alts = dec.get_alternatives(40);
        // 應包含注音 [注]、原始英文按鍵 [英]、整句英文 [全英]、以及異調 [調] (如 ㄊㄧㄢˊ 田/填)
        let zy_idx = alts
            .iter()
            .position(|c| c.text == "ㄊㄧㄢ" && c.annotation.as_deref() == Some("注"))
            .expect("expected 'ㄊㄧㄢ' [注] in alternatives");
        assert_eq!(alts[zy_idx].preview_start_utf16, Some(2));
        assert_eq!(alts[zy_idx].preview_len_utf16, Some(1));

        assert!(
            alts.iter().any(|c| c.text == "wu0" && c.annotation.as_deref() == Some("英")),
            "expected 'wu0' [英] in alternatives: {:?}",
            alts.iter().map(|c| (&c.text, &c.annotation)).collect::<Vec<_>>()
        );
        assert!(
            alts.iter().any(|c| c.text == "ji3rupwu0" && c.annotation.as_deref() == Some("全英")),
            "expected 'ji3rupwu0' [全英] in alternatives"
        );
        assert!(
            alts.iter().any(|c| c.annotation.as_deref() == Some("調")),
            "expected tone-variant [調] candidates in alternatives"
        );

        // 套用第 1 項「ㄊㄧㄢ」[注]，應僅替換該字為 "我今ㄊㄧㄢ"
        dec.apply_alternative_selection(zy_idx);
        assert_eq!(dec.get_preedit_string(), "我今ㄊㄧㄢ");

        // 2. 句中未按 Shift 混打英文：
        // (a) >= 3 字母的英文句尾 ("ji3good") 應自動維持為 "我good"，且按 ↑ 可切換大小寫 ("Good") 與詞庫補全
        dec.clear();
        for ch in "ji3good".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "我good");
        let mixed_alts = dec.get_alternatives(40);
        let good_title_idx = mixed_alts
            .iter()
            .position(|c| c.text == "Good" && c.annotation.as_deref() == Some("英"))
            .expect("expected 'Good' [英] in alternatives for 我good");
        assert_eq!(mixed_alts[good_title_idx].span_start, 1);
        assert_eq!(mixed_alts[good_title_idx].span_end, 3);
        dec.apply_alternative_selection(good_title_idx);
        assert_eq!(dec.get_preedit_string(), "我Good");

        // (b) 游標移至句中某字（「我|會」）時，透過 ↑ 可將該字轉回原始英文按鍵 "cjo4"
        dec.clear();
        for ch in "ji3cjo4".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "我會");
        assert!(dec.move_cursor_left());
        let hui_alts = dec.get_alternatives(40);
        let cjo4_idx = hui_alts
            .iter()
            .position(|c| c.text == "cjo4" && c.annotation.as_deref() == Some("英"))
            .expect("expected 'cjo4' [英] in alternatives for 我會");
        dec.apply_alternative_selection(cjo4_idx);
        assert_eq!(dec.get_preedit_string(), "我cjo4");

        // 送出 "我cjo4" 後不應把 ASCII "cjo4" 污染進讀音資料庫
        dec.commit_text("我cjo4");
        let entries = dec.user_db.get_entries_snapshot(50);
        assert!(!entries.unigrams.iter().any(|u| u.word == "cjo4"));

        // 3. 純英文緩衝區 "hello" 按 ↑ 也應提供大小寫與英文候補轉換
        dec.clear();
        for ch in "hello".chars() {
            dec.push_char(ch);
        }
        let eng_alts = dec.get_alternatives(20);
        assert!(eng_alts.iter().any(|c| c.text == "Hello"));
        assert!(eng_alts.iter().any(|c| c.text == "HELLO"));

        // 4. 捲舌/平舌與前後鼻音近音容錯 ([近音])：例如輸入 "gp " ("ㄕㄣ" 身) 應包含 ㄙㄣ (森) 與 ㄕㄥ (生/聲)
        dec.clear();
        for ch in "gp ".chars() {
            dec.push_char(ch);
        }
        let sound_alts = dec.get_alternatives(40);
        assert!(
            sound_alts.iter().any(|c| c.annotation.as_deref() == Some("近音")),
            "expected confused-sound [近音] candidate in {:?}",
            sound_alts.iter().map(|c| (&c.text, &c.annotation)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_backspace_deletes_whole_chinese_char_at_tail() {
        let mut dec = DualPathDecoder::new();
        dec.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 1. 「我好」在句尾按 Backspace 應直接整字刪掉「好」，不會變回「我cl」
        for ch in "ji3cl3".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "我好");
        assert!(dec.pop_char().is_some());
        assert_eq!(dec.get_preedit_string(), "我");
        assert!(dec.pop_char().is_some());
        assert_eq!(dec.get_preedit_string(), "");

        // 2. 無空白連續詞「我今天」按 Backspace 刪「天」後，「今」必須維持為中文而非退回「我rup」
        for ch in "ji3rupwu0".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "我今天");
        assert!(dec.pop_char().is_some());
        assert_eq!(dec.get_preedit_string(), "我今");
        assert!(dec.pop_char().is_some());
        assert_eq!(dec.get_preedit_string(), "我");
        assert!(dec.pop_char().is_some());
        assert_eq!(dec.get_preedit_string(), "");

        // 3. 若尾端是尚未組完的按鍵「我c」，Backspace 僅刪 'c' 保留「我」
        for ch in "ji3c".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "我c");
        assert_eq!(dec.pop_char(), Some('c'));
        assert_eq!(dec.get_preedit_string(), "我");
    }

    #[test]
    fn test_english_inline_ghost_and_tab_completion() {
        let mut dec = DualPathDecoder::new();
        dec.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 1. 輸入 "comp" 應產生行內半透明補字 "uter"，按 Tab (accept_english_ghost) 補完為 "computer"
        for ch in "comp".chars() {
            dec.push_char(ch);
        }
        let snap1 = dec.get_composition_snapshot(10);
        assert_eq!(snap1.preedit, "comp");
        assert_eq!(snap1.english_ghost, "uter");
        assert!(dec.accept_english_ghost());
        let snap1_done = dec.get_composition_snapshot(10);
        assert_eq!(snap1_done.preedit, "computer");
        assert_eq!(snap1_done.english_ghost, "");

        // 2. 輸入 "app" -> "apple"
        dec.clear();
        for ch in "app".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_composition_snapshot(10).english_ghost, "le");
        assert!(dec.accept_english_ghost());
        assert_eq!(dec.get_preedit_string(), "apple");

        // 3. 中英混打句尾英文："這是app" -> ghost "le" -> Tab 後變成 "這是apple"
        dec.clear();
        for ch in "5k4g4app".chars() {
            dec.push_char(ch);
        }
        let mixed_snap = dec.get_composition_snapshot(10);
        assert_eq!(mixed_snap.preedit, "這是app");
        assert_eq!(mixed_snap.english_ghost, "le");
        assert!(dec.accept_english_ghost());
        assert_eq!(dec.get_preedit_string(), "這是apple");

        // 4. 正在打中文單音節未標調時（如 "ji" -> 準備按 '3' 成「我」）不應誤顯示英文幽靈字
        dec.clear();
        for ch in "ji".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_composition_snapshot(10).english_ghost, "");
    }

    #[test]
    fn test_standalone_consonant_bopomofo_symbols_and_zhuyin_selection() {
        let mut dec = DualPathDecoder::new();
        dec.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 1. 單純聲母注音符號 + 空白鍵 (一聲) 必須可直接輸出注音文（如 c  -> ㄏ、1  -> ㄅ、c c  -> ㄏㄏ、ji3c  -> 我ㄏ）
        for (keys, expected) in [
            ("c ", "ㄏ"),
            ("1 ", "ㄅ"),
            ("2 ", "ㄉ"),
            ("c c ", "ㄏㄏ"),
            ("ji3c ", "我ㄏ"),
        ] {
            dec.clear();
            for ch in keys.chars() {
                dec.push_char(ch);
            }
            assert_eq!(
                dec.get_preedit_string(),
                expected,
                "failed standalone bopomofo '{}' -> '{}'",
                keys,
                dec.get_preedit_string()
            );
        }

        // 2. 輸入 "5j4" (住) 時，↓ 前 5 個候選應直接包含「ㄓㄨˋ [注]」，↑ 第 1 項應為「ㄓㄨˋ [注]」
        dec.clear();
        for ch in "5j4".chars() {
            dec.push_char(ch);
        }
        let snap = dec.get_composition_snapshot(20);
        let zy_pos_down = snap
            .candidates
            .iter()
            .position(|c| c.text == "ㄓㄨˋ" && c.annotation.as_deref() == Some("注"))
            .expect("expected ㄓㄨˋ [注] in down candidates");
        assert!(zy_pos_down < 5, "expected ㄓㄨˋ [注] in top 5, got index {}", zy_pos_down);

        let alts = dec.get_alternatives(20);
        assert_eq!(alts[0].text, "ㄓㄨˋ");
        assert_eq!(alts[0].annotation.as_deref(), Some("注"));

        // 3. 尚未按空白鍵時（只按 'c'）透過 ↑ 選「ㄏ」後再繼續輸入 "c "，應接續組成 "ㄏㄏ"
        dec.clear();
        dec.push_char('c');
        let c_alts = dec.get_alternatives(10);
        assert_eq!(c_alts[0].text, "ㄏ");
        dec.apply_alternative_selection(0);
        assert_eq!(dec.get_preedit_string(), "ㄏ");
        dec.push_char('c');
        dec.push_char(' ');
        assert_eq!(dec.get_preedit_string(), "ㄏㄏ");
    }

    #[test]
    fn test_punctuation_in_preedit_cursor_backspace_and_navigation() {
        let mut dec = DualPathDecoder::new();
        dec.user_db = UserAdaptiveDb::in_memory().unwrap();

        // 1. 組字中途輸入全形標點（如 '，'）：必須留在暫存區而不清空，並支援游標左移回到前面改字與刪除
        for ch in "ji3rupwu0，".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "我今天，");

        // 左移一次停在 "我今天|，"，再左移一次停在 "我今|天，" 進行重選與刪字
        assert!(dec.move_cursor_left());
        assert_eq!(dec.get_composition_snapshot(10).cursor_utf16, 3);
        assert!(dec.move_cursor_left());
        assert_eq!(dec.get_composition_snapshot(10).cursor_utf16, 2);

        // 在 "我今|天，" 按 Backspace 應刪掉「今」，保留「我|天，」
        assert!(dec.pop_char().is_some());
        assert_eq!(dec.get_preedit_string(), "我天，");

        // 移回末尾接續輸入「真好」，組成「我天，真好」
        while dec.move_cursor_right() {}
        for ch in "5p cl3".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "我天，真好");

        // 提交「我天，真好」：逗號前後各自學習，不應跨逗號產生「天 -> 真好」的假 bigram
        dec.commit_text("我天，真好");
        let entries = dec.user_db.get_entries_snapshot(50);
        assert!(entries.unigrams.iter().any(|u| u.word == "真好"));
        assert!(!entries
            .bigrams
            .iter()
            .any(|b| b.prev_word == "天" && b.next_word == "真好"));

        // 2. 句尾標點也可透過 ↑ / ↓ 取得半形與相似標點候選，且 Backspace 可直接刪除標點
        dec.clear();
        for ch in "ji3，".chars() {
            dec.push_char(ch);
        }
        assert_eq!(dec.get_preedit_string(), "我，");
        let alts = dec.get_alternatives(10);
        assert!(alts.iter().any(|c| c.text == ","));
        assert!(dec.pop_char().is_some());
        assert_eq!(dec.get_preedit_string(), "我");
    }
}
