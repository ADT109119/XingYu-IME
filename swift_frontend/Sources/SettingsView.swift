import SwiftUI
import AppKit

public enum SettingsTab: Hashable {
    case general
    case customDict
    case learning
    case about
}

public struct SettingsView: View {
    private enum LearningScope: String, CaseIterable, Identifiable {
        case unigrams = "單詞記憶"
        case bigrams = "詞彙搭配"
        var id: String { rawValue }
    }

    private enum LearningSortOrder: String, CaseIterable, Identifiable {
        case recent = "最近使用"
        case frequency = "最常使用"
        var id: String { rawValue }
    }

    @ObservedObject var settingsModel: SettingsModel
    @State private var selectedTab: SettingsTab
    @State private var learningScope: LearningScope = .unigrams
    @State private var learningSort: LearningSortOrder = .recent
    @State private var learningSearch: String = ""
    @State private var showClearConfirm: Bool = false

    public init(settingsModel: SettingsModel, initialTab: SettingsTab = .general) {
        self.settingsModel = settingsModel
        _selectedTab = State(initialValue: initialTab)
    }

    public var body: some View {
        TabView(selection: $selectedTab) {
            generalTab
                .tag(SettingsTab.general)
                .tabItem {
                    Label("一般設定", systemImage: "gearshape")
                }

            customDictTab
                .tag(SettingsTab.customDict)
                .tabItem {
                    Label("自訂辭典", systemImage: "character.book.closed")
                }

            learningTab
                .tag(SettingsTab.learning)
                .tabItem {
                    Label("詞袋學習", systemImage: "brain.head.profile")
                }

            aboutTab
                .tag(SettingsTab.about)
                .tabItem {
                    Label("關於行魚", systemImage: "info.circle")
                }
        }
        .frame(width: 580, height: 500)
        .padding()
    }

    // MARK: - 1. 一般設定
    private var generalTab: some View {
        Form {
            Section(header: Text("鍵盤配置與模式").font(.headline)) {
                Picker("注音鍵盤配置", selection: $settingsModel.keyboardLayout) {
                    Text("大千式標準注音 (預設)").tag("dachen")
                    Text("倚天 26/41 鍵 (規劃中)").tag("eten")
                    Text("漢語拼音 (規劃中)").tag("pinyin")
                }
                .pickerStyle(.menu)

                Toggle("啟用中英混打 (智慧判斷英文單字與 Tab 補字)", isOn: $settingsModel.enableMixedTyping)
                Toggle("按住 Shift + 英文直接輸出大寫 (不觸發選字)", isOn: $settingsModel.shiftToEnglish)
            }

            Section(header: Text("候選字顯示").font(.headline)) {
                Stepper("每頁候選字數量: \(settingsModel.candidateCount)", value: $settingsModel.candidateCount, in: 5...9)
                Picker("候選字窗方向", selection: $settingsModel.windowOrientation) {
                    Text("水平橫向排列").tag("horizontal")
                    Text("垂直直向排列").tag("vertical")
                }
                .pickerStyle(.segmented)
            }
        }
        .padding()
    }

    // MARK: - 2. 自訂辭典設定
    private var customDictTab: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("自訂辭典 (優先順序最高)")
                .font(.headline)

            Text("當輸入符合的注音或英文時，自訂辭典中的詞條將無條件排在候選第 1 位。")
                .font(.caption)
                .foregroundColor(.secondary)

            HStack {
                Text(settingsModel.customDictPath)
                    .font(.system(.caption, design: .monospaced))
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .padding(6)
                    .background(Color.secondary.opacity(0.1))
                    .cornerRadius(6)

                Button("在訪達中顯示") {
                    let url = URL(fileURLWithPath: settingsModel.customDictPath)
                    NSWorkspace.shared.activateFileViewerSelecting([url])
                }

                Button("文字編輯器開啟") {
                    let url = URL(fileURLWithPath: settingsModel.customDictPath)
                    NSWorkspace.shared.open(url)
                }
            }

            Divider()

            Text("快速新增詞條")
                .font(.subheadline).bold()

