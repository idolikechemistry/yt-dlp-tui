use crate::config::Config;
use anyhow::{Context, Result};
use dirs::config_dir;
use inquire::{Confirm, CustomType, Select, Text};
use std::env::consts::OS;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 1. 檢查系統環境是否具備必要工具 (整合跨平台友善安裝指引)
pub fn check_dependencies() -> Result<()> {
    let deps = [
        ("yt-dlp", "https://github.com/yt-dlp/yt-dlp#installation"),
        ("ffmpeg", "https://ffmpeg.org/download.html"),
        ("ffprobe", "https://ffmpeg.org/download.html"),
        ("danmaku2ass", "https://github.com/m13253/danmaku2ass"),
    ];

    let mut missing = Vec::new();
    for (dep, url) in deps {
        // 同時檢查 --version 與 -h 以確保工具確實可調用
        if Command::new(dep)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
            && Command::new(dep)
                .arg("-h")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_err()
        {
            missing.push((dep, url));
        }
    }

    if !missing.is_empty() {
        let mut error_msg = String::from("\n 偵測到系統缺少必要依賴組件，請先安裝以下工具：\n\n");
        for (name, url) in &missing {
            error_msg.push_str(&format!("  [{}]\n  線上線索：{}\n", name, url));

            // 依據作業系統提供最直覺的終端機安裝命令建議
            error_msg.push_str("  快速安裝指引：\n");
            match *name {
                "yt-dlp" => match OS {
                    "macos" => error_msg.push_str("      Mac 命令：brew install yt-dlp\n"),
                    "windows" => error_msg.push_str("      Windows (PowerShell)：pip install yt-dlp 或使用 winget install yt-dlp\n"),
                    _ => error_msg.push_str("      Linux (Ubuntu/Debian)：sudo apt install yt-dlp 或 pip3 install yt-dlp\n"),
                },
                "ffmpeg" | "ffprobe" => match OS {
                    "macos" => error_msg.push_str("      Mac 命令：brew install ffmpeg\n"),
                    "windows" => error_msg.push_str("      Windows (PowerShell)：winget install Gyan.FFmpeg 或至官方下載二進位檔加入 PATH\n"),
                    _ => error_msg.push_str("      Linux (Ubuntu/Debian)：sudo apt install ffmpeg\n"),
                },
                "danmaku2ass" => match OS {
                    "macos" => error_msg.push_str("      Mac 命令：brew install danmaku2ass 或 pip3 install danmaku2ass\n"),
                    "windows" => error_msg.push_str("      Windows (PowerShell)：pip install danmaku2ass\n"),
                    _ => error_msg.push_str("      Linux：pip3 install danmaku2ass\n"),
                },
                _ => {}
            }
            error_msg.push_str("--------------------------------------------------\n");
        }
        anyhow::bail!(error_msg);
    }

    Ok(())
}

/// 2. 初始化設定環境：建立 App 資料夾並載入設定檔
pub fn init_config() -> Result<(PathBuf, Config)> {
    let mut path = config_dir().context("❌ 無法取得系統設定目錄")?;
    path.push("yt-dlp-tui");

    if !path.exists() {
        fs::create_dir_all(&path).context("❌ 無法建立應用程式設定資料夾")?;
        // 建立隔離暫存總目錄
        fs::create_dir_all(path.join(".tmp")).context("❌ 無法建立暫存檔總目錄")?;
    }

    let config_file = path.join("config.toml");
    // 呼叫 Config 結構自帶的 load，由其決定要「初次生成」或「讀取升級」
    let config_data = Config::load(&config_file)?;

    Ok((path, config_data))
}

