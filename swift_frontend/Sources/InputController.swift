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

    private enum CandidateSelectionMode {
        case none
        case candidates    // ↓ 向下展開一般選字 / 選詞選單
        case alternatives  // ↑ 向上展開華碩式快速轉換選單（注音 / 英文 / 調與近音 / 相似詞）
    }

    // 是否正處於方向鍵 / Tab 選字或轉換模式
    private var selectionMode: CandidateSelectionMode = .none
    private var isSelectingCandidate: Bool { selectionMode != .none }

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
        if selectionMode == .none {
            selectionMode = .candidates
        }
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
        // 若目前正在組字暫存區中，將標點符號保留並加入暫存區（不清空緩衝），方便用方向鍵 ← / → 回到標點前面修改或重選中文字
        if let punct = getFullwidthPunctuation(event: event) {
            if !engine.isEmpty {
                if isSelectingCandidate {
                    let applyOk = (selectionMode == .alternatives)
                        ? engine.selectAlternative(at: selectedIndex)
                        : engine.selectCandidate(at: selectedIndex)
                    if !applyOk {
                        commitCurrentBuffer(client: client, bySpace: false)
                    }
                    selectionMode = .none
                    selectedIndex = 0
                }
                for ch in punct {
                    engine.push(char: ch)
                }
                updateUI(client: client, hideCandidates: true)
                return true
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
                selectionMode = .none
                updateUI(client: client)
                return true
            }
            // 緩衝區為空時直接放行給系統執行標準退格刪除，避免干擾正常文字編輯！
            return false

        case 117: // Forward Delete (Fn + Backspace)
            if !engine.isEmpty {
                engine.deleteForward()
                selectedIndex = 0
                selectionMode = .none
                updateUI(client: client)
                return true
            }
            return false

        case 53: // Escape
            if !engine.isEmpty {
                if isSelectingCandidate {
                    selectionMode = .none
                    selectedIndex = 0
                    updateUI(client: client)
                } else if engine.hasActiveCursor {
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
                if isSelectingCandidate {
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

        case 123: // Left Arrow: 一般狀態在暫存區中字間左移游標（可搭配 Backspace 刪字或插入）；水平選單模式則左移選項
            if !engine.isEmpty {
                if isSelectingCandidate && currentWindowOrientation == "horizontal" && !currentCandidates.isEmpty {
                    selectedIndex = max(0, selectedIndex - 1)
                    showCandidates(client: client)
                    previewSelectedCandidate(client: client)
                } else {
                    engine.moveCursorLeft()
                    selectionMode = .none
                    selectedIndex = 0
                    updateUI(client: client)
                }
                return true
            }
            return false

        case 124: // Right Arrow: 一般狀態在暫存區中字間右移游標；水平選單模式則右移選項
            if !engine.isEmpty {
                if isSelectingCandidate && currentWindowOrientation == "horizontal" && !currentCandidates.isEmpty {
                    selectedIndex = min(currentCandidates.count - 1, selectedIndex + 1)
                    showCandidates(client: client)
                    previewSelectedCandidate(client: client)
                } else {
                    engine.moveCursorRight()
                    selectionMode = .none
                    selectedIndex = 0
                    updateUI(client: client)
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

        case 48: // Tab: 若有行內英文半透明預選補字則直接補入；否則進入選字模式並循環切換候選詞
            if !engine.isEmpty {
                if selectionMode == .none,
                   let ghost = lastSnapshot?.english_ghost,
                   !ghost.isEmpty,
                   engine.acceptEnglishGhost() {
                    selectedIndex = 0
                    selectionMode = .none
                    updateUI(client: client, hideCandidates: true)
                    return true
                }
                if !currentCandidates.isEmpty {
                    if selectionMode == .none {
                        selectionMode = .candidates
                        selectedIndex = currentCandidates.count > 1 ? 1 : 0
                    } else {
                        selectedIndex = (selectedIndex + 1) % currentCandidates.count
                    }
                    showCandidates(client: client)
                    previewSelectedCandidate(client: client)
                    return true
                }
            }

        case 126: // Up Arrow: 華碩智慧輸入法式快速轉換選單（注音 / 英文 / 同音異調 / 近音與相似詞）
            if !engine.isEmpty {
                switch selectionMode {
                case .none:
                    let alts = engine.getAlternatives(limit: 50)
                    if !alts.isEmpty {
                        selectionMode = .alternatives
                        currentCandidates = alts
                        selectedIndex = 0
                        showCandidates(client: client)
                        previewSelectedCandidate(client: client)
                        return true
                    } else if !currentCandidates.isEmpty {
                        selectionMode = .candidates
                        selectedIndex = 0
                        showCandidates(client: client)
                        previewSelectedCandidate(client: client)
                        return true
                    }
                case .alternatives:
                    if !currentCandidates.isEmpty {
                        if currentWindowOrientation == "horizontal" {
                            selectedIndex = (selectedIndex + 1) % currentCandidates.count
                        } else {
                            selectedIndex = selectedIndex > 0 ? selectedIndex - 1 : currentCandidates.count - 1
                        }
                        showCandidates(client: client)
                        previewSelectedCandidate(client: client)
                        return true
                    }
                case .candidates:
                    if !currentCandidates.isEmpty {
                        if selectedIndex > 0 {
                            selectedIndex -= 1
                            showCandidates(client: client)
                            previewSelectedCandidate(client: client)
                            return true
                        } else if currentWindowOrientation == "horizontal" {
                            let alts = engine.getAlternatives(limit: 50)
                            if !alts.isEmpty {
                                selectionMode = .alternatives
                                currentCandidates = alts
                                selectedIndex = 0
                                showCandidates(client: client)
                                previewSelectedCandidate(client: client)
                                return true
                            }
                        }
                    }
                }
            }

        case 125: // Down Arrow: 專門開啟一般選字 / 選詞 / 英文補全選單，或往下選擇候選字
            if !engine.isEmpty {
                if selectionMode == .none {
                    if let snap = lastSnapshot ?? engine.getCompositionSnapshot(limit: 200) {
                        lastSnapshot = snap
                        currentCandidates = snap.candidates
                    }
                    guard !currentCandidates.isEmpty else { return false }
                    selectionMode = .candidates
                    selectedIndex = currentCandidates.count > 1 ? 1 : 0
                } else if !currentCandidates.isEmpty {
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

        // C. 主鍵盤數字鍵 (1-9): 僅當處於選字模式時，直接按數字鍵完成當頁選詞/重選（避免移動游標編輯時攔截注音鍵 1,2,5,8,9）
        if !event.modifierFlags.contains(.shift) && isSelectingCandidate && firstChar.isNumber && firstChar != "0" {
            if let num = Int(String(firstChar)), num >= 1 && num <= pageSize {
                let targetIndex = pageStart + num - 1
                if targetIndex < currentCandidates.count {
                    applySelectionOrCommit(at: targetIndex, client: client, bySpace: false)
                    return true
                }
            }
        }

        // D. 按住 Shift + 英文字母：直接輸出純英文大寫，不開啟暫存區也不觸發任何選字
        let directUppercaseEnabled = UserDefaults.standard.object(forKey: "XingYu_ShiftDirectUppercase") as? Bool ?? true
        if directUppercaseEnabled && event.modifierFlags.contains(.shift) && firstChar.isASCII && firstChar.isLetter {
            if !engine.isEmpty {
                commitCurrentBuffer(client: client, bySpace: false)
            }
            client.insertText(String(firstChar) as NSString, replacementRange: NSRange(location: NSNotFound, length: 0))
            candidatePanel.hide()
            selectionMode = .none
            selectedIndex = 0
            return true
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
            // 5. 當「直接輸出大寫」被使用者關閉時，保留舊版大寫自動切英文緩衝行為
            if !directUppercaseEnabled && firstChar.isASCII && firstChar.isUppercase {
                if let snap = lastSnapshot, snap.has_resolved_chinese && !snap.has_uppercase {
                    commitCurrentBuffer(client: client, bySpace: false)
                }
            }
        }

        // 6. 一般字元推入（支援連續整句注音與 Shift 大寫英文）
        engine.push(char: firstChar)
        selectionMode = .none
        selectedIndex = 0
        updateUI(client: client)
        return true
    }

    private func previewSelectedCandidate(client: IMKTextInput) {
        guard selectedIndex >= 0 && selectedIndex < currentCandidates.count else { return }
        let chosen = currentCandidates[selectedIndex]
        let preview = chosen.text
        if let snap = lastSnapshot {
            let pStart = chosen.preview_start_utf16 ?? snap.target_start_utf16
            let pLen = chosen.preview_len_utf16 ?? snap.target_len_utf16
            if pLen > 0 {
                let fullNs = snap.preedit as NSString
                let replaceRange = NSRange(location: pStart, length: pLen)
                if replaceRange.location + replaceRange.length <= fullNs.length {
                    let updated = fullNs.replacingCharacters(in: replaceRange, with: preview)
                    let newSel = NSRange(location: pStart, length: preview.utf16.count)
                    client.setMarkedText(updated as NSString, selectionRange: newSel, replacementRange: NSRange(location: NSNotFound, length: 0))
                    return
                }
            } else if snap.segment_count > 1 {
                // 若未指定範圍但有多段詞組，替換最後一個區段預覽
                if let cand = snap.candidates.first, snap.preedit.hasSuffix(cand.text) {
                    let prefixLen = snap.preedit.utf16.count - cand.text.utf16.count
                    let prefix = (snap.preedit as NSString).substring(to: prefixLen)
                    let updated = prefix + preview
                    client.setMarkedText(updated as NSString, selectionRange: NSRange(location: updated.utf16.count, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
                    return
                }
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
            selectionMode = .none
            return
        }

        guard let snap = engine.getCompositionSnapshot(limit: 200) else { return }
        lastSnapshot = snap
        currentCandidates = snap.candidates
        selectedIndex = 0
        if !keepSelectingIfActiveCursor || !engine.hasActiveCursor {
            selectionMode = .none
        }

        let selRange = NSRange(location: snap.cursor_utf16, length: snap.selection_len_utf16)
        let ghost = snap.english_ghost ?? ""
        if !ghost.isEmpty {
            let composed = NSMutableAttributedString(
                string: snap.preedit,
                attributes: [
                    .underlineStyle: NSUnderlineStyle.single.rawValue
                ]
            )
            let ghostAttr = NSAttributedString(
                string: ghost,
                attributes: [
                    // 採用具象 sRGB 中性灰 (#9999A1)，確保跨行程 IMK 傳遞並在全黑與淺色背景上皆清晰可辨
                    .foregroundColor: NSColor(srgbRed: 0.60, green: 0.60, blue: 0.63, alpha: 1.0)
                ]
            )
            composed.append(ghostAttr)
            client.setMarkedText(composed, selectionRange: selRange, replacementRange: NSRange(location: NSNotFound, length: 0))
        } else {
            client.setMarkedText(snap.preedit as NSString, selectionRange: selRange, replacementRange: NSRange(location: NSNotFound, length: 0))
        }

        let isQuietEnglishGhost = !ghost.isEmpty && selectionMode == .none
        if (hideCandidates || isQuietEnglishGhost) && !engine.hasActiveCursor {
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
        let modeLabel = (selectionMode == .alternatives) ? "注音 / 英文 / 相似轉換" : nil
        candidatePanel.update(
            candidates: currentCandidates,
            selectedIndex: selectedIndex,
            pageSize: currentPageSize,
            orientation: currentWindowOrientation,
            modeLabel: modeLabel,
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
        let wasAlternativeMode = (selectionMode == .alternatives)
        let totalSyllables = lastSnapshot?.syllable_count ?? 0
        let chosenSpanStart = chosen.span_start ?? 0
        let chosenSpanEnd = chosen.span_end ?? totalSyllables
        let hasSpan = chosenSpanEnd > chosenSpanStart
        let chosenEndsEarly = hasSpan && chosenSpanEnd < totalSyllables

        let applyToEngine: (Int) -> Bool = { idx in
            wasAlternativeMode ? self.engine.selectAlternative(at: idx) : self.engine.selectCandidate(at: idx)
        }

        // 僅在無 span 區段的純英文選單項目時直接上屏；若帶有 span 區段（如句中轉英文按鍵或單字），則留在暫存區套用
        if chosen.candidate_type == "English" && !hasSpan {
            commitCandidate(at: index, client: client, bySpace: bySpace)
            return
        }

        // 若選詞後仍有未涵蓋的後續音節（如從「第一」改選單字「地」，或於長句前段重選詞組），在句內套用並推進至下一區段繼續編輯
        if chosenEndsEarly {
            if applyToEngine(index) {
                selectedIndex = 0
                selectionMode = .none
                updateUI(client: client, keepSelectingIfActiveCursor: true)
                return
            }
        }

        // 已涵蓋至句尾：
        // - 按 Enter 確認時（commitOnComplete == true）：套用此次選詞並立即送出整句結果、結束組字。
        // - 透過數字鍵 / 點擊 / 空白鍵選字時（commitOnComplete == false）：套用選詞並保留於組字緩衝區，讓使用者仍可繼續打字或重選。
        if commitOnComplete {
            let segCount = lastSnapshot?.segment_count ?? 0
            if segCount > 1 || engine.hasActiveCursor || hasSpan {
                if applyToEngine(index),
                   let updatedSnap = engine.getCompositionSnapshot(limit: 1),
                   !updatedSnap.preedit.isEmpty {
                    commitText(updatedSnap.preedit, learningWord: updatedSnap.preedit, client: client)
                    return
                }
            }
            commitCandidate(at: index, client: client, bySpace: bySpace)
            return
        }

        if applyToEngine(index) {
            selectedIndex = 0
            selectionMode = .none
            // 透過數字鍵 / 點擊 / 空白鍵選完最後一字後：先收合選字窗避免誤以為還要再選，同時保持底線組字狀態方便重選或一次 Enter 送出
            updateUI(client: client, keepSelectingIfActiveCursor: false, hideCandidates: true)
            return
        }

        commitCandidate(at: index, client: client, bySpace: bySpace)
    }

    private func commitCurrentBuffer(client: IMKTextInput, bySpace: Bool) {
        let snap = lastSnapshot ?? engine.getCompositionSnapshot(limit: 200)
        if let snap = snap {
            // 若為英文模式或首選詞為英文且尚未解出中文句子
            if !snap.has_resolved_chinese, let top = snap.candidates.first, top.candidate_type == "English" {
                let ghost = snap.english_ghost ?? ""
                // 若使用者未按 Tab 補入行內幽靈預測字即直接按 Space / Enter，保留實際輸入的英文而非強行替換成預測長詞
                let baseWord = (!ghost.isEmpty && top.text.caseInsensitiveCompare(snap.preedit) != .orderedSame)
                    ? snap.preedit
                    : top.text
                var textToInsert = baseWord
                if bySpace {
                    textToInsert += " "
                }
                commitText(textToInsert, learningWord: baseWord, client: client)
                return
            }
            // 若整句皆已解為中文詞句（或含混打結尾英文），直接送出整句組字結果；若以英文結尾且由空白鍵送出，保留空格
            if !snap.preedit.isEmpty {
                let appendSpace = bySpace && (snap.preedit.last?.isASCII ?? false)
                let outText = appendSpace ? (snap.preedit + " ") : snap.preedit
                commitText(outText, learningWord: snap.preedit, client: client)
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
        selectionMode = .none
    }
}
