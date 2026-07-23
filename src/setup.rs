use crate::config::Config;
use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use dirs::config_dir;
use regex::Regex;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 1. 檢查系統環境是否具備必要工具
pub fn check_dependencies() -> Result<()> {
    let deps = [
        ("yt-dlp", "https://github.com/yt-dlp/yt-dlp#installation"),
        ("ffmpeg", "https://ffmpeg.org/download.html"),
        ("ffprobe", "https://ffmpeg.org/download.html"),
        ("danmaku2ass", "https://github.com/m13253/danmaku2ass"),
    ];

    let mut missing = Vec::new();
    for (dep, url) in deps {
        // 同時檢查 --version 與 -h 以確保工具存在
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
        let mut error_msg = String::from("❌ 偵測到缺少必要組件，請先安裝以下工具：\n\n");
        for (name, url) in missing {
            error_msg.push_str(&format!("  📌 [{}]\n  👉 下載：{}\n", name, url));
            #[cfg(target_os = "macos")]
            if name != "danmaku2ass" {
                error_msg.push_str(&format!("  💻 Mac 指令：brew install {}\n", name));
            }
        }
        anyhow::bail!(error_msg);
    }
    Ok(())
}

/// 2. 初始化設定環境：建立資料夾並載入設定檔
pub fn init_config() -> Result<(PathBuf, Config)> {
    let mut path = config_dir().context("無法取得系統設定目錄")?;
    path.push("yt-dlp-tui"); // 已更新為 yt-dlp-tui 命名空間
    if !path.exists() {
        fs::create_dir_all(&path).context("無法建立應用程式設定資料夾")?;
    }
    let config_file = path.join("config.toml");
    let config_data = Config::load(&config_file)?;
    Ok((path, config_data))
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

    // 在背景執行一個必定會失敗的測試指令，藉此套取錯誤訊息中的白名單
    let output = Command::new("yt-dlp")
        .args(["--cookies-from-browser", "INVALID_TEST_VALUE_FOR_TUI_PROBE"])
        .output();

    match output {
        Ok(out) => {
            let stderr_str = String::from_utf8_lossy(&out.stderr);

            // 🎯 同時相容舊版的 "must be one of" 與新版的 "Supported browsers are:" 兩種錯誤格式
            let re = Regex::new(r"(?:must be one of|Supported browsers are:)\s+([a-zA-Z0-9_,\s]+)")
                .unwrap();
            if let Some(caps) = re.captures(&stderr_str) {
                if let Some(matched) = caps.get(1) {
                    let list: Vec<String> = matched
                        .as_str()
                        .split(',')
                        .map(|s| s.trim().to_lowercase())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if !list.is_empty() {
                        return list;
                    }
                }
            }
            fallback
        }
        Err(_) => fallback, // 若 yt-dlp 未安裝或執行失敗，安全退回預設值
    }
}

