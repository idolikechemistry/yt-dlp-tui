mod args;
mod config;
mod parser;
mod proc;
mod setup;
mod ui;
mod utils;

use anyhow::{Context, Result};
use args::Args;
use clap::{CommandFactory, Parser};
use proc::LogManager;
use std::path::PathBuf;
use std::process;

#[tokio::main]
async fn main() {
    println!("🚀 dl-media v{}", env!("CARGO_PKG_VERSION"));
    if let Err(e) = run().await {
        eprintln!("\n❌ [執行錯誤]: {}", e);
        for cause in e.chain().skip(1) {
            eprintln!("  原因: {}", cause);
        }
        process::exit(1);
    }
}

/// 互動式配置下載偏好（選擇語言與解析度規格）
fn setup_download_options(
    videos: &mut Vec<parser::VideoItem>,
    cookie_args: &[String],
    is_silent: bool,
    media_type: u8,
    target_ext: &str,
) {
    if is_silent {
        return;
    }
    for video in videos.iter_mut() {
        println!("⏳ 正在獲取 {} 的可選設定...", video.title);
        if let Ok(info) = parser::probe_video_info(&video.url, cookie_args) {
            video.chosen_langs = ui::select_subtitles(&info.langs);
            if media_type != 1 && target_ext == "mkv" {
                video.chosen_format = ui::select_resolution(&info.formats);
            }
            video.metadata = Some(info);
        } else {
            println!("⚠️ 無法獲取 {} 的進階資訊，將使用預設參數。", video.title);
        }
    }
}