/// 3. 互動式設定引導 (TUI)：全面改寫為 inquire，支持拖曳路徑與防護設定
pub fn interactive_config_setup(config_path: &Path, mut config: Config) -> Result<()> {
    loop {
        let dl_dir_display = if config.download_dir.is_empty() {
            "預設 (下載資料夾 Downloads)"
        } else {
            &config.download_dir
        };
        let ck_dir_display = if config.cookie_dir.is_empty() {
            "預設 (App 設定夾)"
        } else {
            &config.cookie_dir
        };

        // 自訂瀏覽器列表字串化顯示
        let browsers_display = config.preferred_browsers.join(", ");

        let options = vec![
            format!("下載存檔路徑 [目前: {}]", dl_dir_display),
            format!("Cookie 存放路徑 [目前: {}]", ck_dir_display),
            format!("最大並行下載數 [目前: {}]", config.max_concurrent_downloads),
            format!("慣用瀏覽器列表 [目前: {}]", browsers_display),
            "儲存並完成退出".to_string(),
        ];

        let selection = Select::new(
            "=== yt-dlp-tui 偏好設定引導 (請使用上下鍵選擇項目) === ",
            options,
        )
        .prompt()
        .unwrap_or_else(|_| "儲存並完成退出".to_string());

        if selection == "儲存並完成退出" {
            break;
        }

        if selection.contains("下載存檔路徑") {
            println!("\n操作指引：");
            println!("  1. 系統即將為您開啟該設定資料夾。");
            println!("  2. 請將您想要指定的「下載資料夾」從檔案總管「拖曳」進此終端機視窗中，並按下 Enter。\n");

            let _ = open_folder(&config_path.parent().unwrap().to_path_buf());
            let input_path = Text::new("請拖入下載路徑：").prompt().unwrap_or_default();

            let cleaned_path = clean_dropped_path(&input_path);
            if !cleaned_path.is_empty() {
                config.download_dir = cleaned_path;
            }
        } else if selection.contains("Cookie 目錄") {
            println!("\n 操作指引：");
            println!("  1. 系統即將為您開啟該設定資料夾。");
            println!(
                "  2. 請將存放 cookie_site.txt 的資料夾「拖曳」進此終端機視窗中，並按下 Enter。\n"
            );

            let _ = open_folder(&config_path.parent().unwrap().to_path_buf());
            let input_path = Text::new("請拖入 Cookie 目錄路徑：")
                .prompt()
                .unwrap_or_default();

            let cleaned_path = clean_dropped_path(&input_path);
            if !cleaned_path.is_empty() {
                config.cookie_dir = cleaned_path;
            }
        } else if selection.contains("⚡") {
            println!("\n【並行防護警告】");
            println!("  並行下載數設置過高（建議保持在 1-5 之間）將有極高風險觸發 YouTube ");
            println!("  或 Bilibili 等影音伺服器的頻寬流量清洗防護（DDoS Block），導致您的 IP 被暫時封鎖！\n");

            let input_num = CustomType::<u32>::new("⚡ 請輸入新的最大並行任務數：")
                .with_default(config.max_concurrent_downloads)
                .with_error_message("❌ 請輸入有效的正整數！")
                .prompt()
                .unwrap_or(config.max_concurrent_downloads);

            config.max_concurrent_downloads = input_num;
        } else if selection.contains("慣用瀏覽器自訂說明") {
            println!("\n 慣用瀏覽器自訂說明：");
            println!("  當下載受限內容時，系統會自動在您指定的瀏覽器內抓取登入狀態（Cookie）。");
            println!("  如果僅填寫單一瀏覽器（例如: chrome），遇到 Cookie 限制時系統會「完全背景自動套用、零按鍵干預」。");
            println!("  請輸入小寫瀏覽器名稱，並以「英文逗號」隔開，例如：chrome, edge, firefox\n");

            let current_input = config.preferred_browsers.join(", ");
            let input_browsers = Text::new("請輸入慣用瀏覽器清單：")
                .with_default(&current_input)
                .prompt()
                .unwrap_or(current_input);

            let cleaned_list: Vec<String> = input_browsers
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();

            if !cleaned_list.is_empty() {
                config.preferred_browsers = cleaned_list;
            }
        }

        // 每一步修改完畢即時儲存至硬碟防遺失
        config.save(config_path).context("❌ 儲存設定失敗")?;
        println!("變更已成功套用！\n");
    }

    Ok(())
}