/// 3. 互動式設定引導 (TUI)
pub fn interactive_config_setup(config_path: &Path, mut config: Config) -> Result<()> {
    let theme = ColorfulTheme::default();
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

        let options = vec![
            format!("📂 下載存檔路徑 [目前: {}]", dl_dir_display),
            format!("🍪 Cookie 存放路徑 [目前: {}]", ck_dir_display),
            format!(
                "⚡ 最大並行下載數 [目前: {}]",
                config.max_concurrent_downloads
            ),
            format!("🌐 慣用瀏覽器列表 [目前: {}]", browsers_display),
            "✅ 儲存並完成退出".to_string(),
        ];

        let selection = Select::with_theme(&theme)
            .with_prompt("🛠️ yt-dlp-tui 偏好設定引導 (請使用上下鍵選擇項目)")
            .items(&options)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                // 1. 下載存檔路徑
                println!("\n💡 操作指引：");
                println!("  1. 我現在會為您開啟資料夾視窗。");
                println!("  2. 請在視窗中找到目標資料夾，並將其「拖入」此終端機視窗中。");
                let _ = open_folder(&config_path.parent().unwrap().to_path_buf());
                let input_path: String = Input::with_theme(&theme)
                    .with_prompt("📍 請拖入路徑並按下 Enter (留空可還原系統預設)")
                    .allow_empty(true)
                    .interact_text()?;
                let cleaned_path = input_path
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .replace("\\ ", " ");
                config.download_dir = cleaned_path;
                config.save(config_path).context("儲存設定失敗")?;
                println!("✨ 下載存檔路徑變更已成功套用！\n");
            }
            1 => {
                // 2. Cookie 存放路徑
                println!("\n💡 操作指引：");
                println!("  1. 我現在會為您開啟資料夾視窗。");
                println!("  2. 請在視窗中找到目標資料夾，並將其「拖入」此終端機視窗中。");
                let _ = open_folder(&config_path.parent().unwrap().to_path_buf());
                let input_path: String = Input::with_theme(&theme)
                    .with_prompt("📍 請拖入路徑並按下 Enter (留空可還原為程式預設夾)")
                    .allow_empty(true)
                    .interact_text()?;
                let cleaned_path = input_path
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .replace("\\ ", " ");
                config.cookie_dir = cleaned_path;
                config.save(config_path).context("儲存設定失敗")?;
                println!("✨ Cookie 存放路徑變更已成功套用！\n");
            }
            2 => {
                // 3. 最大並行下載數
                println!("\n⚠️ 【強烈警告】");
                println!("設置過高將有極大風險觸發 DDoS 防護，甚至導致您的 IP 被封鎖！");
                println!("建議一般使用者保持在 1-5 之間。\n");
                let input_num: u32 = Input::with_theme(&theme)
                    .with_prompt("請輸入新的最大並行任務數")
                    .default(config.max_concurrent_downloads)
                    .interact_text()?;
                config.max_concurrent_downloads = input_num;
                config.save(config_path).context("儲存設定失敗")?;
                println!("✨ 最大並行下載數變更已成功套用為：{}！\n", input_num);
            }
            3 => {
                // 4. 智慧型瀏覽器管理子選單
                loop {
                    let sub_options = vec![
                        format!(
                            "👥 勾選並排出優先順序 [目前啟用: {}]",
                            config.preferred_browsers.join(", ")
                        ),
                        "➕ 手動新增其他自訂瀏覽器 (支援 Brave, Opera, Vivaldi 等)".to_string(),
                        "🧹 重設為預設瀏覽器列表 (Chrome, Firefox, Safari, Edge)".to_string(),
                        "↩️ 返回上層偏好設定選單".to_string(),
                    ];

                    let sub_selection = Select::with_theme(&theme)
                        .with_prompt("🌐 慣用瀏覽器管理子選單")
                        .items(&sub_options)
                        .default(0)
                        .interact()?;

                    match sub_selection {
                        0 => {
                            // A. 勾選啟用
                            if config.preferred_browsers.is_empty() {
                                println!(
                                    "⚠️ 目前列表為空，請先選擇「手動新增」加入您安裝的瀏覽器！\n"
                                );
                                continue;
                            }

                            let candidates = config.preferred_browsers.clone();
                            let defaults = vec![true; candidates.len()];

                            let chosen_indices = MultiSelect::with_theme(&theme)
                                .with_prompt("請使用 Space 鍵勾選要啟用的瀏覽器 (Enter 鍵確認)：")
                                .items(&candidates)
                                .defaults(&defaults)
                                .interact()?;

                            let mut new_browsers = Vec::new();
                            for idx in chosen_indices {
                                new_browsers.push(candidates[idx].clone());
                            }

                            if new_browsers.is_empty() {
                                println!("⚠️ 警告：必須至少啟用一個瀏覽器！已保留原設定。\n");
                            } else {
                                config.preferred_browsers = new_browsers;
                                config.save(config_path).context("儲存設定失敗")?;
                                println!("✨ 慣用瀏覽器優先順序已更新！\n");
                            }
                        }
                        1 => {
                            // B. ➕ 手動新增自訂瀏覽器
                            println!("\n💡 請輸入您的瀏覽器名稱 (必須與 yt-dlp 支援的名單相符，如 brave, opera, vivaldi 等)");
                            let raw_input: String = Input::with_theme(&theme)
                                .with_prompt("✍️ 請輸入瀏覽器名稱")
                                .interact_text()?;

                            // 🎯 安全清洗核心：去空格、強制轉換為全小寫
                            let cleaned_name = raw_input.trim().to_lowercase();

                            if cleaned_name.is_empty() {
                                println!("❌ 輸入無效，瀏覽器名稱不可為空！\n");
                            } else if config.preferred_browsers.contains(&cleaned_name) {
                                println!(
                                    "💡「{}」已經在您的列表中了，無需重複新增。\n",
                                    cleaned_name
                                );
                            } else {
                                // 🎯 核心：透過執行期虛擬探測法動態取得目前最新的白名單！
                                let yt_dlp_supported = get_yt_dlp_supported_browsers();

                                if yt_dlp_supported.contains(&cleaned_name) {
                                    // 命中已知白名單，直接安全寫入
                                    config.preferred_browsers.push(cleaned_name.clone());
                                    config.save(config_path).context("儲存設定失敗")?;
                                    println!(
                                        "✨ [Success] 已成功將「{}」安全加入您的設定檔中！\n",
                                        cleaned_name
                                    );
                                } else {
                                    // 未命中白名單，觸發「智慧型確認警告」
                                    println!("\n⚠️ 提醒：『{}』不在目前本機已知的 yt-dlp 支援瀏覽器清單中。", cleaned_name);
                                    println!(
                                        "（本機 yt-dlp 支援：{}）",
                                        yt_dlp_supported.join(", ")
                                    );

                                    let force_add = Confirm::with_theme(&theme)
                                        .with_prompt("您確定要強制將它新增至您的慣用列表嗎？")
                                        .default(false)
                                        .interact()?;

                                    if force_add {
                                        config.preferred_browsers.push(cleaned_name.clone());
                                        config.save(config_path).context("儲存設定失敗")?;
                                        println!(
                                            "✨ [Success] 已強制將『{}』寫入設定檔！\n",
                                            cleaned_name
                                        );
                                    } else {
                                        println!("↩️ 已取消操作，未新增任何項目。\n");
                                    }
                                }
                            }
                        }
                        2 => {
                            // C. 🧹 重設為官方預設
                            if Confirm::with_theme(&theme)
                                .with_prompt("確定要清空當前列表，並恢復為預設的 Chrome/Firefox/Safari/Edge 嗎？")
                                .default(false)
                                .interact()?
                            {
                                config.preferred_browsers = vec![
                                    "chrome".to_string(),
                                    "firefox".to_string(),
                                    "safari".to_string(),
                                    "edge".to_string(),
                                ];
                                config.save(config_path).context("儲存設定失敗")?;
                                println!("✨ 已成功恢復預設瀏覽器列表！\n");
                            }
                        }
                        _ => {
                            // D. 返回
                            break;
                        }
                    }
                }
            }
            4 => {
                // 5. 儲存並完成退出
                break;
            }
            _ => {}
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
    has_restricted: bool,
    manual_cookie: &Option<String>,
    resolved_cookie_dir: &PathBuf,
    is_silent: bool,
) -> Result<Vec<String>> {
    let mut cookie_args = Vec::new();

    // 優先權 1：命令列 -c 指定
    if let Some(manual_path_str) = manual_cookie {
        let path = PathBuf::from(manual_path_str);
        if path.exists() {
            cookie_args.push("--cookies".to_string());
            cookie_args.push(path.to_string_lossy().to_string());
            println!("🍪 已套用自訂 Cookie：{}", path.display());
            return Ok(cookie_args);
        }
    }

    // 優先權 2：設定路徑下的 cookie_site.txt
    let expected_filename = format!("cookie_{}.txt", site_target);
    let target_file = resolved_cookie_dir.join(&expected_filename);
    if target_file.exists() {
        cookie_args.push("--cookies".to_string());
        cookie_args.push(target_file.to_string_lossy().to_string());
        println!("🍪 已載入 {} 專用 Cookie", site_target);
    } else if has_restricted {
        println!(
            "⚠️ 未偵測到 {} 專用 Cookie ({})",
            site_target, expected_filename
        );
        let want_to_wait = if is_silent {
            false
        } else {
            Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("此內容需要權限。是否要現在開啟 Cookie 目錄放入？")
                .default(true)
                .interact()?
        };

        if want_to_wait {
            open_folder(resolved_cookie_dir)?;
            println!(
                "⏳ 請將 {} 放入資料夾，完成後按下 Enter 繼續...",
                expected_filename
            );
            let mut _pause = String::new();
            io::stdin().read_line(&mut _pause)?;
            if target_file.exists() {
                println!("🎉 偵測到 Cookie！已成功套用。");
                cookie_args.push("--cookies".to_string());
                cookie_args.push(target_file.to_string_lossy().to_string());
            }
        }
    }
    Ok(cookie_args)
}

