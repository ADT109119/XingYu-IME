import Cocoa
import InputMethodKit

@objc(XingYuInputController)
public class XingYuInputController: IMKInputController {
    private var engine: SmartEngine
    private var candidatePanel: CandidatePanel
    private var currentCandidates: [CandidateItem] = []
    private var lastSnapshot: CompositionSnapshot?
    private var selectedIndex: Int = 0
    private var customDictPath: String

    // 是否正處於方向鍵 / Tab 選字模式
    private var isSelectingCandidate: Bool = false

    public override init!(server: IMKServer!, delegate: Any!, client inputClient: Any!) {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let imeDir = appSupport.appendingPathComponent("XingYuIME")
        try? FileManager.default.createDirectory(at: imeDir, withIntermediateDirectories: true)

        let dbPath = imeDir.appendingPathComponent("user_learning.db").path
        let dictPath = imeDir.appendingPathComponent("custom_dict.txt").path
        self.customDictPath = dictPath

        // 若自訂辭典尚未建立，建立預設示範檔
        if !FileManager.default.fileExists(atPath: dictPath) {
            let sampleContent = """
            # 行魚輸入法自訂辭典 (格式: 詞條\t注音或英文\t權重)
            行魚輸入法\tㄒㄧㄥˊ ㄩˊ ㄕㄨ ㄖㄨˋ ㄈㄚˇ\t9999
            深度學習\tㄕㄣ ㄉㄨˋ ㄒㄩㄝˊ ㄒㄧˊ\t5000
            人工智慧\tㄖㄣˊ ㄍㄨㄥ ㄓˋ ㄏㄨㄟˋ\t5000
            Jerry\tJerry\t5000
            Gemini\tGemini\t5000
            Swift\tSwift\t5000
            Rust\tRust\t5000
            """
            try? sampleContent.write(toFile: dictPath, atomically: true, encoding: .utf8)
        }

        self.engine = SmartEngine(userDbPath: dbPath, customDictPath: dictPath)
        self.candidatePanel = CandidatePanel()
        super.init(server: server, delegate: delegate, client: inputClient)
        NSLog("行魚輸入法 (XingYuIME) XingYuInputController 初始化成功")
    }

    // MARK: - 宣告接收的事件類型 (IMKStateSetting 核心必要方法)
    public override func recognizedEvents(_ sender: Any!) -> Int {
        let mask = NSEvent.EventTypeMask(arrayLiteral: .keyDown, .flagsChanged)
        return Int(mask.rawValue)
    }

    public override func activateServer(_ sender: Any!) {
        NSLog("行魚輸入法 (XingYuIME) activateServer")
        super.activateServer(sender)
    }

    public override func deactivateServer(_ sender: Any!) {
        NSLog("行魚輸入法 (XingYuIME) deactivateServer")
        if !engine.isEmpty {
            if let client = sender as? IMKTextInput {
                commitCurrentBuffer(client: client, bySpace: false)
            }
        }
        candidatePanel.hide()
        super.deactivateServer(sender)
    }

    public override func commitComposition(_ sender: Any!) {
        guard let client = sender as? IMKTextInput else { return }
        if !engine.isEmpty {
            commitCurrentBuffer(client: client, bySpace: false)
        }
    }