            HStack(spacing: 8) {
                TextField("詞條 (如: 人工智慧)", text: $settingsModel.newWord)
                    .textFieldStyle(.roundedBorder)
                TextField("注音/英文 (如: ㄖㄣˊ ㄍㄨㄥ ㄓˋ ㄏㄨㄟˋ)", text: $settingsModel.newKey)
                    .textFieldStyle(.roundedBorder)
                Button("新增置頂") {
                    settingsModel.addCustomWord()
                }
                .disabled(settingsModel.newWord.trimmingCharacters(in: .whitespaces).isEmpty)
            }

            if let message = settingsModel.dictStatusMessage {
                Text(message)
                    .font(.caption)
                    .foregroundColor(message.contains("成功") ? .green : .red)
            }

            Divider()

            HStack {
                Text("辭典即時狀態：已載入 \(settingsModel.customDictLineCount) 筆自訂詞條")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Spacer()
                Button("立即重新載入") {
                    settingsModel.reloadCustomDict()
                }
            }
        }
        .padding()
        .onAppear {
            settingsModel.refreshDictCount()
        }
    }

    // MARK: - 3. 詞袋學習設定
    private var filteredUnigrams: [LearnedUnigramItem] {
        let query = learningSearch.trimmingCharacters(in: .whitespacesAndNewlines)
        let filtered = query.isEmpty
            ? settingsModel.learnedUnigrams
            : settingsModel.learnedUnigrams.filter { $0.word.localizedCaseInsensitiveContains(query) }
        return sortLearningItems(filtered, count: \.count, lastUsed: \.last_used)
    }

    private var filteredBigrams: [LearnedBigramItem] {
        let query = learningSearch.trimmingCharacters(in: .whitespacesAndNewlines)
        let filtered = query.isEmpty
            ? settingsModel.learnedBigrams
            : settingsModel.learnedBigrams.filter {
                $0.prev_word.localizedCaseInsensitiveContains(query)
                    || $0.next_word.localizedCaseInsensitiveContains(query)
                    || "\($0.prev_word)\($0.next_word)".localizedCaseInsensitiveContains(query)
            }
        return sortLearningItems(filtered, count: \.count, lastUsed: \.last_used)
    }

    private func sortLearningItems<T>(
        _ items: [T],
        count: KeyPath<T, Int>,
        lastUsed: KeyPath<T, Int64>
    ) -> [T] {
        items.sorted { lhs, rhs in
            switch learningSort {
            case .recent:
                if lhs[keyPath: lastUsed] != rhs[keyPath: lastUsed] {
                    return lhs[keyPath: lastUsed] > rhs[keyPath: lastUsed]
                }
                return lhs[keyPath: count] > rhs[keyPath: count]
            case .frequency:
                if lhs[keyPath: count] != rhs[keyPath: count] {
                    return lhs[keyPath: count] > rhs[keyPath: count]
                }
                return lhs[keyPath: lastUsed] > rhs[keyPath: lastUsed]
            }
        }
    }

    private func formatLearnedTimestamp(_ unixSeconds: Int64) -> String {
        guard unixSeconds > 0 else { return "—" }
        let date = Date(timeIntervalSince1970: TimeInterval(unixSeconds))
        return date.formatted(.relative(presentation: .named))
    }

    @ViewBuilder
    private func learningStatCard(
        title: String,
        valueText: String,
        systemImage: String,
        scope: LearningScope
    ) -> some View {
        let isSelected = learningScope == scope
        Button {
            learningScope = scope
        } label: {
            HStack(spacing: 10) {
                Image(systemName: systemImage)
                    .font(.title3)
                    .foregroundColor(isSelected ? .accentColor : .secondary)
                    .frame(width: 24)

                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Text(valueText)
                        .font(.headline)
                        .foregroundColor(.primary)
                }
                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 9)
            .background(
                RoundedRectangle(cornerRadius: 9)
                    .fill(isSelected ? Color.accentColor.opacity(0.10) : Color.secondary.opacity(0.07))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 9)
                    .stroke(isSelected ? Color.accentColor.opacity(0.45) : Color.secondary.opacity(0.12), lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder
    private func learningRowTrailing(
        count: Int,
        lastUsed: Int64,
        onDelete: @escaping () -> Void
    ) -> some View {
        HStack(spacing: 10) {
            Text("\(count) 次")
                .font(.caption.monospacedDigit())
                .padding(.horizontal, 7)
                .padding(.vertical, 2)
                .background(Color.accentColor.opacity(0.12))
                .foregroundColor(.accentColor)
                .cornerRadius(5)

            Text(formatLearnedTimestamp(lastUsed))
                .font(.caption)
                .foregroundColor(.secondary)
                .frame(minWidth: 68, alignment: .trailing)

            Button(role: .destructive, action: onDelete) {
                Image(systemName: "trash")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            .buttonStyle(.borderless)
            .help("刪除此學習紀錄")
        }
    }

    @ViewBuilder
    private func learningEmptyState(isFiltering: Bool) -> some View {
        VStack(spacing: 8) {
            Image(systemName: isFiltering ? "magnifyingglass" : "tray")
                .font(.title2)
                .foregroundColor(.secondary)
            Text(isFiltering ? "找不到符合「\(learningSearch)」的學習紀錄" : "目前尚未累積此類學習詞彙")
                .font(.subheadline)
                .foregroundColor(.secondary)
            if !isFiltering {
                Text("平時切換行魚輸入法選字輸出後，會自動記錄於此。")
                    .font(.caption)
                    .foregroundColor(.secondary.opacity(0.8))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private var learningTab: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("自適應詞袋學習 (SQLite)")
                        .font(.headline)
                    Text("越常選用的字詞與搭配會自動加權；可搜尋並刪除誤打或不想要的記憶。")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                Spacer()
                Button {
                    settingsModel.refreshStats()
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("重新讀取學習資料庫")
            }

            HStack(spacing: 10) {
                learningStatCard(
                    title: "單詞使用記憶",
                    valueText: "\(settingsModel.unigramCount) 個詞彙",
                    systemImage: "textformat.abc",
                    scope: .unigrams
                )
                learningStatCard(
                    title: "詞彙二元搭配",
                    valueText: "\(settingsModel.bigramCount) 種搭配",
                    systemImage: "arrow.left.arrow.right",
                    scope: .bigrams
                )
            }

            HStack(spacing: 8) {
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass")
                        .foregroundColor(.secondary)
                        .font(.caption)
                    TextField("搜尋已學習的詞彙...", text: $learningSearch)
                        .textFieldStyle(.plain)
                        .font(.subheadline)
                    if !learningSearch.isEmpty {
                        Button {
                            learningSearch = ""
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundColor(.secondary)
                                .font(.caption)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 5)
                .background(Color.secondary.opacity(0.08))
                .cornerRadius(7)

                Picker("排序", selection: $learningSort) {
                    ForEach(LearningSortOrder.allCases) { order in
                        Text(order.rawValue).tag(order)
                    }
                }
                .labelsHidden()
                .pickerStyle(.menu)
                .frame(width: 105)
            }

            let isFiltering = !learningSearch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            Group {
                switch learningScope {
                case .unigrams:
                    let items = filteredUnigrams
                    if items.isEmpty {
                        learningEmptyState(isFiltering: isFiltering)
                    } else {
                        ScrollView {
                            LazyVStack(spacing: 0) {
                                ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                                    HStack(spacing: 8) {
                                        Text(item.word)
                                            .font(.system(size: 13.5, weight: .medium))
                                            .lineLimit(1)
                                        Spacer()
                                        learningRowTrailing(count: item.count, lastUsed: item.last_used) {
                                            settingsModel.deleteUnigram(item.word)
                                        }
                                    }
                                    .padding(.horizontal, 12)
                                    .padding(.vertical, 7)

                                    if index < items.count - 1 {
                                        Divider().padding(.leading, 12)
                                    }
                                }
                            }
                        }
                    }

                case .bigrams:
                    let items = filteredBigrams
                    if items.isEmpty {
                        learningEmptyState(isFiltering: isFiltering)
                    } else {
                        ScrollView {
                            LazyVStack(spacing: 0) {
                                ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                                    HStack(spacing: 6) {
                                        Text(item.prev_word)
                                            .font(.system(size: 13, weight: .medium))
                                            .padding(.horizontal, 7)
                                            .padding(.vertical, 2)
                                            .background(Color.secondary.opacity(0.10))
                                            .cornerRadius(5)

                                        Image(systemName: "arrow.right")
                                            .font(.caption2)
                                            .foregroundColor(.secondary)

                                        Text(item.next_word)
                                            .font(.system(size: 13, weight: .medium))
                                            .padding(.horizontal, 7)
                                            .padding(.vertical, 2)
                                            .background(Color.secondary.opacity(0.10))
                                            .cornerRadius(5)

                                        Spacer()

                                        learningRowTrailing(count: item.count, lastUsed: item.last_used) {
                                            settingsModel.deleteBigram(prevWord: item.prev_word, nextWord: item.next_word)
                                        }
                                    }
                                    .padding(.horizontal, 12)
                                    .padding(.vertical, 7)

                                    if index < items.count - 1 {
                                        Divider().padding(.leading, 12)
                                    }
                                }
                            }
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color(nsColor: .controlBackgroundColor).opacity(0.6))
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(Color.secondary.opacity(0.16), lineWidth: 1)
            )

            HStack {
                Text(
                    learningScope == .unigrams
                        ? "提示：刪除單詞會一併移除包含該詞的搭配紀錄（最多顯示 500 筆）。"
                        : "提示：刪除搭配只會移除前後詞關聯，保留原本單詞權重（最多顯示 500 筆）。"
                )
                .font(.caption2)
                .foregroundColor(.secondary)

                Spacer()

                Button("重設全部...", role: .destructive) {
                    showClearConfirm = true
                }
                .controlSize(.small)
                .disabled(settingsModel.unigramCount == 0 && settingsModel.bigramCount == 0)
            }
        }
        .padding()
        .onAppear {
            settingsModel.refreshStats()
        }
        .alert("確定要清除所有學習記憶嗎？", isPresented: $showClearConfirm) {
            Button("取消", role: .cancel) {}
            Button("清除全部", role: .destructive) {
                settingsModel.clearLearningDb()
            }
        } message: {
            Text("此操作會清空目前累積的 \(settingsModel.unigramCount) 個單詞與 \(settingsModel.bigramCount) 種搭配記憶，無法復原。")
        }
    }

    // MARK: - 4. 關於行魚
    private var appIconImage: NSImage? {
        Bundle.main.image(forResource: NSImage.Name("AppIcon"))
    }

    private var appVersionText: String {
        let info = Bundle.main.infoDictionary
        let shortVersion = info?["CFBundleShortVersionString"] as? String ?? "1.0.0"
        let buildNumber = info?["CFBundleVersion"] as? String ?? "1"
        return "版本 \(shortVersion) (Build \(buildNumber))"
    }

    private var aboutTab: some View {
        VStack(spacing: 12) {
            Spacer()
            if let appIcon = appIconImage {
                Image(nsImage: appIcon)
                    .resizable()
                    .interpolation(.high)
                    .antialiased(true)
                    .aspectRatio(contentMode: .fit)
                    .frame(width: 80, height: 80)
                    .shadow(color: .black.opacity(0.18), radius: 6, y: 3)
            } else {
                Image(systemName: "fish.circle.fill")
                    .resizable()
                    .frame(width: 64, height: 64)
                    .foregroundColor(.accentColor)
            }

            Text("行魚輸入法 (XingYu IME)")
                .font(.title).bold()

            Text(appVersionText)
                .font(.caption).foregroundColor(.secondary)

            Text("「行如流水，如魚得水」\n專為 macOS 打造的高效能智慧注音輸入法")
                .multilineTextAlignment(.center)
                .font(.body)
                .padding(.vertical, 8)

            VStack(alignment: .leading, spacing: 4) {
                Text("• 核心技術：Swift 6 + Rust Core (Lattice Viterbi + SQLite WAL)")
                Text("• 智慧混打：大千注音音律學合法性驗證與英文即時推薦")
                Text("• 絕對優先：自訂辭典支援熱重載與首位強制置頂")
            }
            .font(.caption)
            .padding()
            .background(Color.secondary.opacity(0.08))
            .cornerRadius(8)

            Spacer()
        }
        .padding()
    }
}
