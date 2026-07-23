use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// 🎯 確保版本號與 Cargo.toml 同步
fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// 🎯 為所有欄位提供明確的預設字串，防止被 serde 隱藏
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

// 🎯 新增：設定預設的慣用瀏覽器優先順序
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

    // 🎯 新增此欄位來儲存使用者自訂的慣用瀏覽器列表
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
            preferred_browsers: default_browsers(), // 🎯 套用預設順序
        }
    }
}

impl Config {
    /// 從指定路徑載入設定檔，並自動處理版本結構同步
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            // 🎯 直接實例化 Default，並呼叫 save() 統一寫入邏輯
            let default_config = Config::default();
            default_config.save(path)?;
            println!("✨ 初次執行：已為您生成帶有註解的設定檔 (config.toml)。");
            return Ok(default_config);
        }

        let content = fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)
            .context("解析設定檔失敗，若格式毀損請刪除設定檔讓程式重新生成")?;

        let app_ver = current_version();
        if config.version != app_ver {
            println!(
                "🔄 偵測到版本更新 ({} -> {})，正在同步設定檔結構...",
                config.version, app_ver
            );
            config.version = app_ver;
            // 升級時一樣呼叫統一的 save()，寫入新欄位並保留舊設定
            config.save(path)?;
            println!("✨ 設定檔結構已自動補齊，並保留您的自訂內容。");
        }

        Ok(config)
    }

    /// 🎯 統一的 save 方法：負責將詳細的使用手冊「烙印」在設定檔頂部，並寫入硬碟
    pub fn save(&self, path: &Path) -> Result<()> {
        let data = toml::to_string_pretty(self).context("序列化設定資料失敗")?;

        // 📝 統一的手動註解區 (已更新至 yt-dlp-tui 並加入 preferred_browsers 的完整說明)
        let manual = r#"# ======================================================
# yt-dlp-tui 使用者設定檔
# ======================================================
# 💡 提示：本檔案在版本更新時會自動重構結構。
#
# 📍 download_dir:
# 預設下載目錄。留空則使用系統「下載」資料夾。
# - Mac: ~/Library/Application Support/yt-dlp-tui/
# - Linux: ~/.config/yt-dlp-tui/
# - Windows: %APPDATA%\yt-dlp-tui\
# 範例: "/Users/username/Movies"
#
# 🍪 cookie_dir:
# 存放 cookie_youtube.txt 等檔案的目錄。
# 留空則使用本程式的設定資料夾。
#
# 🎬 default_video_format / default_audio_format:
# 預設封裝格式。影片可選: mp4, mkv / 音訊可選: mp3, m4a
#
# ⚡ max_concurrent_downloads:
# 最大並行下載數。建議範圍 1-5，設太高可能導致 IP 被封鎖。
#
# 🌐 preferred_browsers:
# 發生下載權限/年齡限制錯誤時，自動獲取 Cookie 的瀏覽器優先列表。
# 預設順序為 ["chrome", "firefox", "safari", "edge"]。
# 支援：chrome, firefox, safari, edge, brave, opera, vivaldi, chromium, whale, mullvad, orion 等。
#
# ⚠️ version: 版本追蹤標籤，請勿手動修改。
# ======================================================
"#;

        // 將手冊與機器生成的資料拼接
        let final_content = format!("{}{}", manual, data);
        fs::write(path, final_content).with_context(|| format!("無法寫入設定檔至: {:?}", path))?;

        Ok(())
    }
}
