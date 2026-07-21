use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// 確保版本號與 Cargo.toml 同步
fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// 為所有欄位提供明確的預設值，防止在反序列化舊設定檔時缺失而崩潰
fn default_empty_string() -> String {
    "".into()
}

fn default_concurrency() -> u32 {
    3
}

fn default_video_fmt() -> String {
    "mp4".into()
}

fn default_audio_fmt() -> String {
    "m4a".into()
}

//新增：預設的慣用瀏覽器列表 (用於自動 Cookie 匯入與動態排除重試)
fn default_browsers() -> Vec<String> {
    vec![
        "chrome".to_string(),
        "firefox".to_string(),
        "safari".to_string(),
        "edge".to_string(),
    ]
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(default = "current_version")]
    pub version: String,

    #[serde(default = "default_empty_string")]
    pub download_dir: String,

    #[serde(default = "default_empty_string")]
    pub cookie_dir: String,

    #[serde(default = "default_video_fmt")]
    pub default_video_format: String,

    #[serde(default = "default_audio_fmt")]
    pub default_audio_format: String,

    #[serde(default = "default_concurrency")]
    pub max_concurrent_downloads: u32,

    //核心升級：加入自訂瀏覽器偏好列表，支援安全自動跳過與黑名單排除機制
    #[serde(default = "default_browsers")]
    pub preferred_browsers: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: current_version(),
            download_dir: "".into(),
            cookie_dir: "".into(),
            default_video_format: "mp4".into(),
            default_audio_format: "m4a".into(),
            max_concurrent_downloads: 3,
            preferred_browsers: default_browsers(),
        }
    }
}

impl Config {
    /// 從指定路徑載入設定檔，並自動處理跨版本結構同步與補齊
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            // 直接實例化預設設定，並寫入具有詳細說明手冊的全新 config.toml
            let default_config = Config::default();
            default_config.save(path)?;
            println!("初次執行：已為您生成帶有詳細說明與註解的設定檔 (config.toml)。");
            return Ok(default_config);
        }

        let content = fs::read_to_string(path)?;
        
        // 這裡在解析時，Serde 的 #[serde(default = "...")] 
        // 會自動為舊版設定檔補齊缺失的新欄位（例如 preferred_browsers），絕不發生崩潰
        let mut config: Config = toml::from_str(&content)
            .context("解析設定檔失敗，若格式毀損請刪除設定檔讓程式重新生成")?;

        let app_ver = current_version();
        if config.version != app_ver {
            println!(
                "偵測到版本更新 ({} -> {})，正在平滑升級並同步設定檔結構...",
                config.version, app_ver
            );
            config.version = app_ver;
            
            // 升級時一樣呼叫統一的 save()，烙印手冊並自動補齊新欄位寫入硬碟
            config.save(path)?;
            println!("設定檔結構已自動補齊（已為您新增並保留預設瀏覽器清單），並保留您的個人自訂內容。");
        }

        Ok(config)
    }

    /// 統一的 save 方法：負責將詳細的使用手冊「烙印」在設定檔頂部，並寫入硬碟
    pub fn save(&self, path: &Path) -> Result<()> {
        let data = toml::to_string_pretty(self).context("序列化設定資料失敗")?;

        // 統一的手動詳細說明註解區
        let manual = r#"# =====================================================================
# yt-dlp-tui 使用者偏好設定檔 (config.toml)
# =====================================================================
# 提示：本檔案在程式版本更新時會自動重構結構，並自動保留您既有的自訂內容。
# 
# download_dir:
#   預設下載目錄。若留空（""）則程式會自動套用您系統預設的「下載」資料夾。
#   - macOS 預設設定夾位置: ~/Library/Application Support/yt-dlp-tui/
#   - Linux 預設設定夾位置: ~/.config/yt-dlp-tui/
#   - Windows 預設設定夾位置: %APPDATA%\yt-dlp-tui\
#   範例: download_dir = "/Users/username/Movies"
# 
# cookie_dir:
#   存放 cookie_youtube.txt, cookie_bilibili.txt 等實體 Cookie 檔案的目錄。
#   若留空（""）則預設使用本程式的設定資料夾。
# 
# default_video_format / default_audio_format:
#   預設的影音封裝格式。
#   - 影片可選: mp4, mkv
#   - 音訊可選: mp3, m4a
# 
# max_concurrent_downloads:
#   最大並行下載數。建議範圍為 1-5，設置過高極易觸發影音網站的安全連線限制或封鎖 IP。
# 
# preferred_browsers:
#   您的慣用瀏覽器清單（依優先順序排列）。
#   當遇到需要登入、權限或年齡限制的影片時，系統會自動在重試選單中過濾並顯示。
#   零干預密技：如果您在清單中僅填入「單一」瀏覽器，如 preferred_browsers = ["chrome"]
#   則系統遇到 Cookie 失效或受限時，會「自動跳過選單、直接套用該瀏覽器 Cookie 進行重試」，
#   提供您最極致與自動化的流暢下載體驗！
#   支援選項: "chrome", "firefox", "safari", "edge", "brave", "vivaldi", "opera"
#   範例: preferred_browsers = ["chrome", "edge"]
# 
# version: 版本追蹤標籤，請勿手動修改，否則會影響跨版本自動升級功能。
# =====================================================================
"#;

        // 將手冊與機器生成的資料拼接
        let final_content = format!("{}{}", manual, data);
        fs::write(path, final_content).with_context(|| format!("無法寫入設定檔至: {:?}", path))?;

        Ok(())
    }
}
