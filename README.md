# yt-dlp-tui

yt-dlp-tui 是一款專為終端機使用者設計的跨平台影音下載與媒體後處理工具。本專案將 yt-dlp 的解析下載能力與 FFmpeg 的無損封裝技術進行整合，具備非同步並行任務控制、智慧 Cookie 救援與無損軌道合併等功能。

提供靜默下載的自動化命令列（CLI）模式與基於 inquire 驅動的互動式終端選單（TUI）雙核心模式。

---

## 核心功能說明

### 1. 智慧 CLI / TUI 雙核心模式
* **自動化命令列（CLI）模式**：當指令帶齊 `--url`、`--media-type` 與 `--format` 等核心引數時，程式將自動進入 CLI 模式進行靜默下載，非常適合與系統排程任務（如 NAS、cron job）進行對接。
* **互動式選單（TUI）模式**：若未帶齊上述參數，程式會自動開啟互動式選單，引導使用者依序完成下載類型、封裝格式、多國字幕軌與 MKV 高解析度規格的直覺化配置。

### 2. Cookie 沙盒隔離與動態救援重試
* **Cookie 沙盒機制**：依據目標網址（如 YouTube、Bilibili、X、Instagram）自動匹配其設定目錄下的專屬 Cookie 實體檔案（如 `cookie_youtube.txt`），避免污染全域環境。
* **瀏覽器 Cookie 自動救援**：當遇到年齡限制或權限受限內容導致下載失敗時，系統會精準攔截認證錯誤（AuthError），並依據設定檔定義的優先順序（preferred_browsers），自動、無感地自本機瀏覽器中安全提取 Cookie 重新嘗試。
* **重試保護鎖**：單一任務最大重試限制為 3 次，防止因連線中斷或無效連結導致程式陷入死鎖。

### 3. 非同步並行下載與多進度條渲染
* **信號量並行控制**：基於 Tokio 非同步非阻塞 Runtime 與 Semaphore 機制，將最大並行數限制在安全的預設值 3（可自訂），降低因高頻請求而被平台封鎖 IP 的風險。
* **獨立進度條渲染**：利用 indicatif 繪製多個互不搶佔的下載與封裝進度條，動態顯示即時速度、百分比與預估剩餘時間。

### 4. 無損媒體封裝與字幕/彈幕清洗管線
* **Stream Copy 無損合併**：在 MP4 封裝格式下，自動限制視訊編碼為 H.264、音訊編碼為 AAC。合併時採用 `-c:v copy -c:a copy` 的無損合併模式，完全免除 CPU 二次編碼重轉碼的時間與效能損耗。
* **字幕過濾與彈幕封裝**：下載後會自動過濾 VTT 字幕檔中的特效標籤，保留純淨字幕。若偵測到 Bilibili 下載任務，會自動將 XML 格式彈幕轉換為 ASS 字幕音軌，並一同無損封裝至影片中。
* **廣告剔除**：預設注入 SponsorBlock 參數，在下載合併階段自動移除片頭片尾與置入性廣告。

### 5. 設定檔自動遷移與 Markdown 報表
* **設定檔無痛升級**：軟體升級時會自動對比本機與最新設定檔的 schema。若有新增欄位，會自動補齊並寫入白話註解，同時完美保留使用者既有的自訂路徑設定。
* **任務執行報表**：每次任務結束後，會在輸出目錄自動建立 `download_session-[Timestamp].md` Markdown 報表，詳細記錄各檔案的下載狀態、解析度、儲存檔名與錯誤堆疊。

---

## 系統依賴 (Prerequisites)

本工具啟動時會自動檢測系統環境，請確保您的環境中已安裝以下基礎套件：

1. **Python (3.10+)**：yt-dlp 的執行基礎。
2. **yt-dlp**：影音解析與下載核心。
3. **FFmpeg / FFprobe**：多媒體資訊偵測、字幕清洗與軌道封裝。
4. **danmaku2ass**（選用）：用於 Bilibili 彈幕轉換封裝。

---

## 安裝與部署指引 (Installation)

### 1. macOS / Linux 平台

#### 方法 A：一鍵自動化安裝（推薦）
本專案提供自動化安裝指令碼，可自動偵測您的作業系統與硬體架構，自 GitHub Releases 下載對應的三元組資產，完成解壓、權限設定與路徑配置：

```bash
curl -fsSL https://raw.githubusercontent.com/idolikechemistry/yt-dlp-tui/main/install.sh | bash
```
*註：由於需要 sudo 權限以移動檔案至 `/usr/local/bin`，安裝過程中可能會要求您輸入系統管理員密碼。*

#### 方法 B：手動下載部署
您也可以手動下載適用您平台架構的壓縮檔並進行安裝：
* macOS (Apple Silicon)：`yt-dlp-tui-aarch64-apple-darwin.tar.gz`
* Linux (x86_64)：`yt-dlp-tui-x86_64-unknown-linux-gnu.tar.gz`

下載後，打開終端機執行以下步驟進行部署（以 Linux 版本為例）：

1. **解壓檔案**：
   ```bash
   tar -xzvf yt-dlp-tui-x86_64-unknown-linux-gnu.tar.gz
   ```
