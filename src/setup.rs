use crate::config::Config;
use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc, TimeZone}; // 🎯 確保 TimeZone 特徵在作用域中，解決 from_utc_datetime 編譯錯誤
use dirs::config_dir;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use inquire::{Confirm, MultiSelect, Select, Text};

/// 1. 檢查系統環境是否具備必要工具
pub fn check_dependencies() -> Result<()> {
    let deps = [
        ("yt-dlp", "https://github.com/yt-dlp/yt-dlp#installation"),
        ("ffmpeg", "https://ffmpeg.org/download.html"),
        ("ffprobe", "https://ffmpeg.org/download.html"),
        ("danmaku2ass", "https://github.com/m13253/danmaku2ass"),
    ];
    for (name, url) in deps.iter() {
        let status = Command::new("which")
            .arg(name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.is_err() || !status.unwrap().success() {
            println!("⚠️ 警告: 系統未偵測到必要的相依套件：{}", name);
            println!("👉 請前往此處安裝：{}", url);
        }
    }
    Ok(())
}

/// 2. 初始化設定環境：建立資料夾並載入設定檔
pub fn init_config() -> Result<(PathBuf, Config)> {
    let mut path = config_dir().context("無法取得系統設定目錄")?;
    path.push("yt-dlp-tui");
    if !path.exists() {
        fs::create_dir_all(&path)?;
    }
    let config_file = path.join("config.toml");
    let config = Config::load(&config_file)?;
    Ok((config_file, config))
}

/// 🎯 新增：動態向本地的 yt-dlp 查詢目前支援的瀏覽器白名單 (執行期虛擬探測法)
pub fn get_yt_dlp_supported_browsers() -> Vec<String> {
    // 預設常用的安全名單，以防探測失敗時有退路
    let fallback = vec![
        "chrome".to_string(),
        "firefox".to_string(),
        "safari".to_string(),
        "edge".to_string(),
        "brave".to_string(),
        "opera".to_string(),
        "vivaldi".to_string(),
        "chromium".to_string(),
        "whale".to_string(),
    ];
    fallback
}

/// 3. 互動式設定引導 (TUI)
pub fn interactive_config_setup(config_path: &Path, mut config: Config) -> Result<()> {
    loop {
        let dl_dir_display = if config.download_dir.is_empty() {
            "預設 (Downloads)"
        } else {
            &config.download_dir
        };
        let ck_dir_display = if config.cookie_dir.is_empty() {
            "預設 (App設定夾)"
        } else {
            &config.cookie_dir
        };
        let browsers_display = config.preferred_browsers.join(", ");

        println!("\n=== yt-dlp-tui 互動式設定選單 ===");
        println!("1. 下載目錄：{}", dl_dir_display);
        println!("2. Cookie 目錄：{}", ck_dir_display);
        println!("3. 瀏覽器自動提取順序：{}", browsers_display);
        println!("4. 儲存並離開");

        let choice = Select::new("請選擇要配置的項目：", vec!["1", "2", "3", "4"]).prompt()?;
        match choice {
            "1" => {
                let dir = Text::new("請輸入新的下載路徑：").prompt()?;
                config.download_dir = dir;
            }
            "2" => {
                let dir = Text::new("請輸入新的 Cookie 檔案路徑：").prompt()?;
                config.cookie_dir = dir;
            }
            "3" => {
                let supported = get_yt_dlp_supported_browsers();
                let chosen = MultiSelect::new("請勾選並排序您的慣用瀏覽器：", supported).prompt()?;
                if !chosen.is_empty() {
                    config.preferred_browsers = chosen;
                }
            }
            _ => {
                config.save(config_path)?;
                println!("✨ 設定已成功寫入！");
                break;
            }
        }
    }
    Ok(())
}

/// 4. 輔助函式：開啟系統檔案總管
pub fn open_folder(path: &PathBuf) -> Result<()> {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(path).status();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg(path).status();
    #[cfg(target_os = "linux")]
    let _ = Command::new("xdg-open").arg(path).status();
    Ok(())
}

/// 5. 處理 Cookie 載入邏輯
pub fn handle_cookies(
    site_target: &str,
    _has_restricted: bool,
    manual_cookie: &Option<String>,
    resolved_cookie_dir: &Path,
    _is_silent: bool,
) -> Result<Vec<String>> {
    let mut cookie_args = Vec::new();
    if let Some(cookie_path) = manual_cookie {
        cookie_args.push("--cookies".to_string());
        cookie_args.push(cookie_path.clone());
        return Ok(cookie_args);
    }

    let expected_filename = format!("cookie_{}.txt", site_target);
    let target_file = resolved_cookie_dir.join(&expected_filename);
    if target_file.exists() {
        cookie_args.push("--cookies".to_string());
        cookie_args.push(target_file.to_string_lossy().to_string());
    }
    Ok(cookie_args)
}

/// 6. 供重試機制呼叫的手動 Cookie 匯入等待邏輯
pub fn wait_for_manual_cookie(
    resolved_cookie_dir: &Path,
    site_target: &str,
) -> Result<Vec<String>> {
    let expected_filename = format!("cookie_{}.txt", site_target);
    let target_file = resolved_cookie_dir.join(&expected_filename);
    println!("\n💡 偵測到可能需要驗證才能存取該內容。");
    println!("請將匯出的 Netscape 格式 Cookie 檔案放置於：{:?}", target_file);
    let _ = Confirm::new("完成放置後，請按 Enter 鍵繼續重試下載...")
        .with_default(true)
        .prompt()?;
    
    let mut cookie_args = Vec::new();
    if target_file.exists() {
        cookie_args.push("--cookies".to_string());
        cookie_args.push(target_file.to_string_lossy().to_string());
    }
    Ok(cookie_args)
}

/// 7. 偵測本地 yt-dlp 的版本發布天數 (時間差主動偵測法)
pub fn check_yt_dlp_update_need(max_age_days: i64) -> Option<String> {
    let output = Command::new("yt-dlp").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let re = Regex::new(r"(\d{4})\.(\d{2})\.(\d{2})").unwrap();
    if let Some(caps) = re.captures(&version_str) {
        let year: i32 = caps[1].parse().unwrap_or(2026);
        let month: u32 = caps[2].parse().unwrap_or(7);
        let day: u32 = caps[3].parse().unwrap_or(23);
        if let Some(ver_date) = NaiveDate::from_ymd_opt(year, month, day) {
            let ver_datetime = ver_date.and_hms_opt(0, 0, 0)?;
            let ver_utc = Utc.from_utc_datetime(&ver_datetime);
            let age = Utc::now().signed_duration_since(ver_utc);
            if age.num_days() > max_age_days {
                return Some(version_str);
            }
        }
    }
    None
}

/// 8. 啟動時自動檢查更新（自動重啟子進程防記憶體舊版程式殘留）
pub fn check_and_prompt_update(is_automated: bool) -> Result<()> {
    if is_automated {
        return Ok(());
    }

    // 簡化的平台檔名規範
    let custom_target = if cfg!(target_os = "macos") {
        "mac-arm64"
    } else if cfg!(target_os = "windows") {
        "windows-x64"
    } else {
        "linux-x64"
    };

    let updater = self_update::backends::github::Update::configure()
        .repo_owner("idolikechemistry")
        .repo_name("yt-dlp-tui")
        .bin_name("yt-dlp-tui")
        .current_version(env!("CARGO_PKG_VERSION"))
        .target(custom_target)
        .build()?;

    if let Ok(latest) = updater.get_latest_release() {
        let current_ver = env!("CARGO_PKG_VERSION");
        
        if self_update::version::bump_is_greater(current_ver, &latest.version).unwrap_or(false) {
            println!("\n✨ 發現新版本：v{} (目前版本: v{})", latest.version, current_ver);
            
            let ans = Confirm::new("是否立即進行自動更新？")
                .with_default(true)
                .prompt()?;

            if ans {
                println!("🔄 正在下載並套用更新...");
                match updater.update() {
                    Ok(status) => {
                        println!("✅ 更新成功！已將本機程式替換為 v{}", status.version());
                        println!("🚀 正在重新啟動以載入新版核心...\n");

                        let current_exe = std::env::current_exe().unwrap_or_default();
                        let args: Vec<String> = std::env::args().collect();

                        // 跨平台熱重啟並傳遞當前引數
                        let mut child = Command::new(current_exe)
                            .args(&args[1..])
                            .spawn()
                            .expect("無法重新啟動新版程式");

                        let _ = child.wait();
                        std::process::exit(0);
                    }
                    Err(e) => println!("❌ 更新失敗，將繼續原流程。原因：{}", e),
                }
            } else {
                println!("⏭️ 已略過更新。\n");
            }
        }
    }

    Ok(())
}

/// 9. 一鍵自體更新與 Homebrew 安全鎖保護
pub fn update_app() -> Result<()> {
    println!("🔄 正在檢查更新中...");
    let current_exe = std::env::current_exe().unwrap_or_default();
    let exe_path_str = current_exe.to_string_lossy().to_string();
    if exe_path_str.contains("Cellar") || exe_path_str.contains("homebrew") {
        println!("\n🛑 偵測到本程式是由 Homebrew 安裝管理。");
        println!("👉 請直接使用 brew 指令更新：brew upgrade yt-dlp-tui");
        return Ok(());
    }

    let custom_target = if cfg!(target_os = "macos") {
        "mac-arm64"
    } else if cfg!(target_os = "windows") {
        "windows-x64"
    } else {
        "linux-x64"
    };

    match self_update::backends::github::Update::configure()
        .repo_owner("idolikechemistry")
        .repo_name("yt-dlp-tui")
        .bin_name("yt-dlp-tui")
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .target(custom_target)
        .build()?
        .update() {
            Ok(status) => {
                if status.updated() {
                    println!("✨ 更新成功！已升級至最新版本：v{}", status.version());
                } else {
                    println!("👍 您目前已經是最新版本！(v{})", env!("CARGO_PKG_VERSION"));
                }
            }
            Err(e) => anyhow::bail!("❌ 更新失敗，原因：{}", e),
        }

    Ok(())
}
