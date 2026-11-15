import SwiftUI
import AppKit

public struct SettingsView: View {
    @ObservedObject var settingsModel: SettingsModel

    public init(settingsModel: SettingsModel) {
        self.settingsModel = settingsModel
    }

    public var body: some View {
        TabView {
            generalTab
                .tabItem {
                    Label("一般設定", systemImage: "gearshape")
                }

            customDictTab
                .tabItem {
                    Label("自訂辭典", systemImage: "character.book.closed")
                }

            learningTab
                .tabItem {
                    Label("詞袋學習", systemImage: "brain.head.profile")
                }

            aboutTab
                .tabItem {
                    Label("關於行魚", systemImage: "info.circle")
                }
        }
        .frame(width: 540, height: 440)
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

                Toggle("啟用中英混打 (智慧判斷英文單字)", isOn: $settingsModel.enableMixedTyping)
                Toggle("首字母大寫自動切換英文模式", isOn: $settingsModel.shiftToEnglish)
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
    private var learningTab: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("自適應學習資料庫 (SQLite)")
                .font(.headline)

            Text("行魚輸入法會在使用過程中，逐步記錄您的常用字詞與前後搭配習慣，越用越聰明。")
                .font(.caption)
                .foregroundColor(.secondary)

            HStack(spacing: 20) {
                VStack(alignment: .leading) {
                    Text("單詞使用記憶 (Unigrams)")
                        .font(.caption).foregroundColor(.secondary)
                    Text("\(settingsModel.unigramCount) 個詞彙")
                        .font(.title2).bold()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
                .background(Color.secondary.opacity(0.08))
                .cornerRadius(10)

                VStack(alignment: .leading) {
                    Text("詞彙二元搭配 (Bigrams)")
                        .font(.caption).foregroundColor(.secondary)
                    Text("\(settingsModel.bigramCount) 種搭配")
                        .font(.title2).bold()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
                .background(Color.secondary.opacity(0.08))
                .cornerRadius(10)
            }

            Spacer()

            VStack(alignment: .leading, spacing: 8) {
                Text("重設學習記憶")
                    .font(.subheadline).bold()
                Text("若輸入法記憶了不想要的錯字或頻率異常，您可以隨時重設學習資料庫。")
                    .font(.caption).foregroundColor(.secondary)

                Button(role: .destructive) {
                    settingsModel.clearLearningDb()
                } label: {
                    Text("清除所有學習記憶並重設")
                }
            }
        }
        .padding()
        .onAppear {
            settingsModel.refreshStats()
        }
    }

    // MARK: - 4. 關於行魚
    private var aboutTab: some View {
        VStack(spacing: 12) {
            Spacer()
            Image(systemName: "fish.circle.fill")
                .resizable()
                .frame(width: 64, height: 64)
                .foregroundColor(.accentColor)

            Text("行魚輸入法 (XingYu IME)")
                .font(.title).bold()

            Text("版本 1.0.0 (Build 2026)")
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
