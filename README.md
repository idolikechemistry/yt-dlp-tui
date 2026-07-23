# yt-dlp-tui

yt-dlp-tui 是一款基於 Rust 開發的 yt-dlp 與 FFmpeg 封裝工具，整合了命令列介面（CLI）與終端機使用者介面（TUI），提供多執行緒非同步並行下載、無損串流合併、CJK 字幕與彈幕處理，以及瀏覽器 Cookie 自動匯入與錯誤恢復機制。

---

## 核心功能

### 1. CLI 與 TUI 雙模切換
* **自動化命令列模式（CLI）**：提供 `-u`、`-m` 與 `-f` 等參數。完整帶入參數時將直接啟動靜默下載，適用於自動化腳本、排程任務或 NAS 系統。
* **終端互動模式（TUI）**：當啟動參數不完整時，系統將自動啟動基於 inquire 的互動式選單，引導使用者完成下載配置（包括多國語言字幕勾選、MKV 高解析度規格選擇等）。

### 2. 智慧 Cookie 沙盒與瀏覽器自動重試
* **Cookie 沙盒隔離**：依據目標 URL 自動匹配對應網站的專用 Cookie 檔案（如 `cookie_youtube.txt`、`cookie_bilibili.txt`），避免污染全域環境。
* **自動提取重試**：當下載因年齡限制或權限受限失敗時，系統會精準識別認證錯誤（`AuthError`），並依據設定檔中的瀏覽器優先順序（`preferred_browsers`），自動提取使用者本地端瀏覽器的 Cookie 進行重試，無需人工干預。
* **安全限制機制**：針對同一批任務設定最大 3 次重試限制，防止因極端連線或無效 URL 導致程序進入死鎖迴圈。

### 3. 非同步並行下載與進度渲染
* **Semaphore 並行控制**：基於 Tokio 非同步執行緒與信號量機制，預設將最大並行數限制為 3（可在設定檔中調整），降低因請求頻率過高而遭影音平台暫時封鎖 IP 的風險。
* **獨立進度渲染**：採用 `indicatif` 繪製多進度條，動態顯示各任務的即時速率、已下載百分比與預估剩餘時間，避免終端輸出互相搶佔覆蓋。

### 4. 無損媒體後處理管線
* **無損合併（Stream Copy）**：針對 MP4 容器下載，自動限制視訊為 H.264、音訊為 AAC 格式，封裝時採用 `-c:v copy -c:a copy` 參數直接進行軌道複製，免除耗費 CPU 的二次編碼過程，顯著提升合併速度。
* **字幕與彈幕封裝**：下載後自動過濾 WebVTT 的特效標籤，保留純淨字幕。若偵測到 Bilibili 下載任務，會自動調用 `danmaku2ass` 將 XML 彈幕檔轉換為 ASS 字幕格式，並與清洗後的字幕一併封裝至影片中。
* **置入性內容過濾**：預設注入 SponsorBlock 引數，在封裝階段自動剔除片頭、片尾與置入性廣告。

### 5. 自適應升級與日誌系統
* **設定檔結構同步**：軟體升級時自動對比本地 `config.toml` 與最新版本的欄位結構，自動補齊新增欄位（如 `preferred_browsers`）並保留使用者原有的自訂路徑配置。
* **Markdown 任務報表**：每次執行完成後，自動於輸出目錄生成 `download_session-[Timestamp].md` 執行日誌，詳細記錄各任務的下載狀態、解析度、儲存檔名及錯誤堆疊資訊，便於除錯追蹤。

---

## 系統依賴

本工具啟動時會自動進行執行環境與關鍵依賴套件檢測，請確保系統已完成以下套件的安裝：
1. **Python (3.10+)**：執行 yt-dlp 的基礎環境。
2. **yt-dlp**：影音解析與下載核心。
3. **FFmpeg / FFprobe**：媒體軌道偵測、字幕清洗與無損封裝核心。
4. **danmaku2ass** (選用)：Bilibili 彈幕轉換所需的指令列工具。

---

## 安裝指引

目前專案尚未配置 Homebrew 軟體源，非 Windows 平台使用者請依循以下手動部署說明安裝。

### macOS / Linux 平台

請由 GitHub Releases 頁面下載適用於您系統架構的壓縮檔：
* macOS (ARM64): `yt-dlp-tui-aarch64-apple-darwin.tar.gz`
* Linux (x64): `yt-dlp-tui-x86_64-unknown-linux-gnu.tar.gz`

下載後，請執行以下安裝流程：

1. **解壓縮歸檔檔案**：
   ```bash
   # 以 Linux x64 版本為例
   tar -xzvf yt-dlp-tui-x86_64-unknown-linux-gnu.tar.gz
   ```

2. **賦予可執行權限**：
   ```bash
   chmod +x yt-dlp-tui
   ```

3. **移動至系統環境路徑**（以便在任何終端路徑下直接調用）：
   ```bash
   sudo mv yt-dlp-tui /usr/local/bin/
   ```

