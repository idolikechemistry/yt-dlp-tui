use chrono::Local;
use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf; // 引入 chrono 處理本地時間

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
    pub fn load_or_create() -> Self {
        let proj_dirs =
            ProjectDirs::from("", "", "yt-dlp-tui_config").expect("無法取得系統的設定檔目錄");
        let config_dir = proj_dirs.config_dir().to_path_buf();
        let config_file = config_dir.join("config.toml");

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).expect("無法建立設定檔目錄");
            fs::create_dir_all(config_dir.join(".tmp")).expect("無法建立暫存檔目錄");
        }

        let default_settings = AppSettings {
            version: "0.3.0-beta.9".to_string(),
            download_dir: "".to_string(),
        };

        let settings = if config_file.exists() {
            let contents = fs::read_to_string(&config_file).expect("無法讀取設定檔");
            toml::from_str(&contents).unwrap_or(default_settings)
        } else {
            let toml_string = toml::to_string(&default_settings).unwrap();
            fs::write(&config_file, toml_string).expect("無法寫入初始設定檔");
            default_settings
        };

        ConfigManager {
            config_dir,
            settings,
        }
    }

    pub fn get_final_download_dir(&self) -> String {
        if self.settings.download_dir.trim().is_empty() {
            if let Some(user_dirs) = UserDirs::new() {
                if let Some(dl_dir) = user_dirs.download_dir() {
                    return dl_dir.to_string_lossy().to_string();
                }
            }
            ".".to_string()
        } else {
            self.settings.download_dir.clone()
        }
    }

    /// 建立隔離的任務暫存資料夾 (YYYYMMDD_HHMMSS + PID + Index)
    pub fn create_isolated_tmp_dir(&self, task_idx: usize) -> std::io::Result<PathBuf> {
        // 1. 取得秒級本地時間戳記
        let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();

        // 2. 取得當前執行程式的進程 PID
        let pid = std::process::id();

        // 3. 組裝唯一的暫存目錄名稱
        let folder_name = format!("{} *pid{}* {}", ts, pid, task_idx);
        let session_tmp_dir = self.config_dir.join(".tmp").join(folder_name);

        // 4. 遞迴建立實體資料夾
        fs::create_dir_all(&session_tmp_dir)?;

        Ok(session_tmp_dir)
    }
}
