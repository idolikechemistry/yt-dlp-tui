# yt-dlp-tui

**yt-dlp-tui** 是一款為終端機使用者設計的現代化、極致效能影音下載與封裝工具。它完美融合了 [yt-dlp](https://github.com/yt-dlp/yt-dlp) 的強大下載能力與 [ffmpeg](https://github.com/FFmpeg/FFmpeg) 的專業後處理封裝技術，並具備優雅的命令列（CLI）自動化參數與直覺的一體化終端選單（TUI）雙核心模式。

本專案集成了 **「並行下載限制」**、**「無損軌道複製合併」**、**「CJK 彈幕與純淨字幕過濾封裝」**，以及專為解決受限影音開發的 **「動態瀏覽器 Cookie 黑名單與一鍵自動繞過」** 功能。

---

## 核心特色 (Key Features)

### 1. 互動與自動雙模切換 (Dual-Mode Intelligence)
* **CLI 自動化模式**：提供完整的命令列 Flag（`-u`、`-m`、`-f`），一經調用便自動開啟靜默下載，非常適合與系統 Cron Job 或 NAS 自動化指令碼排程對接。
* **直覺式 TUI 選單**：若未帶齊參數，程式會自動開啟全 `inquire` 驅動的互動選單。我們對影片解析度、封裝編碼與字幕進行了「去技術化」的白話提示優化，新手也能一目了然、輕鬆操作。

### 2. 智慧 Cookie 沙盒與「動態黑名單過濾重試」
* **Cookie 沙盒隔離**：依據目標網址（Bilibili、YouTube、X 等）自動匹配專屬實體 Cookie（如 `cookie_youtube.txt`），不污染全域環境。
* **一鍵自動繞過**：若您在 `config.toml` 中只填寫了一款主力瀏覽器（如 Chrome），當影片因年齡或會員限制下載失敗時，系統會**自動套用該瀏覽器 Cookie 重試，全程零按鍵干預**。
* **動態黑名單過濾**：若配置了多個瀏覽器，當前瀏覽器 Cookie 驗證失敗後（精準識別 `AuthError`而非網路瞬斷），系統會自動將其排除出候選列表，防止使用者重複踩坑。
* **實體重試防護鎖**：内建同一批任務最大 3 次重試限制，徹底杜絕程式因極端連線或 404 而陷入 TUI 無盡迴圈的死鎖風險。

### 3. 高並行下載控制與 Indicatif 渲染
* 藉由 Tokio 非同步非阻塞執行緒與 **Semaphore 信號量機制**，將預設並行數限制在安全的 **3**（可在設定檔調整），並主動降低因高頻請求而被影音平台封鎖 IP 的風險。
* 採用 `indicatif` 繪製多個互不搶佔、精美且帶有時間估算與實時速度的下載與封裝進度條。

### 4. 影音無損封裝與 CJK 字幕/彈幕淨化管線
* **無損 H.264/AAC 合併**：針對 MP4 容器，自動限制影軌與音軌編碼格式，封裝時採用 **`-c:v copy -c:a copy` 的無損合併模式**，完全省去耗時的 CPU 二度解碼與重轉碼，速度提升高達 10 倍以上。
* **外掛字幕與彈幕封裝**：下載後自動過濾 VTT 的特效標籤保留純淨字幕，若偵測到 Bilibili 影音，會自動將 XML 彈幕轉為 ASS 軌道，一併使用 `ffmpeg` 無損封裝進 MP4/MKV 影片中，解鎖極致觀看體驗。
* **廣告業配剔除**：預設注入 `SponsorBlock` 引數，在下載合併階段自動剔除片頭片尾與置入廣告，精簡硬碟佔用。

### 5. 設定檔自適應升級與 Markdown 報表
* 軟體每次升級時，會自動比對本地與最新版的欄位結構。**若有新增欄位（如 `preferred_browsers`），會自動補齊並補上白話註解，同時完美保留使用者既有的自訂路徑設定**，防範解析崩潰。
* 下載結束後，會自動在輸出資料夾內建立 `download_session.md` 任務執行報表，清晰記錄每部影片的下載狀態與詳細錯誤堆疊，方便日後追蹤與除錯。

---

## 系統依賴 (Prerequisites)

無論您使用哪種安裝方式，請確保您的系統已安裝以下核心相依套件（本工具在啟動時會自動進行智慧環境檢測）：

1. **Python (3.10+)**：`yt-dlp` 的執行基礎。
2. **yt-dlp**：影音下載核心。
3. **ffmpeg / ffprobe**：多媒體軌道資訊偵測、字幕清洗與無損封裝。
4. **danmaku2ass** (可選)：用於 Bilibili 彈幕轉換。

---

## 安裝與更新指引 (Installation)

### macOS (推薦 Homebrew)
```bash
# 新增倉庫並安裝
brew tap idolikechemistry/yt-dlp-tui && brew install idolikechemistry/yt-dlp-tui/yt-dlp-tui

# 日後更新
brew upgrade yt-dlp-tui

# 解除安裝
brew uninstall yt-dlp-tui && brew untap idolikechemistry/yt-dlp-tui
```

### Linux
```bash
# 一鍵下載二進位檔並賦予權限
curl -L "https://github.com/idolikechemistry/yt-dlp-tui/releases/latest/download/yt-dlp-tui-linux-x64" -o yt-dlp-tui && chmod +x yt-dlp-tui && sudo mv yt-dlp-tui /usr/local/bin/
```
*(Mac 晶片使用者亦可手動將 `yt-dlp-tui-linux-x64` 替換為 `yt-dlp-tui-mac-arm64` 執行同等安裝)*

### Windows
1. 請前往 [Releases](https://github.com/idolikechemistry/yt-dlp-tui/releases) 頁面下載最新的 `yt-dlp-tui-windows-x64.exe` 。
2. 將其手動放置於您慣用的資料夾中（建議加入系統環境變數 `Path` 以便在任一 CMD/PowerShell 視窗直接呼叫）。

---

## 指令參數說明 (Options)

| 參數 | 說明 | 命令範例 |
| :--- | :--- | :--- |
| `-u, --url` | 貼上要下載的影片或播放清單網址 (支援多個，以空格隔開) | `-u "https://..."` |
| `-m, --media-type` | 指定下載類型 (`1`: 純音訊, `2`: 無聲影片, `3`: 有聲影片) | `-m 3` |
| `-f, --format` | 指定輸出格式 (音訊可選: `mp3`/`m4a`；影片可選: `mp4`/`mkv`) | `-f mp4` |
| `-o, --output` | 指定儲存資料夾路徑 (預設為系統的 `Downloads`) | `-o "./my_videos"` |
| `-c, --cookie` | 手動指定本地特定 Cookie 檔案路徑 | `-c "./cookie.txt"` |
| `--fc` | 強制調用 App 設定資料夾內已儲存的 Cookie | `--fc` |
| `--open-config` | 開啟設定與專屬 Cookie 的系統路徑 | `--open-config` |
| `-h, --help` | 顯示所有中文化參數說明手冊 | `-h` |
| `-V, --version` | 顯示當前應用程式版本 | `-V` |

---

## 偏好設定 (`config.toml`)

開啟終端機執行 `yt-dlp-tui --open-config`，系統會自動在檔案管理器中為您開啟設定目錄。您可以使用任何文字編輯器編輯 `config.toml`

### 設定檔預設儲存路徑
* **Windows**：`%APPDATA%\yt-dlp-tui\`
* **macOS**：`~/Library/Application Support/yt-dlp-tui/`
* **Linux**：`~/.config/yt-dlp-tui/`

---

## Cookie 沙盒管理防護

部分高畫質、年齡限制或會員/粉絲專屬影片，必須提供登入後的 Cookie 才能獲取影片資料。
您可以利用瀏覽器外掛將 Cookie 匯出為 **Netscape 標準文字格式**，並重新命名放入設定目錄中：

* **YouTube** 專用：`cookie_youtube.txt`
* **Bilibili** 專用：`cookie_bilibili.txt`
* **Twitter/X** 專用：`cookie_twitter.txt`
* **Instagram** 專用：`cookie_instagram.txt`

系統在掃描到對應網站時，會**優先自動套用這些沙盒 Cookie 進行解析**，不損害日常主瀏覽器的安全性。

---

## 開發者構建指引 (Build from Source)

如果您希望從原始碼構建此專案，請確保您已安裝 [Rust & Cargo](https://rustup.rs/) 編譯鏈：

```bash
# 1. 複製倉庫
git clone https://github.com/idolikechemistry/yt-dlp-tui.git
cd yt-dlp-tui

# 2. 編譯 Release 版本 (已優化執行效能與體積)
cargo build --release

# 3. 執行編譯後的程式
./target/release/yt-dlp-tui
```

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
