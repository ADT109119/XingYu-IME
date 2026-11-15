#ifndef XINGYU_ENGINE_H
#define XINGYU_ENGINE_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void* SmartEngineHandle;

SmartEngineHandle smart_ime_new(void);
void smart_ime_free(SmartEngineHandle engine);

void smart_ime_push_char(SmartEngineHandle engine, uint32_t ch);
bool smart_ime_pop_char(SmartEngineHandle engine);
bool smart_ime_delete_forward(SmartEngineHandle engine);
void smart_ime_clear(SmartEngineHandle engine);
bool smart_ime_is_empty(SmartEngineHandle engine);

const char* smart_ime_get_preedit(SmartEngineHandle engine);
const char* smart_ime_get_candidates_json(SmartEngineHandle engine, size_t limit);
const char* smart_ime_get_composition_json(SmartEngineHandle engine, size_t limit);
void smart_ime_commit_text(SmartEngineHandle engine, const char* text);

bool smart_ime_move_cursor_left(SmartEngineHandle engine);
bool smart_ime_move_cursor_right(SmartEngineHandle engine);
void smart_ime_clear_cursor(SmartEngineHandle engine);
bool smart_ime_has_active_cursor(SmartEngineHandle engine);
bool smart_ime_select_candidate(SmartEngineHandle engine, size_t index);

bool smart_ime_load_custom_dict(SmartEngineHandle engine, const char* path);
bool smart_ime_init_user_db(SmartEngineHandle engine, const char* path);

void smart_ime_get_learning_stats(SmartEngineHandle engine, size_t* out_unigrams, size_t* out_bigrams);
const char* smart_ime_get_learning_entries_json(SmartEngineHandle engine, size_t limit);
bool smart_ime_delete_learned_unigram(SmartEngineHandle engine, const char* word);
bool smart_ime_delete_learned_bigram(SmartEngineHandle engine, const char* prev_word, const char* next_word);
bool smart_ime_clear_learning_db(SmartEngineHandle engine);

void smart_ime_free_string(char* s);

#ifdef __cplusplus
}
#endif

#endif // XINGYU_ENGINE_H