async fn run() -> Result<()> {
    // 1. 命令列參數解析
    let args = Args::parse();
    if let Some(generator) = args.generator {
        let mut cmd = Args::command();
        let name = cmd.get_name().to_string();
        clap_complete::generate(generator, &mut cmd, name, &mut std::io::stdout());
        return Ok(());
    }
    args.validate()?;

    // 2. 初始化設定環境並載入偏好 config.toml
    let (app_config_dir, config) = setup::init_config()?;
    let config_file_path = app_config_dir.join("config.toml");
    if args.config {
        setup::interactive_config_setup(&config_file_path, config)?;
        println!("👋 設定已完成，您可以重新執行程式來套用新設定。");
        return Ok(());
    }

    // 3. 系統核心依賴工具檢查
    setup::check_dependencies()?;

    // 4. 定義最終儲存與暫存目錄
    let final_download_dir = args
        .output
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| {
            if config.download_dir.is_empty() {
                None
            } else {
                Some(PathBuf::from(&config.download_dir))
            }
        })
        .unwrap_or_else(|| dirs::download_dir().expect("找不到系統下載目錄"));

    let final_tmp_dir = app_config_dir.join(".tmp");
    let resolved_cookie_dir = if config.cookie_dir.is_empty() {
        app_config_dir.clone()
    } else {
        PathBuf::from(&config.cookie_dir)
    };

    // 5. 判斷自動化靜默下載 vs 互動選單輸入
    let is_silent = args.is_fully_automated();
    let (input_urls, media_type, target_ext) = if is_silent {
        (
            args.url.clone().unwrap(),
            args.media_type.unwrap() as u8,
            args.format.clone().unwrap().to_lowercase(),
        )
    } else {
        ui::get_user_input(&args).context("無法取得使用者輸入")?
    };

    // 6. 循環處理每一個輸入的影片網址
    for input_url in input_urls {
        println!("\n▶️ 開始處理網址: {}", input_url);
        let site_target = parser::extract_site_name(&input_url);
        
        // 探測與獲取公開或受限內容
        let (mut valid_videos, is_playlist, has_restricted) =
            parser::scan_url(&input_url, args.force_cookie, &site_target)?;
            
        // 優先匹配對應平台的專屬沙盒 Cookie
        let cookie_args = setup::handle_cookies(
            &site_target,
            has_restricted,
            &args.cookie,
            &resolved_cookie_dir,
            is_silent,
        )?;
        
        if !cookie_args.is_empty() && is_playlist {
            valid_videos = parser::rescan_with_cookies(&input_url, &cookie_args, valid_videos.len())?;
        }

        let final_target_dir =
            utils::prepare_output_dir(&final_download_dir, &input_url, &cookie_args, is_playlist);

        // 建立 Markdown 執行日誌
        LogManager::init_log(&final_target_dir, &input_url);
        LogManager::log_event(
            &final_target_dir,
            "INFO",
            &format!("掃描完畢，準備下載 {} 個項目", valid_videos.len()),
        );

        let dl_args = utils::build_download_args(media_type, &target_ext, &input_url, &cookie_args);
        setup_download_options(
            &mut valid_videos,
            &cookie_args,
            is_silent,
            media_type,
            &target_ext,
        );

        let mut current_cookie_args = cookie_args;
        let mut session = proc::DownloadSession {
            pending_videos: valid_videos,
            failed_videos: Vec::new(),
        };

        // 🌟 初始化動態排除黑名單與重試安全防禦控制
        let mut attempted_browsers: Vec<String> = Vec::new();
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 3; // 實體防死鎖臨界上限

        // 7. 啟動非同步並行下載與錯誤恢復重試迴圈
        loop {
            if session.pending_videos.is_empty() {
                break;
            }
            if retry_count >= MAX_RETRIES {
                println!("\n⚠️ 已達到下載重試上限 ({} 次)，自動終止任務以防止死鎖與網絡資源鎖定。", MAX_RETRIES);
                LogManager::log_event(
                    &final_target_dir,
                    "ERROR",
                    &format!("已達到下載重試上限 ({} 次)，自動終止任務", MAX_RETRIES),
                );
                break;
            }

            // 執行當前 Session 的下載與 ffmpeg 封裝
            let failed_tasks = proc::execute_download_session(
                session,
                is_playlist,
                media_type,
                target_ext.clone(),
                dl_args.clone(),
                current_cookie_args.clone(),
                final_target_dir.clone(),
                final_tmp_dir.clone(),
                config.max_concurrent_downloads,
            )
            .await?;

            if failed_tasks.is_empty() {
                break; // 下載全部成功，安全退出
            }

            // 提取下載失敗的 Video 項目
            let failed_videos: Vec<parser::VideoItem> = failed_tasks.iter().map(|t| t.video.clone()).collect();
            
            // 檢查是否包含因登入/權限/年齡限制引起的 AuthError
            let has_auth_error = failed_tasks.iter().any(|t| {
                matches!(t.error, Some(proc::DLMediaError::AuthError(_)))
            });

            LogManager::log_event(
                &final_target_dir,
                "WARN",
                &format!(
                    "本次批次下載未完全成功，共 {} 個項目下載失敗 (是否存在權限問題: {})",
                    failed_tasks.len(),
                    has_auth_error
                ),
            );

            // 在自動化/靜默下載模式下，不進行交互式恢復
            if is_silent {
                println!("⚠️ 全自動靜默模式下發現失敗項目，拒絕彈出選單，中斷流程。");
                break;
            }

            // 8. 進入精準錯誤攔截恢復 TUI 選單
            if has_auth_error {
                retry_count += 1;
                match ui::prompt_error_recovery(failed_tasks.len()) {
                    ui::ErrorRecoveryChoice::Browser => {
                        // 載入自訂瀏覽器配置列表
                        let mut active_browsers = config.preferred_browsers.clone();
                        
                        // 跨平台過濾：Safari 只在 macOS 可用
                        #[cfg(not(target_os = "macos"))]
                        active_browsers.retain(|b| b != "safari");

                        // 核心防禦：動態排除已嘗試失敗的瀏覽器 (黑名單排除)
                        active_browsers.retain(|b| !attempted_browsers.contains(b));

                        if active_browsers.is_empty() {
                            println!("\n❌ 您的配置列表內所有可用瀏覽器的 Cookie 皆已嘗試且全部失效。");
                            LogManager::log_event(
                                &final_target_dir,
                                "WARN",
                                "自訂瀏覽器列表皆已嘗試失效，安全引導降級至手動 Cookie 匯入"
                            );
                            
                            // 安全降級引導：手動放入 cookie_site.txt 檔案
                            let new_cookie = setup::wait_for_manual_cookie(&resolved_cookie_dir, &site_target)?;
                            if new_cookie.is_empty() {
                                LogManager::log_event(&final_target_dir, "WARN", "使用者未提供有效手動 Cookie，放棄重試");
                                break;
                            }
                            current_cookie_args = new_cookie;
                        } else if active_browsers.len() == 1 {
                            // 🌟 一鍵自動繞過：若僅配置或僅剩一個瀏覽器，自動跳過 TUI 選擇直接套用
                            let single_browser = active_browsers[0].clone();
                            println!("\n⚡ 偵測到可用的瀏覽器列表僅剩一組，自動套用 [{}] 瀏覽器 Cookie 進行重試...", single_browser);
                            LogManager::log_event(
                                &final_target_dir,
                                "INFO",
                                &format!("瀏覽器僅剩一組，自動繞過選單套用 {}", single_browser)
                            );
                            attempted_browsers.push(single_browser.clone());
                            current_cookie_args = vec!["--cookies-from-browser".into(), single_browser];
                        } else {
                            // 彈出經過排除過濾後的動態瀏覽器選單
                            let browser = ui::select_browser(&active_browsers);
                            LogManager::log_event(
                                &final_target_dir,
                                "INFO",
                                &format!("使用者手動選擇：自動套用 {} 瀏覽器 Cookie 進行重試", browser),
                            );
                            attempted_browsers.push(browser.clone());
                            current_cookie_args = vec!["--cookies-from-browser".into(), browser];
                        }

                        // 重組失敗項目為 pending 並刷新 Session 重啟
                        session = proc::DownloadSession {
                            pending_videos: failed_videos,
                            failed_videos: Vec::new(),
                        };
                        println!("🔄 正在套用新 Cookie 重新嘗試下載...");
                    }
                    ui::ErrorRecoveryChoice::Manual => {
                        let new_cookie = setup::wait_for_manual_cookie(&resolved_cookie_dir, &site_target)?;
                        if new_cookie.is_empty() {
                            LogManager::log_event(&final_target_dir, "WARN", "使用者未提供有效手動 Cookie，終止重試");
                            println!("❌ 未提供有效的 Cookie，放棄重試。");
                            break;
                        }
                        current_cookie_args = new_cookie;
                        LogManager::log_event(&final_target_dir, "INFO", "使用者選擇：手動匯入 Cookie 進行重試");
                        
                        session = proc::DownloadSession {
                            pending_videos: failed_videos,
                            failed_videos: Vec::new(),
                        };
                        println!("🔄 正在使用手動 Cookie 重新嘗試下載...");
                    }
                    ui::ErrorRecoveryChoice::Abort => {
                        LogManager::log_event(&final_target_dir, "INFO", "使用者選擇：放棄失敗項目並結束");
                        println!("👋 已放棄失敗項目。");
                        break;
                    }
                }
            } else {
                // 原地重新嘗試：非 Auth 類引起的網路斷連波動，允許直接進行原參數重試
                retry_count += 1;
                println!("\n⚠️ 偵測到非權限引起的網路錯誤（如 Socket 逾時），自動進行原地重新嘗試 ({}/{})", retry_count, MAX_RETRIES);
                LogManager::log_event(
                    &final_target_dir,
                    "INFO",
                    &format!("原地重新下載重試 ({}/{})", retry_count, MAX_RETRIES)
                );
                session = proc::DownloadSession {
                    pending_videos: failed_videos,
                    failed_videos: Vec::new(),
                };
            }
        }
    }
    Ok(())
}
