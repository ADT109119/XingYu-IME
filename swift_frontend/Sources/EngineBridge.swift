import Foundation

public struct CandidateItem: Codable, Identifiable {
    public var id: String { "\(text)\u{0000}\(span_start ?? 0)\u{0000}\(span_end ?? 0)" }
    public let text: String
    public let score: Double
    public let candidate_type: String
    public let annotation: String?
    public let span_start: Int?
    public let span_end: Int?
    public let preview_start_utf16: Int?
    public let preview_len_utf16: Int?
}

public struct LearnedUnigramItem: Codable, Identifiable, Equatable {
    public var id: String { word }
    public let word: String
    public let count: Int
    public let last_used: Int64
}

public struct LearnedBigramItem: Codable, Identifiable, Equatable {
    public var id: String { "\(prev_word)\u{0000}\(next_word)" }
    public let prev_word: String
    public let next_word: String
    public let count: Int
    public let last_used: Int64
}

public struct LearningEntriesSnapshot: Codable {
    public let unigrams: [LearnedUnigramItem]
    public let bigrams: [LearnedBigramItem]
}

public struct CompositionSnapshot: Codable {
    public let preedit: String
    public let cursor_utf16: Int
    public let selection_len_utf16: Int
    public let target_start_utf16: Int
    public let target_len_utf16: Int
    public let is_valid_zhuyin: Bool
    public let has_uppercase: Bool
    public let has_resolved_chinese: Bool
    public let should_space_act_as_tone: Bool
    public let segment_count: Int
    public let syllable_count: Int?
    public let english_ghost: String?
    public let candidates: [CandidateItem]
}

public class SmartEngine {
    private var handle: SmartEngineHandle?

    public init(userDbPath: String? = nil, customDictPath: String? = nil) {
        self.handle = smart_ime_new()

        if let dbPath = userDbPath {
            smart_ime_init_user_db(handle, dbPath)
        }
        if let dictPath = customDictPath {
            smart_ime_load_custom_dict(handle, dictPath)
        }
    }

    deinit {
        if let handle = handle {
            smart_ime_free(handle)
        }
    }

    public func push(char: Character) {
        guard let handle = handle, let scalar = char.unicodeScalars.first else { return }
        smart_ime_push_char(handle, scalar.value)
    }

    @discardableResult
    public func pop() -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_pop_char(handle)
    }

    @discardableResult
    public func deleteForward() -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_delete_forward(handle)
    }

    public func clear() {
        guard let handle = handle else { return }
        smart_ime_clear(handle)
    }

    public var isEmpty: Bool {
        guard let handle = handle else { return true }
        return smart_ime_is_empty(handle)
    }

    public func getPreedit() -> String {
        guard let handle = handle else { return "" }
        guard let cStr = smart_ime_get_preedit(handle) else { return "" }
        let result = String(cString: cStr)
        smart_ime_free_string(UnsafeMutablePointer(mutating: cStr))
        return result
    }

    public func getCandidates(limit: Int = 9) -> [CandidateItem] {
        guard let handle = handle else { return [] }
        guard let cStr = smart_ime_get_candidates_json(handle, limit) else { return [] }
        let jsonString = String(cString: cStr)
        smart_ime_free_string(UnsafeMutablePointer(mutating: cStr))

        guard let data = jsonString.data(using: .utf8),
              let candidates = try? JSONDecoder().decode([CandidateItem].self, from: data) else {
            return []
        }
        return candidates
    }

    public func getCompositionSnapshot(limit: Int = 200) -> CompositionSnapshot? {
        guard let handle = handle else { return nil }
        guard let cStr = smart_ime_get_composition_json(handle, limit) else { return nil }
        let jsonString = String(cString: cStr)
        smart_ime_free_string(UnsafeMutablePointer(mutating: cStr))

        guard let data = jsonString.data(using: .utf8),
              let snap = try? JSONDecoder().decode(CompositionSnapshot.self, from: data) else {
            return nil
        }
        return snap
    }

    @discardableResult
    public func moveCursorLeft() -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_move_cursor_left(handle)
    }

    @discardableResult
    public func moveCursorRight() -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_move_cursor_right(handle)
    }

    public func clearCursor() {
        guard let handle = handle else { return }
        smart_ime_clear_cursor(handle)
    }

    public var hasActiveCursor: Bool {
        guard let handle = handle else { return false }
        return smart_ime_has_active_cursor(handle)
    }

    @discardableResult
    public func selectCandidate(at index: Int) -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_select_candidate(handle, index)
    }

    public func getAlternatives(limit: Int = 50) -> [CandidateItem] {
        guard let handle = handle else { return [] }
        guard let cStr = smart_ime_get_alternatives_json(handle, limit) else { return [] }
        let jsonString = String(cString: cStr)
        smart_ime_free_string(UnsafeMutablePointer(mutating: cStr))

        guard let data = jsonString.data(using: .utf8),
              let alts = try? JSONDecoder().decode([CandidateItem].self, from: data) else {
            return []
        }
        return alts
    }

    @discardableResult
    public func selectAlternative(at index: Int) -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_select_alternative(handle, index)
    }

    @discardableResult
    public func acceptEnglishGhost() -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_accept_english_ghost(handle)
    }

    public func commit(text: String) {
        guard let handle = handle else { return }
        smart_ime_commit_text(handle, text)
    }

    public func loadCustomDict(path: String) -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_load_custom_dict(handle, path)
    }

    public func getLearningStats() -> (unigrams: Int, bigrams: Int) {
        guard let handle = handle else { return (0, 0) }
        var u: Int = 0
        var b: Int = 0
        smart_ime_get_learning_stats(handle, &u, &b)
        return (u, b)
    }

    public func getLearningEntries(limit: Int = 500) -> LearningEntriesSnapshot {
        guard let handle = handle else { return LearningEntriesSnapshot(unigrams: [], bigrams: []) }
        guard let cStr = smart_ime_get_learning_entries_json(handle, limit) else {
            return LearningEntriesSnapshot(unigrams: [], bigrams: [])
        }
        let jsonString = String(cString: cStr)
        smart_ime_free_string(UnsafeMutablePointer(mutating: cStr))

        guard let data = jsonString.data(using: .utf8),
              let snapshot = try? JSONDecoder().decode(LearningEntriesSnapshot.self, from: data) else {
            return LearningEntriesSnapshot(unigrams: [], bigrams: [])
        }
        return snapshot
    }

    @discardableResult
    public func deleteLearnedUnigram(word: String) -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_delete_learned_unigram(handle, word)
    }

    @discardableResult
    public func deleteLearnedBigram(prevWord: String, nextWord: String) -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_delete_learned_bigram(handle, prevWord, nextWord)
    }

    public func clearLearningDb() -> Bool {
        guard let handle = handle else { return false }
        return smart_ime_clear_learning_db(handle)
    }
}