/// 6. 供重試機制呼叫的手動 Cookie 匯入等待邏輯
pub fn wait_for_manual_cookie(
    resolved_cookie_dir: &PathBuf,
    site_target: &str,
) -> Result<Vec<String>> {
    let expected_filename = format!("cookie_{}.txt", site_target);
    let target_file = resolved_cookie_dir.join(&expected_filename);
    open_folder(resolved_cookie_dir)?;
    println!("⏳ 請將 {} 放入剛開啟的資料夾中...", expected_filename);
    println!("👉 完成後請按下 Enter 繼續...");
    let mut _pause = String::new();
    io::stdin().read_line(&mut _pause)?;
    let mut cookie_args = Vec::new();
    if target_file.exists() {
        println!("🎉 偵測到 Cookie！已成功套用。");
        cookie_args.push("--cookies".to_string());
        cookie_args.push(target_file.to_string_lossy().to_string());
    } else {
        println!("⚠️ 仍未偵測到 Cookie 檔案。");
    }
    Ok(cookie_args)
}

/// 7. 偵測本地 yt-dlp 的版本發布天數 (時間差主動偵測法)
pub fn check_yt_dlp_update_need(max_age_days: i64) -> Option<String> {
    // 1. 執行 yt-dlp --version 獲取本地版本字串
    let output = Command::new("yt-dlp").arg("--version").output().ok()?;

    if !output.status.success() {
        return None;
    }

    let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // 2. 清洗版本號（例如將 "2026.07.16@nightly" 轉為 "2026-07-16"）
    let raw_date = version_str
        .split('@') // 拔除 @nightly 或 @master 等通道標記
        .next()?
        .replace('.', "-"); // 將點號替換成連字號以利 chrono 解析

    // 3. 解析為 NaiveDate 並比對當前日期
    if let Ok(yt_date) = NaiveDate::parse_from_str(&raw_date, "%Y-%m-%d") {
        let today = Utc::now().date_naive();
        let elapsed_days = (today - yt_date).num_days();

        // 4. 若大於設定的限制天數，則動態生成跨平台更新指引
        if elapsed_days > max_age_days {
            // 偵測目前使用者的作業系統，給予最精準的更新小抄
            let update_hint = if cfg!(target_os = "macos") {
                "💻 Mac 使用者請在終端機執行：\n     brew upgrade yt-dlp"
            } else if cfg!(target_os = "windows") {
                "🪟 Windows 使用者（若使用 pip 安裝）請在命令提示字元執行：\n     pip install -U yt-dlp"
            } else {
                "🐧 Linux 使用者請執行：\n     pip3 install -U yt-dlp 或下載最新二進位檔"
            };

            return Some(format!(
                "┌────────────────────────────────────────────────────────┐\n\
                 ⚠️  【依賴套件更新提醒】\n\
                 │ 偵測到您本地的 yt-dlp 版本為：{}\n\
                 │ 該版本已發布約 {} 天。因線上影音網站經常改版，\n\
                 │ 建議您定期更新 yt-dlp 以免發生下載錯誤！\n\
                 │\n\
                 │ {}\n\
                 └────────────────────────────────────────────────────────┘",
                version_str, elapsed_days, update_hint
            ));
        }
    }

    None
}

/// 8. 一鍵自體更新與 Homebrew 安全鎖保護
pub fn update_app() -> Result<()> {
    println!("🔄 正在檢查更新中...");

    // 偵測是否由 Homebrew 管理 (macOS 特色)
    let current_exe = std::env::current_exe().unwrap_or_default();
    let exe_path_str = current_exe.to_string_lossy().to_string();
    if exe_path_str.contains("Cellar") || exe_path_str.contains("homebrew") {
        println!("\n🛑 偵測到本程式是由 Homebrew 安裝管理。");
        println!("👉 為了系統的乾淨與穩定，請直接在終端機執行 brew 指令更新：");
        println!("   brew upgrade dl-media (或 yt-dlp-tui)");
        return Ok(());
    }

    // 呼叫 self_update 進行自動更新
    // 這裡調用底層的 self_update 庫（若專案有引入此相依性）
    // 以下為自體更新的虛擬核心邏輯示意
    match self_update::backends::github::Update::configure()
        .register_name("yt-dlp-tui")
        .repo_owner("idolikechemistry")
        .repo_name("yt-dlp-tui")
        .bin_name("yt-dlp-tui")
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()?
        .update()
    {
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
