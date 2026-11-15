use crate::engine::user_db::UserAdaptiveDb;
use crate::engine::DualPathDecoder;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn smart_ime_new() -> *mut DualPathDecoder {
    let engine = Box::new(DualPathDecoder::new());
    Box::into_raw(engine)
}

#[no_mangle]
pub extern "C" fn smart_ime_free(engine: *mut DualPathDecoder) {
    if !engine.is_null() {
        unsafe {
            drop(Box::from_raw(engine));
        }
    }
}

#[no_mangle]
pub extern "C" fn smart_ime_push_char(engine: *mut DualPathDecoder, ch: u32) {
    if let Some(ch) = char::from_u32(ch) {
        let engine = unsafe { &mut *engine };
        engine.push_char(ch);
    }
}

#[no_mangle]
pub extern "C" fn smart_ime_pop_char(engine: *mut DualPathDecoder) -> bool {
    let engine = unsafe { &mut *engine };
    engine.pop_char().is_some()
}

#[no_mangle]
pub extern "C" fn smart_ime_delete_forward(engine: *mut DualPathDecoder) -> bool {
    if engine.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    engine.delete_forward()
}

#[no_mangle]
pub extern "C" fn smart_ime_clear(engine: *mut DualPathDecoder) {
    let engine = unsafe { &mut *engine };
    engine.clear();
}

#[no_mangle]
pub extern "C" fn smart_ime_is_empty(engine: *mut DualPathDecoder) -> bool {
    let engine = unsafe { &*engine };
    engine.is_empty()
}

#[no_mangle]
pub extern "C" fn smart_ime_get_preedit(engine: *mut DualPathDecoder) -> *const c_char {
    let engine = unsafe { &*engine };
    let preedit = engine.get_preedit_string();
    CString::new(preedit).unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn smart_ime_get_candidates_json(
    engine: *mut DualPathDecoder,
    limit: usize,
) -> *const c_char {
    let engine = unsafe { &*engine };
    let candidates = engine.get_candidates(limit);
    let json = serde_json::to_string(&candidates).unwrap_or_else(|_| "[]".to_string());
    CString::new(json).unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn smart_ime_get_composition_json(
    engine: *mut DualPathDecoder,
    limit: usize,
) -> *const c_char {
    let engine = unsafe { &*engine };
    let snapshot = engine.get_composition_snapshot(limit);
    let json = serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string());
    CString::new(json).unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn smart_ime_move_cursor_left(engine: *mut DualPathDecoder) -> bool {
    if engine.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    engine.move_cursor_left()
}

#[no_mangle]
pub extern "C" fn smart_ime_move_cursor_right(engine: *mut DualPathDecoder) -> bool {
    if engine.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    engine.move_cursor_right()
}

#[no_mangle]
pub extern "C" fn smart_ime_clear_cursor(engine: *mut DualPathDecoder) {
    if engine.is_null() {
        return;
    }
    let engine = unsafe { &mut *engine };
    engine.clear_cursor();
}

#[no_mangle]
pub extern "C" fn smart_ime_has_active_cursor(engine: *mut DualPathDecoder) -> bool {
    if engine.is_null() {
        return false;
    }
    let engine = unsafe { &*engine };
    engine.has_active_cursor()
}

#[no_mangle]
pub extern "C" fn smart_ime_select_candidate(engine: *mut DualPathDecoder, index: usize) -> bool {
    if engine.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    engine.apply_candidate_selection(index).is_some()
}

#[no_mangle]
pub extern "C" fn smart_ime_commit_text(engine: *mut DualPathDecoder, text: *const c_char) {
    if text.is_null() {
        return;
    }
    let engine = unsafe { &mut *engine };
    if let Ok(s) = unsafe { CStr::from_ptr(text) }.to_str() {
        engine.commit_text(s);
    }
}

#[no_mangle]
pub extern "C" fn smart_ime_load_custom_dict(
    engine: *mut DualPathDecoder,
    path: *const c_char,
) -> bool {
    if path.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    if let Ok(p) = unsafe { CStr::from_ptr(path) }.to_str() {
        engine.custom_dict.load_from_file(p).is_ok()
    } else {
        false
    }
}

#[no_mangle]
pub extern "C" fn smart_ime_init_user_db(
    engine: *mut DualPathDecoder,
    path: *const c_char,
) -> bool {
    if path.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    if let Ok(p) = unsafe { CStr::from_ptr(path) }.to_str() {
        if let Ok(db) = UserAdaptiveDb::open(p) {
            engine.user_db = db;
            return true;
        }
    }
    false
}

#[no_mangle]
pub extern "C" fn smart_ime_get_learning_stats(
    engine: *mut DualPathDecoder,
    out_unigrams: *mut usize,
    out_bigrams: *mut usize,
) {
    if engine.is_null() {
        return;
    }
    let engine = unsafe { &*engine };
    let (u, b) = engine.user_db.get_stats();
    if !out_unigrams.is_null() {
        unsafe { *out_unigrams = u; }
    }
    if !out_bigrams.is_null() {
        unsafe { *out_bigrams = b; }
    }
}

#[no_mangle]
pub extern "C" fn smart_ime_get_learning_entries_json(
    engine: *mut DualPathDecoder,
    limit: usize,
) -> *const c_char {
    if engine.is_null() {
        return CString::new("{\"unigrams\":[],\"bigrams\":[]}")
            .unwrap_or_default()
            .into_raw();
    }
    let engine = unsafe { &*engine };
    let snapshot = engine.user_db.get_entries_snapshot(limit);
    let json = serde_json::to_string(&snapshot)
        .unwrap_or_else(|_| "{\"unigrams\":[],\"bigrams\":[]}".to_string());
    CString::new(json).unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn smart_ime_delete_learned_unigram(
    engine: *mut DualPathDecoder,
    word: *const c_char,
) -> bool {
    if engine.is_null() || word.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    let Ok(word_str) = unsafe { CStr::from_ptr(word) }.to_str() else {
        return false;
    };
    engine.user_db.delete_unigram(word_str).unwrap_or(false)
}

#[no_mangle]
pub extern "C" fn smart_ime_delete_learned_bigram(
    engine: *mut DualPathDecoder,
    prev_word: *const c_char,
    next_word: *const c_char,
) -> bool {
    if engine.is_null() || prev_word.is_null() || next_word.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    let (Ok(prev_str), Ok(next_str)) = (
        unsafe { CStr::from_ptr(prev_word) }.to_str(),
        unsafe { CStr::from_ptr(next_word) }.to_str(),
    ) else {
        return false;
    };
    engine
        .user_db
        .delete_bigram(prev_str, next_str)
        .unwrap_or(false)
}

#[no_mangle]
pub extern "C" fn smart_ime_clear_learning_db(engine: *mut DualPathDecoder) -> bool {
    if engine.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    engine.user_db.clear_all().is_ok()
}

#[no_mangle]
pub extern "C" fn smart_ime_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}