*macOS 使用者注意事項*：若執行時遭遇 Gatekeeper 攔截（顯示無法驗證開發者），請在終端機執行以下命令清除隔離屬性：
```bash
xattr -d com.apple.quarantine /usr/local/bin/yt-dlp-tui
```

### Windows 平台

1. 前往 GitHub Releases 下載最新版的 `yt-dlp-tui-x86_64-pc-windows-msvc.zip`。
2. 解壓縮後將 `yt-dlp-tui.exe` 放置於自訂工作目錄。
3. 建議將該工作目錄路徑新增至系統環境變數的 `Path` 中，以便在命令提示字元（CMD）或 PowerShell 中直接調用。

### 從原始碼編譯 (Build from Source)

您亦可直接透過 Rust 工具鏈進行本地編譯：
```bash
# 1. 複製倉庫
git clone https://github.com/idolikechemistry/yt-dlp-tui.git
cd yt-dlp-tui

# 2. 編譯 Release 版本
cargo build --release

# 3. 執行編譯後的程式
./target/release/yt-dlp-tui
```

---

## 設定檔說明

偏好設定儲存於 `config.toml` 中，可在終端機執行 `yt-dlp-tui --open-config` 自動打開設定路徑。

### 設定檔儲存路徑
* **Windows**：`%APPDATA%\yt-dlp-tui\`
* **macOS**：`~/Library/Application Support/yt-dlp-tui/`
* **Linux**：`~/.config/yt-dlp-tui/`

### 設定參數表 (config.toml)

| 參數 | 型態 | 預設值 | 說明 |
| :--- | :--- | :--- | :--- |
| `version` | 字串 | 當前程式版本 | 用於設定檔結構自動升級追蹤，請勿手動修改。 |
| `download_dir` | 字串 | `""` | 預設下載存檔目錄。留空時將預設使用系統的 Downloads 目錄。 |
| `cookie_dir` | 字串 | `""` | 專屬 Cookie 檔案存放目錄。留空時預設為程式的設定資料夾。 |
| `default_video_format` | 字串 | `"mp4"` | 預設視訊封裝格式（可選：mp4, mkv）。 |
| `default_audio_format` | 字串 | `"m4a"` | 預設音訊封裝格式（可選：mp3, m4a）。 |
| `max_concurrent_downloads` | 整數 | `3` | 最大並行下載任務數。建議範圍為 1 至 5，設過高易遭平台封鎖 IP。 |
| `preferred_browsers` | 陣列 | `["chrome", "firefox", "safari", "edge"]` | 當權限受阻時，自動提取 Cookie 的本機瀏覽器候選清單（支援 chrome, firefox, safari, edge, brave, opera, vivaldi 等）。 |

### Cookie 沙盒命名規範

將瀏覽器導出的 Netscape 格式 Cookie 檔案重新命名，並置於 `cookie_dir` 目錄下：
* YouTube 專用：`cookie_youtube.txt`
* Bilibili 專用：`cookie_bilibili.txt`
* Twitter / X 專用：`cookie_twitter.txt`
* Instagram 專用：`cookie_instagram.txt`

---

## 參數與指令說明

### 命令列參數 (CLI Options)

| 參數 | 說明 | 範例 |
| :--- | :--- | :--- |
| `-u, --url` | 指定下載目標 URL，支援輸入多個網址（以空格分隔） | `-u "https://..."` |
| `-m, --media-type` | 指定下載媒體類型（1: 純音訊, 2: 無聲視訊, 3: 有聲視訊） | `-m 3` |
| `-f, --format` | 指定封裝格式（音訊：mp3/m4a；視訊：mp4/mkv） | `-f mp4` |
| `-o, --output` | 手動覆寫本次任務的下載存檔路徑 | `-o "./my_videos"` |
| `-c, --cookie` | 手動指定本地特定 Cookie 檔案路徑 | `-c "./cookie.txt"` |
| `--fc` | 強制調用設定夾中已儲存的 Cookie（跳過受限內容探測） | `--fc` |
| `--config` | 開啟互動式 TUI 設定引導，配置路徑與瀏覽器清單 | `--config` |
| `--open-config` | 開啟作業系統中該設定檔與專屬 Cookie 的預設儲存目錄 | `--open-config` |
| `--update` | 檢查並自動升級至 GitHub 最新 Release 版本 | `--update` |
| `-h, --help` | 顯示中文化幫助說明手冊 | `-h` |
| `-V, --version` | 顯示當前應用程式版本 | `-V` |

### 執行範例

1. **自動化模式（CLI 靜默下載）**：
   ```bash
   yt-dlp-tui -u "https://www.youtube.com/watch?v=..." -m 3 -f mp4
   ```

2. **互動模式（TUI）**：
   ```bash
   yt-dlp-tui
   ```

---

## 授權條款

本專案採用 [MIT License](LICENSE) 進行授權。
