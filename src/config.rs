use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct AppSettings {
    pub version: String,
    pub download_dir: String,
}

pub struct ConfigManager {
    pub config_dir: PathBuf,
    pub settings: AppSettings,
}

impl ConfigManager {
    /// 載入或在首次啟動時建立設定檔
    pub fn load_or_create() -> Self {
        // 取得跨系統標準設定目錄
        // Mac: ~/Library/Application Support/yt-dlp-tui_config
        // Linux: ~/.config/yt-dlp-tui_config
        // Win: AppData\Roaming\yt-dlp-tui_config
        let proj_dirs =
            ProjectDirs::from("", "", "yt-dlp-tui_config").expect("無法取得系統的設定檔目錄");
        let config_dir = proj_dirs.config_dir().to_path_buf();

        let config_file = config_dir.join("config.toml");

        // 若目錄不存在，建立它（包含未來的 .tmp 資料夾）
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).expect("無法建立設定檔目錄");
            fs::create_dir_all(config_dir.join(".tmp")).expect("無法建立暫存檔目錄");
        }

        // 預設設定內容
        let default_settings = AppSettings {
            version: "0.3.0-beta.9".to_string(),
            download_dir: "".to_string(), // 預設為空，後續動態解析
        };

        let settings = if config_file.exists() {
            // 讀取並解析現有的 toml
            let contents = fs::read_to_string(&config_file).expect("無法讀取設定檔");
            toml::from_str(&contents).unwrap_or(default_settings)
        } else {
            // 首次執行：寫入預設設定檔
            let toml_string = toml::to_string(&default_settings).unwrap();
            fs::write(&config_file, toml_string).expect("無法寫入初始設定檔");
            default_settings
        };

        ConfigManager {
            config_dir,
            settings,
        }
    }

    /// 取得最終的下載路徑（若設定為空，則 fallback 到系統下載資料夾）
    pub fn get_final_download_dir(&self) -> String {
        if self.settings.download_dir.trim().is_empty() {
            if let Some(user_dirs) = UserDirs::new() {
                if let Some(dl_dir) = user_dirs.download_dir() {
                    return dl_dir.to_string_lossy().to_string();
                }
            }
            // 極端情況 fallback 到當前目錄
            ".".to_string()
        } else {
            self.settings.download_dir.clone()
        }
    }
}