2. **賦予執行權限**：
   ```bash
   chmod +x yt-dlp-tui
   ```
3. **將執行檔移至系統環境路徑**：
   ```bash
   sudo mv yt-dlp-tui /usr/local/bin/
   ```

*macOS 使用者注意事項*：若執行時遭遇系統阻擋（無法驗證開發者），請執行以下指令清除 Gatekeeper 安全隔離鎖：
```bash
xattr -d com.apple.quarantine /usr/local/bin/yt-dlp-tui
```

---

### 2. Windows 平台

Windows 使用者請採用手動方式部署：

1. 前往 GitHub Releases 頁面下載適用於 Windows x64 的 `yt-dlp-tui-x86_64-pc-windows-msvc.zip` 歸檔檔案。
2. 解壓縮後將內含的 `yt-dlp-tui.exe` 移動至您慣用的工作資料夾。
3. 建議將該資料夾路徑加入至系統環境變數的 `Path` 中，以便在任何 CMD 或 PowerShell 視窗中直接調用。

---

### 3. 從原始碼建構 (Build from Source)

若您希望自行從原始碼編譯專案，請確保本機已安裝 [Rust 與 Cargo](https://rustup.rs/) 編譯鏈：

```bash
# 1. 複製專案倉庫
git clone https://github.com/idolikechemistry/yt-dlp-tui.git
cd yt-dlp-tui

# 2. 編譯 Release 最佳化二進位檔
cargo build --release

# 3. 執行或部署編譯產物
# 編譯產物將位於 ./target/release/yt-dlp-tui
./target/release/yt-dlp-tui
```

---

## 偏好設定說明 (`config.toml`)

執行 `yt-dlp-tui --open-config` 可自動在檔案總管中開啟設定目錄。

### 設定檔儲存路徑
* **Windows**：`%APPDATA%\yt-dlp-tui\`
* **macOS**：`~/Library/Application Support/yt-dlp-tui/`
* **Linux**：`~/.config/yt-dlp-tui/`

### 欄位參數表
| 參數名稱 | 欄位型態 | 預設值 | 說明 |
| :--- | :--- | :--- | :--- |
| **version** | 字串 | 當前程式版本 | 用於結構升級對照與版本追蹤，請勿手動修改。 |
| **download_dir** | 字串 | `""` | 預設下載目錄。若留空則預設採用系統的「下載」資料夾。 |
| **cookie_dir** | 字串 | `""` | 專用 Cookie 存放目錄。若留空則預設採用本程式的設定目錄。 |
| **default_video_format** | 字串 | `"mp4"` | 預設視訊封裝格式（可選：`mp4`, `mkv`）。 |
| **default_audio_format** | 字串 | `"m4a"` | 預設音訊封裝格式（可選：`mp3`, `m4a`）。 |
| **max_concurrent_downloads** | 整數 | `3` | 最大並行任務數。建議介於 1 至 5 之間。 |
| **preferred_browsers** | 陣列 | `["chrome", "firefox", "safari", "edge"]` | 動態自動提取 Cookie 的本機瀏覽器優先順序。 |

### Cookie 沙盒命名規範
將您的 Netscape 標準文字格式 Cookie 檔案放置於 `cookie_dir` 設定目錄下，並依據網站重新命名：
* **YouTube** 專用：`cookie_youtube.txt`
* **Bilibili** 專用：`cookie_bilibili.txt`
* **Twitter/X** 專用：`cookie_twitter.txt`
* **Instagram** 專用：`cookie_instagram.txt`

---

## 指令參數說明 (CLI Options)

| 參數 | 說明 | 範例 |
| :--- | :--- | :--- |
| `-u, --url` | 貼上要下載的影片或播放清單網址（支援多個，以空格分隔） | `-u "https://..."` |
| `-m, --media-type` | 指定下載類型（`1`: 純音訊, `2`: 無聲影片, `3`: 有聲影片） | `-m 3` |
| `-f, --format` | 指定輸出格式（音訊：`mp3`/`m4a`；影片：`mp4`/`mkv`） | `-f mp4` |
| `-o, --output` | 指定本次任務的儲存路徑（覆寫 `download_dir` 設定） | `-o "./my_videos"` |
| `-c, --cookie` | 手動指定本地特定 Cookie 檔案路徑（最高優先權） | `-c "./cookie.txt"` |
| `--fc` | 強制調用設定資料夾內已儲存的 Cookie（跳過受限內容探測） | `--fc` |
| `--config` | 開啟互動式設定選單，配置路徑與慣用瀏覽器優先權 | `--config` |
| `--open-config` | 開啟設定檔與專屬 Cookie 的預設儲存目錄路徑 | `--open-config` |
| `--update` | 檢查並自動升級至 GitHub 最新 Release 版本 | `--update` |
| `-h, --help` | 顯示中文化參數說明手冊 | `-h` |
| `-V, --version` | 顯示當前應用程式版本 | `-V` |

---

## 授權條款 (License)

本專案採用 [MIT License](LICENSE) 授權條款，詳細資訊請參閱專案中的 LICENSE 檔案。
