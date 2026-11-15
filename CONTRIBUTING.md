# 貢獻指南 (Contributing to 行魚輸入法 XingYuIME)

感謝你對 **行魚輸入法 (XingYuIME)** 的興趣！本專案採用 **Rust 核心引擎 + Swift 原生 InputMethodKit 前端** 的混合架構，兼顧高效率斷詞演算與 macOS 原生使用者體驗。

## 開發環境需求

- **macOS 13.0 (Ventura) 或更新版本**
- **Rust Toolchain** (`rustc`, `cargo`)
- **Xcode Command Line Tools** (`swiftc`, `codesign`)
- **Python 3**（`scripts/install.sh` 用於同步更新系統輸入來源偏好清單）

## 專案架構導覽

```text
IME/
├── assets/                     # Logo 與 GitHub Banner 視覺資源
├── rust_engine/                # 核心輸入法推論與斷詞引擎 (Rust Staticlib + C-ABI)
│   ├── Cargo.toml
│   ├── data/
│   │   └── lexicon.txt         # 內建繁體中文注音詞庫（189,654 筆詞條）
│   └── src/
│       ├── lib.rs              # 核心 Engine 控制狀態機、緩衝區管理、多音節切換與整合測試
│       ├── engine/
│       │   ├── decoder.rs      # Viterbi 整句最佳路徑斷詞解碼器與候選區段邏輯
│       │   └── user_db.rs      # SQLite WAL 個人用詞頻率與學習資料庫
│       ├── ffi/
│       │   └── c_api.rs        # 暴露給 Swift 前端的 C-ABI 介面
│       ├── keyboard/
│       │   ├── layout.rs       # KeyboardLayout trait 與注音符號抽象層
│       │   ├── dachen.rs       # 標準大千式注音鍵盤配置實作
│       │   ├── eten.rs         # 倚天配置實作骨架
│       │   └── pinyin.rs       # 漢語拼音配置實作骨架
│       ├── lexicon/
│       │   ├── trie.rs         # 內建詞庫前綴查詢與詞頻檢索
│       │   └── custom_dict.rs  # 使用者自訂辭典載入與熱重載
│       └── phonetics/
│           └── zhuyin.rs       # 注音音節拼合與輸入順序容錯正規化
├── swift_frontend/             # macOS InputMethodKit 前端介面 (Swift / SwiftUI)
│   ├── Resources/
│   │   ├── Info.plist          # IMK 服務宣告、TIS Input Source ID 與圖示設定
│   │   ├── AppIcon.icns        # 應用程式 Bundle 圖示
│   │   ├── XingYuTemplate.tiff # 選單列 Template 飛魚圖示 (18x18 / 36x36)
│   │   └── *.lproj/            # 多語系輸入法名稱本地化
│   └── Sources/
│       ├── main.swift          # IMKServer 啟動進入點
│       ├── AppDelegate.swift   # 應用程式生命週期與選單設定
│       ├── Bridging-Header.h   # Rust C-ABI 橋接標頭檔
│       ├── EngineBridge.swift  # Swift 端安全封裝與 JSON Snapshot 解析
│       ├── InputController.swift # 按鍵攔截、全形標點、組字預覽與游標區段重選
│       ├── CandidateWindow.swift # SwiftUI 毛玻璃懸浮候選字視窗
│       ├── SettingsModel.swift # 偏好設定與自訂辭典資料模型
│       ├── SettingsView.swift  # 四分頁偏好設定介面 (一般 / 自訂詞庫 / 學習統計 / 關於)
│       └── SettingsWindowController.swift
└── scripts/
    ├── build.sh                # 編譯 Rust staticlib 與 Swift `.app` 並進行 ad-hoc 簽章
    └── install.sh              # 建置、安裝至 ~/Library/Input Methods 並向系統註冊輸入來源
```

## 常用開發與測試指令

### 1. 執行 Rust 引擎單元測試

修改 `rust_engine` 內的斷詞、注音組合、容錯邏輯或詞庫解析後，請務必執行完整測試：

```bash
cargo test --manifest-path rust_engine/Cargo.toml
```

### 2. 本機編譯打包 `.app`

```bash
./scripts/build.sh
```

編譯產物會輸出至 `build/XingYuIME.app`。

### 3. 安裝並於本機 macOS 即時測試

```bash
./scripts/install.sh
```

此腳本會自動結束舊行程、複製 `XingYuIME.app` 至 `~/Library/Input Methods/`、清理隔離屬性並呼叫 `TISRegisterInputSource` 與重啟 `TextInputMenuAgent`。

## 擴充指南

### 新增鍵盤配置 (Keyboard Layout)

你可以透過實作 `rust_engine/src/keyboard/layout.rs` 中的 `KeyboardLayout` trait，來擴充新的鍵盤排列（例如完成 `eten.rs` 或新增許氏鍵盤）：

1. 在 `rust_engine/src/keyboard/` 新增對應模組並實作 `KeyboardLayout`。
2. 將該模組註冊至 `rust_engine/src/keyboard/mod.rs`。
3. 補充對應的單元測試以驗證聲母、介音、韻母與聲調映射正確無誤。

## 提交 Pull Request 準則

1. 保持提交紀錄清晰，每個 PR 聚焦於單一功能修正或效能改進。
2. 確認 `cargo test --manifest-path rust_engine/Cargo.toml` 全數通過。
3. 確認 `./scripts/build.sh` 可以無錯誤編譯出完整 `XingYuIME.app`。
4. 不要將 `build/`、`rust_engine/target/`、`.DS_Store` 或個人測試用 `.db` 檔案提交至版本庫。
