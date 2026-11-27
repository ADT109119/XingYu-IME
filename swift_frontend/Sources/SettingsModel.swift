import Foundation
import SwiftUI
import Combine

public class SettingsModel: ObservableObject {
    @Published public var keyboardLayout: String {
        didSet { UserDefaults.standard.set(keyboardLayout, forKey: "XingYu_KeyboardLayout") }
    }
    @Published public var enableMixedTyping: Bool {
        didSet { UserDefaults.standard.set(enableMixedTyping, forKey: "XingYu_EnableMixedTyping") }
    }
    @Published public var shiftToEnglish: Bool {
        didSet { UserDefaults.standard.set(shiftToEnglish, forKey: "XingYu_ShiftDirectUppercase") }
    }
    @Published public var candidateCount: Int {
        didSet { UserDefaults.standard.set(candidateCount, forKey: "XingYu_CandidateCount") }
    }
    @Published public var windowOrientation: String {
        didSet { UserDefaults.standard.set(windowOrientation, forKey: "XingYu_WindowOrientation") }
    }

    // 自訂辭典狀態
    @Published public var customDictPath: String
    @Published public var customDictLineCount: Int = 0
    @Published public var newWord: String = ""
    @Published public var newKey: String = ""
    @Published public var dictStatusMessage: String?

    // 學習資料庫統計與明細
    @Published public var unigramCount: Int = 0
    @Published public var bigramCount: Int = 0
    @Published public var learnedUnigrams: [LearnedUnigramItem] = []
    @Published public var learnedBigrams: [LearnedBigramItem] = []

    private weak var engine: SmartEngine?

    public init(engine: SmartEngine?, customDictPath: String) {
        self.engine = engine
        self.customDictPath = customDictPath

        self.keyboardLayout = UserDefaults.standard.string(forKey: "XingYu_KeyboardLayout") ?? "dachen"
        self.enableMixedTyping = UserDefaults.standard.object(forKey: "XingYu_EnableMixedTyping") as? Bool ?? true
        self.shiftToEnglish = UserDefaults.standard.object(forKey: "XingYu_ShiftDirectUppercase") as? Bool ?? true
        self.candidateCount = UserDefaults.standard.object(forKey: "XingYu_CandidateCount") as? Int ?? 9
        self.windowOrientation = UserDefaults.standard.string(forKey: "XingYu_WindowOrientation") ?? "horizontal"

        refreshDictCount()
        refreshStats()
    }

    public func refreshDictCount() {
        guard let content = try? String(contentsOfFile: customDictPath, encoding: .utf8) else {
            customDictLineCount = 0
            return
        }
        let lines = content.components(separatedBy: .newlines).filter { line in
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            return !trimmed.isEmpty && !trimmed.starts(with: "#")
        }
        customDictLineCount = lines.count
    }

    public func refreshStats() {
        if let engine = engine {
            let stats = engine.getLearningStats()
            self.unigramCount = stats.unigrams
            self.bigramCount = stats.bigrams
            let entries = engine.getLearningEntries(limit: 500)
            self.learnedUnigrams = entries.unigrams
            self.learnedBigrams = entries.bigrams
        }
    }

    public func deleteUnigram(_ word: String) {
        if let engine = engine, engine.deleteLearnedUnigram(word: word) {
            refreshStats()
        }
    }

    public func deleteBigram(prevWord: String, nextWord: String) {
        if let engine = engine, engine.deleteLearnedBigram(prevWord: prevWord, nextWord: nextWord) {
            refreshStats()
        }
    }

    public func addCustomWord() {
        let word = newWord.trimmingCharacters(in: .whitespaces)
        guard !word.isEmpty else { return }

        let key = newKey.trimmingCharacters(in: .whitespaces).isEmpty ? word : newKey.trimmingCharacters(in: .whitespaces)
        let line = "\(word)\t\(key)\t5000\n"

        if let fileHandle = FileHandle(forWritingAtPath: customDictPath) {
            fileHandle.seekToEndOfFile()
            if let data = line.data(using: .utf8) {
                fileHandle.write(data)
            }
            try? fileHandle.close()
            dictStatusMessage = "成功新增詞條「\(word)」！"
            newWord = ""
            newKey = ""
            refreshDictCount()
            reloadCustomDict()
        } else {
            dictStatusMessage = "無法寫入辭典檔案"
        }
    }

    public func reloadCustomDict() {
        if let engine = engine {
            _ = engine.loadCustomDict(path: customDictPath)
            refreshDictCount()
        }
    }

    public func clearLearningDb() {
        if let engine = engine {
            _ = engine.clearLearningDb()
            refreshStats()
        }
    }
}