/// 4. 輔助函式：開啟系統檔案總管 (跨平台支援)
pub fn open_folder(path: &PathBuf) -> Result<()> {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(path).status();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg(path).status();
    #[cfg(target_os = "linux")]
    let _ = Command::new("xdg-open").arg(path).status();
    Ok(())
}

/// 5. 處理 Cookie 載入邏輯 (第一防線：匹配沙盒專用 cookie_site.txt 實體檔案)
pub fn handle_cookies(
    site_target: &str,
    has_restricted: bool,
    manual_cookie: &Option<String>,
    resolved_cookie_dir: &PathBuf,
    is_silent: bool,
) -> Result<Vec<String>> {
    let mut cookie_args = Vec::new();

    // 優先權 1：使用者透過命令列 `-c` / `--cookie` 手動指定的檔案
    if let Some(manual_path_str) = manual_cookie {
        let path = PathBuf::from(manual_path_str);
        if path.exists() {
            cookie_args.push("--cookies".to_string());
            cookie_args.push(path.to_string_lossy().to_string());
            println!("已套用命令列自訂 Cookie：{}", path.display());
            return Ok(cookie_args);
        }
    }

    // 優先權 2：設定目錄下放置的 cookie_[網站].txt (例如 cookie_youtube.txt)
    let expected_filename = format!("cookie_{}.txt", site_target);
    let target_file = resolved_cookie_dir.join(&expected_filename);

    if target_file.exists() {
        cookie_args.push("--cookies".to_string());
        cookie_args.push(target_file.to_string_lossy().to_string());
        println!("偵測並成功加載 {} 專屬實體 Cookie 檔案", site_target);
    } else if has_restricted {
        // 沒有實體檔，但偵測到為受限內容
        println!("\n本影片為限制級/私有內容，需要登入權限才能下載。");
        println!(
            "未在設定夾偵測到 {} 的專屬 Cookie 檔案 ({})",
            site_target, expected_filename
        );

        let want_to_wait = if is_silent {
            false
        } else {
            Confirm::new("是否要現在開啟設定目錄，以便您放入實體 Cookie 檔案？")
                .with_default(true)
                .prompt()
                .unwrap_or(false)
        };

        if want_to_wait {
            open_folder(resolved_cookie_dir)?;
            println!("\n請將匯出的 {} 放入剛開啟的資料夾中...", expected_filename);
            println!("放置完成後，請在本視窗按下 [Enter] 鍵繼續...");

            let mut _pause = String::new();
            io::stdin().read_line(&mut _pause)?;

            if target_file.exists() {
                println!("偵測到 Cookie 檔案！已成功導入。");
                cookie_args.push("--cookies".to_string());
                cookie_args.push(target_file.to_string_lossy().to_string());
            } else {
                println!("仍未發現 Cookie 檔案，系統將嘗試以「無登入狀態」繼續（可能隨後失敗）。");
            }
        }
    }

    Ok(cookie_args)
}

/// 6. 錯誤恢復：引導手動實體 Cookie 補入等待
pub fn wait_for_manual_cookie(
    resolved_cookie_dir: &PathBuf,
    site_target: &str,
) -> Result<Vec<String>> {
    let expected_filename = format!("cookie_{}.txt", site_target);
    let target_file = resolved_cookie_dir.join(&expected_filename);

    open_folder(resolved_cookie_dir)?;
    println!(
        "\n請將您的限制破解 Cookie 檔案重新命名為「{}」並放入剛開啟的資料夾中...",
        expected_filename
    );
    println!("放入完成後，請在本視窗按下 [Enter] 鍵重試任務...");

    let mut _pause = String::new();
    io::stdin().read_line(&mut _pause)?;

    let mut cookie_args = Vec::new();
    if target_file.exists() {
        println!("順利檢測到補入的實體 Cookie！已套用。");
        cookie_args.push("--cookies".to_string());
        cookie_args.push(target_file.to_string_lossy().to_string());
    } else {
        println!("仍未檢測到實體 Cookie 檔案，放棄此次重試。");
    }

    Ok(cookie_args)
}

/// 輔助清洗拖曳進終端機的資料夾路徑字串
fn clean_dropped_path(input: &str) -> String {
    input
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .replace("\\ ", " ")
}