    // MARK: - macOS 狀態列選單整合
    public override func menu() -> NSMenu! {
        let menu = NSMenu(title: "行魚輸入法")

        let prefsItem = NSMenuItem(title: "偏好設定...", action: #selector(openPreferences), keyEquivalent: ",")
        prefsItem.target = self
        menu.addItem(prefsItem)

        menu.addItem(NSMenuItem.separator())

        let aboutItem = NSMenuItem(title: "關於行魚輸入法", action: #selector(openAbout), keyEquivalent: "")
        aboutItem.target = self
        menu.addItem(aboutItem)

        return menu
    }

    @objc public func openPreferences() {
        DispatchQueue.main.async {
            SettingsWindowController.shared.show(engine: self.engine, customDictPath: self.customDictPath)
        }
    }

    @objc public func openAbout() {
        DispatchQueue.main.async {
            SettingsWindowController.shared.show(engine: self.engine, customDictPath: self.customDictPath, tab: .about)
        }
    }

    // MARK: - 候選分頁與介面設定輔助
    private var currentPageSize: Int {
        let configured = UserDefaults.standard.object(forKey: "XingYu_CandidateCount") as? Int ?? 9
        return max(1, min(9, configured))
    }

    private var currentWindowOrientation: String {
        UserDefaults.standard.string(forKey: "XingYu_WindowOrientation") ?? "horizontal"
    }

    private func stepCandidatePage(_ delta: Int, client: IMKTextInput) {
        guard !currentCandidates.isEmpty else { return }
        let pageSize = currentPageSize
        let currentPage = selectedIndex / pageSize
        let totalPages = (currentCandidates.count + pageSize - 1) / pageSize
        let nextPage = max(0, min(totalPages - 1, currentPage + delta))
        selectedIndex = nextPage * pageSize
        isSelectingCandidate = true
        showCandidates(client: client)
        previewSelectedCandidate(client: client)
    }

    // MARK: - 全形標點符號轉換邏輯
    private func getFullwidthPunctuation(event: NSEvent) -> String? {
        if event.modifierFlags.contains(.command) || event.modifierFlags.contains(.option) {
            return nil
        }

        let isShift = event.modifierFlags.contains(.shift)
        let isControl = event.modifierFlags.contains(.control)
        let keyCode = event.keyCode

        switch keyCode {
        case 43: // , / <
            return (isShift || isControl) ? "，" : nil
        case 47: // . / >
            return (isShift || isControl) ? "。" : nil
        case 44: // / / ?
            return (isShift || isControl) ? "？" : nil
        case 41: // ; / :
            return (isShift || isControl) ? "：" : nil
        case 39: // ' / " -> Ctrl+' 為中文全形頓號「、」，Shift+' 為「”」，單按為「’」
            if isControl {
                return "、"
            }
            return isShift ? "”" : "’"
        case 18: // 1 / !
            return (isShift || isControl) ? "！" : nil
        case 42: // \ / |
            return isShift ? "｜" : "、"
        case 33: // [ / {
            return isShift ? "『" : "「"
        case 30: // ] / }
            return isShift ? "』" : "」"
        case 25: // 9 / (
            return isShift ? "（" : nil
        case 29: // 0 / )
            return isShift ? "）" : nil
        case 27: // - / _
            return isShift ? "——" : nil
        case 50: // ` / ~
            return isShift ? "～" : "、"
        case 22: // 6 / ^
            return isShift ? "……" : nil
        case 21: // 4 / $
            return isShift ? "＄" : nil
        case 23: // 5 / %
            return isShift ? "％" : nil
        case 24: // = / +
            return isShift ? "＋" : "＝"
        case 20: // 3 / #
            return isShift ? "＃" : nil
        case 19: // 2 / @
            return isShift ? "＠" : nil
        case 28: // 8 / *
            return isShift ? "＊" : nil
        default:
            return nil
        }
    }

    // MARK: - 鍵盤按鍵事件分發
    public override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event = event, let client = sender as? IMKTextInput else { return false }

        if event.type != .keyDown {
            return false
        }

        // 快捷鍵 Command+, 直接開啟偏好設定
        if event.modifierFlags.contains(.command) && event.characters == "," {
            openPreferences()
            return true
        }

        // 放行 Command / Option 系統快捷鍵
        if event.modifierFlags.contains(.command) || event.modifierFlags.contains(.option) {
            return false
        }

        // 0. 選字模式下若有多頁候選詞，允許使用 [ / ] 快速翻頁（不與大千注音衝突）
        let hasCtrlOrShift = event.modifierFlags.contains(.control) || event.modifierFlags.contains(.shift)
        if !hasCtrlOrShift && isSelectingCandidate && currentCandidates.count > currentPageSize {
            if event.keyCode == 33 { // [ 上一頁
                stepCandidatePage(-1, client: client)
                return true
            }
            if event.keyCode == 30 { // ] 下一頁
                stepCandidatePage(1, client: client)
                return true
            }
        }

        // 1. 全形標點符號優先處理（含 Ctrl+' -> 「、」）
        if let punct = getFullwidthPunctuation(event: event) {
            if !engine.isEmpty {
                commitCurrentBuffer(client: client, bySpace: false)
            }
            client.insertText(punct as NSString, replacementRange: NSRange(location: NSNotFound, length: 0))
            return true
        }

        // 忽略其餘未處理的 Control 快捷鍵
        if event.modifierFlags.contains(.control) {
            return false
        }

        let chars = event.characters ?? ""
        guard let firstChar = chars.first else { return false }

        // 2. 特殊控制鍵處理（編輯、選詞、區段重選、送出）
        switch event.keyCode {
        case 51: // Backspace / Delete
            if !engine.isEmpty {
                engine.pop()
                selectedIndex = 0
                isSelectingCandidate = false
                updateUI(client: client)
                return true
            }
            // 緩衝區為空時直接放行給系統執行標準退格刪除，避免干擾正常文字編輯！
            return false

        case 53: // Escape
            if !engine.isEmpty {
                if isSelectingCandidate || engine.hasActiveCursor {
                    isSelectingCandidate = false
                    engine.clearCursor()
                    selectedIndex = 0
                    updateUI(client: client)
                } else {
                    engine.clear()
                    updateUI(client: client)
                }
                return true
            }
            return false

        case 36, 76: // Return / Enter (含數字鍵盤 Enter)
            if !engine.isEmpty {
                if isSelectingCandidate || engine.hasActiveCursor {
                    // 選字時按 Enter：若僅選取較短前段詞會推進至剩餘音節；一旦選完至句尾最後一字，直接確認並送出整句（無需再多按一次 Enter）
                    applySelectionOrCommit(at: selectedIndex, client: client, bySpace: false, commitOnComplete: true)
                } else {
                    commitCurrentBuffer(client: client, bySpace: false)
                }
                return true
            }
            return false

        case 49: // Space (空白鍵)
            if !engine.isEmpty {
                if isSelectingCandidate {
                    applySelectionOrCommit(at: selectedIndex, client: client, bySpace: true)
                    return true
                }
                if let snap = lastSnapshot, snap.should_space_act_as_tone {
                    // 未標調中文音節尾端：空白鍵作為「一聲（陰平）」，無縫組字並繼續連續輸入！
                    engine.push(char: " ")
                    selectedIndex = 0
                    updateUI(client: client)
                    return true
                }
                commitCurrentBuffer(client: client, bySpace: true)
                return true
            }
            return false

        case 123: // Left Arrow: 選字列橫向往前（單段瀏覽時），或向左移動至前一個詞彙區段進行重選
            if !engine.isEmpty {
                if isSelectingCandidate && !engine.hasActiveCursor && (lastSnapshot?.segment_count ?? 0) <= 1 && !currentCandidates.isEmpty {
                    selectedIndex = max(0, selectedIndex - 1)
                    showCandidates(client: client)
                    previewSelectedCandidate(client: client)
                } else if engine.moveCursorLeft() {
                    isSelectingCandidate = true
                    selectedIndex = 0
                    updateUI(client: client, keepSelectingIfActiveCursor: true)
                } else if !currentCandidates.isEmpty {
                    isSelectingCandidate = true
                    selectedIndex = max(0, selectedIndex - 1)
                    showCandidates(client: client)
                    previewSelectedCandidate(client: client)
                }
                return true
            }
            return false

        case 124: // Right Arrow: 向右移動詞彙區段（或下一個候選字）
            if !engine.isEmpty {
                if engine.hasActiveCursor {
                    engine.moveCursorRight()
                    selectedIndex = 0
                    updateUI(client: client, keepSelectingIfActiveCursor: true)
                    return true
                } else if isSelectingCandidate && !currentCandidates.isEmpty {
                    selectedIndex = min(currentCandidates.count - 1, selectedIndex + 1)
                    showCandidates(client: client)
                    previewSelectedCandidate(client: client)
                    return true
                }
                return true
            }
            return false

        case 116: // PageUp (Fn + Up): 上一頁候選字
            if !engine.isEmpty && !currentCandidates.isEmpty {
                stepCandidatePage(-1, client: client)
                return true
            }

        case 121: // PageDown (Fn + Down): 下一頁候選字
            if !engine.isEmpty && !currentCandidates.isEmpty {
                stepCandidatePage(1, client: client)
                return true
            }

        case 48: // Tab: 進入選字模式並循環切換候選詞
            if !engine.isEmpty && !currentCandidates.isEmpty {
                if !isSelectingCandidate {
                    isSelectingCandidate = true
                    selectedIndex = currentCandidates.count > 1 ? 1 : 0
                } else {
                    selectedIndex = (selectedIndex + 1) % currentCandidates.count
                }
                showCandidates(client: client)
                previewSelectedCandidate(client: client)
                return true
            }

        case 126: // Up Arrow: 上一個候選詞
            if !engine.isEmpty && !currentCandidates.isEmpty {
                isSelectingCandidate = true
                selectedIndex = max(0, selectedIndex - 1)
                showCandidates(client: client)
                previewSelectedCandidate(client: client)
                return true
            }

        case 125: // Down Arrow: 進入選字 / 下一個候選詞
            if !engine.isEmpty && !currentCandidates.isEmpty {
                if !isSelectingCandidate {
                    isSelectingCandidate = true
                    selectedIndex = currentCandidates.count > 1 ? 1 : 0
                } else {
                    selectedIndex = min(currentCandidates.count - 1, selectedIndex + 1)
                }
                showCandidates(client: client)
                previewSelectedCandidate(client: client)
                return true
            }

        default:
            break
        }

        // 3. 候選字數字快速選取（依目前分頁起始位置對應第 1~N 個候選詞）：
        let pageSize = currentPageSize
        let pageStart = (selectedIndex / pageSize) * pageSize

        // A. 數字小鍵盤 (Keypad 1-9)
        let keypadMap: [UInt16: Int] = [
            83: 1, 84: 2, 85: 3, 86: 4, 87: 5, 88: 6, 89: 7, 91: 8, 92: 9
        ]
        if let num = keypadMap[event.keyCode], num <= pageSize {
            let targetIndex = pageStart + num - 1
            if targetIndex < currentCandidates.count {
                applySelectionOrCommit(at: targetIndex, client: client, bySpace: false)
                return true
            }
        }

        // B. Shift + 數字鍵 (Shift + 1~9)
        let numberRowMap: [UInt16: Int] = [
            18: 1, 19: 2, 20: 3, 21: 4, 23: 5, 22: 6, 26: 7, 28: 8, 25: 9
        ]
        if event.modifierFlags.contains(.shift), let num = numberRowMap[event.keyCode], num <= pageSize {
            let targetIndex = pageStart + num - 1
            if targetIndex < currentCandidates.count {
                applySelectionOrCommit(at: targetIndex, client: client, bySpace: false)
                return true
            }
        }

        // C. 主鍵盤數字鍵 (1-9): 當處於選字模式或游標重選狀態時，直接按數字鍵完成當頁選詞/重選
        if !event.modifierFlags.contains(.shift) && (isSelectingCandidate || engine.hasActiveCursor) && firstChar.isNumber && firstChar != "0" {
            if let num = Int(String(firstChar)), num >= 1 && num <= pageSize {
                let targetIndex = pageStart + num - 1
                if targetIndex < currentCandidates.count {
                    applySelectionOrCommit(at: targetIndex, client: client, bySpace: false)
                    return true
                }
            }
        }

        // 4. 若當前輸入緩衝區為空，檢查首鍵合法性
        if engine.isEmpty {
            if !event.modifierFlags.contains(.shift) && ["3", "4", "6", "7"].contains(firstChar) {
                return false
            }

            let bopomofoPunctuation: Set<Character> = [",", ".", "/", ";", "-"]
            let isValidInitialKey = firstChar.isLetter || firstChar.isNumber || bopomofoPunctuation.contains(firstChar)
            if !isValidInitialKey {
                return false
            }
        } else {
            // 5. 若目前正在組中文句，突然按下 Shift + 大寫英文字母（想在中文後直接混打英文）：
            // 自動確認先前的中文組字，並無縫以該大寫字母開啟新英文緩衝區！
            let shiftToEnglishEnabled = UserDefaults.standard.object(forKey: "XingYu_ShiftToEnglish") as? Bool ?? true
            if shiftToEnglishEnabled && firstChar.isASCII && firstChar.isUppercase {
                if let snap = lastSnapshot, snap.has_resolved_chinese && !snap.has_uppercase {
                    commitCurrentBuffer(client: client, bySpace: false)
                }
            }
        }

        // 6. 一般字元推入（支援連續整句注音與 Shift 大寫英文）
        engine.push(char: firstChar)
        isSelectingCandidate = false
        selectedIndex = 0
        updateUI(client: client)
        return true
    }

    private func previewSelectedCandidate(client: IMKTextInput) {
        guard selectedIndex >= 0 && selectedIndex < currentCandidates.count else { return }
        let preview = currentCandidates[selectedIndex].text
        if let snap = lastSnapshot, snap.segment_count > 1 && snap.selection_len_utf16 > 0 {
            // 在長句中重選單一區段時，即時置換該區段於整句中的預覽！
            let fullNs = snap.preedit as NSString
            let replaceRange = NSRange(location: snap.cursor_utf16, length: snap.selection_len_utf16)
            if replaceRange.location + replaceRange.length <= fullNs.length {
                let updated = fullNs.replacingCharacters(in: replaceRange, with: preview)
                let newSel = NSRange(location: snap.cursor_utf16, length: preview.utf16.count)
                client.setMarkedText(updated as NSString, selectionRange: newSel, replacementRange: NSRange(location: NSNotFound, length: 0))
                return
            }
        } else if let snap = lastSnapshot, snap.segment_count > 1 {
            // 若未指定游標但有多段詞組，替換最後一個區段預覽
            if let cand = currentCandidates.first, snap.preedit.hasSuffix(cand.text) {
                let prefixLen = snap.preedit.utf16.count - cand.text.utf16.count
                let prefix = (snap.preedit as NSString).substring(to: prefixLen)
                let updated = prefix + preview
                client.setMarkedText(updated as NSString, selectionRange: NSRange(location: updated.utf16.count, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
                return
            }
        }
        client.setMarkedText(preview as NSString, selectionRange: NSRange(location: preview.utf16.count, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
    }

    private func updateUI(client: IMKTextInput, keepSelectingIfActiveCursor: Bool = false, hideCandidates: Bool = false) {
        if engine.isEmpty {
            client.setMarkedText("" as NSString, selectionRange: NSRange(location: 0, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
            candidatePanel.hide()
            currentCandidates.removeAll()
            lastSnapshot = nil
            selectedIndex = 0
            isSelectingCandidate = false
            return
        }

        guard let snap = engine.getCompositionSnapshot(limit: 200) else { return }
        lastSnapshot = snap
        currentCandidates = snap.candidates
        selectedIndex = 0
        if !keepSelectingIfActiveCursor || !engine.hasActiveCursor {
            isSelectingCandidate = false
        }

        let selRange = NSRange(location: snap.cursor_utf16, length: snap.selection_len_utf16)
        client.setMarkedText(snap.preedit as NSString, selectionRange: selRange, replacementRange: NSRange(location: NSNotFound, length: 0))
        if hideCandidates && !engine.hasActiveCursor {
            candidatePanel.hide()
        } else {
            showCandidates(client: client)
        }
    }

    private func showCandidates(client: IMKTextInput) {
        var cursorRect = NSRect.zero
        let targetIndex = lastSnapshot?.cursor_utf16 ?? 0
        client.attributes(forCharacterIndex: targetIndex, lineHeightRectangle: &cursorRect)
        if cursorRect.origin.x == 0 && cursorRect.origin.y == 0 && targetIndex > 0 {
            client.attributes(forCharacterIndex: 0, lineHeightRectangle: &cursorRect)
        }
        var point = NSPoint(x: cursorRect.origin.x, y: cursorRect.origin.y)
        if point.x == 0 && point.y == 0 {
            point = NSEvent.mouseLocation
        }
        candidatePanel.update(
            candidates: currentCandidates,
            selectedIndex: selectedIndex,
            pageSize: currentPageSize,
            orientation: currentWindowOrientation,
            at: point,
            onSelect: { [weak self] absoluteIndex in
                self?.applySelectionOrCommit(at: absoluteIndex, client: client, bySpace: false)
            },
            onPageStep: { [weak self] delta in
                self?.stepCandidatePage(delta, client: client)
            }
        )
    }

    private func applySelectionOrCommit(at index: Int, client: IMKTextInput, bySpace: Bool, commitOnComplete: Bool = false) {
        guard index >= 0 && index < currentCandidates.count else { return }
        let chosen = currentCandidates[index]
        let totalSyllables = lastSnapshot?.syllable_count ?? 0
        let chosenSpanEnd = chosen.span_end ?? totalSyllables
        let chosenEndsEarly = chosenSpanEnd < totalSyllables

        if chosen.candidate_type == "English" {
            commitCandidate(at: index, client: client, bySpace: bySpace)
            return
        }

        // 若選詞後仍有未涵蓋的後續音節（如從「第一」改選單字「地」，或於長句前段重選詞組），在句內套用並推進至下一區段繼續編輯
        if chosenEndsEarly {
            if engine.selectCandidate(at: index) {
                selectedIndex = 0
                updateUI(client: client, keepSelectingIfActiveCursor: true)
                return
            }
        }

        // 已涵蓋至句尾：
        // - 按 Enter 確認時（commitOnComplete == true）：套用此次選詞並立即送出整句結果、結束組字。
        // - 透過數字鍵 / 點擊 / 空白鍵選字時（commitOnComplete == false）：套用選詞並保留於組字緩衝區，讓使用者仍可繼續打字或重選。
        if commitOnComplete {
            let segCount = lastSnapshot?.segment_count ?? 0
            if segCount > 1 || engine.hasActiveCursor {
                if engine.selectCandidate(at: index),
                   let updatedSnap = engine.getCompositionSnapshot(limit: 1),
                   !updatedSnap.preedit.isEmpty {
                    commitText(updatedSnap.preedit, learningWord: updatedSnap.preedit, client: client)
                    return
                }
            }
            commitCandidate(at: index, client: client, bySpace: bySpace)
            return
        }

        if engine.selectCandidate(at: index) {
            selectedIndex = 0
            // 透過數字鍵 / 點擊 / 空白鍵選完最後一字後：先收合選字窗避免誤以為還要再選，同時保持底線組字狀態方便重選或一次 Enter 送出
            updateUI(client: client, keepSelectingIfActiveCursor: true, hideCandidates: !engine.hasActiveCursor)
            return
        }

        commitCandidate(at: index, client: client, bySpace: bySpace)
    }

    private func commitCurrentBuffer(client: IMKTextInput, bySpace: Bool) {
        let snap = lastSnapshot ?? engine.getCompositionSnapshot(limit: 200)
        if let snap = snap {
            // 若為英文模式或首選詞為英文且尚未解出中文句子
            if !snap.has_resolved_chinese, let top = snap.candidates.first, top.candidate_type == "English" {
                var textToInsert = top.text
                if bySpace {
                    textToInsert += " "
                }
                commitText(textToInsert, learningWord: top.text, client: client)
                return
            }
            // 若整句皆已解為中文詞句，直接送出整句組字結果
            if !snap.preedit.isEmpty {
                commitText(snap.preedit, learningWord: snap.preedit, client: client)
                return
            }
        }

        let preedit = engine.getPreedit()
        commitText(preedit, client: client)
    }

    private func commitCandidate(at index: Int, client: IMKTextInput, bySpace: Bool = false) {
        guard index >= 0 && index < currentCandidates.count else { return }
        let chosen = currentCandidates[index]

        var textToInsert = chosen.text
        if bySpace && chosen.candidate_type == "English" {
            textToInsert += " "
        }

        commitText(textToInsert, learningWord: chosen.text, client: client)
    }

    private func commitText(_ text: String, learningWord: String? = nil, client: IMKTextInput) {
        engine.commit(text: learningWord ?? text)
        client.insertText(text as NSString, replacementRange: NSRange(location: NSNotFound, length: 0))
        candidatePanel.hide()
        currentCandidates.removeAll()
        lastSnapshot = nil
        selectedIndex = 0
        isSelectingCandidate = false
    }
}
