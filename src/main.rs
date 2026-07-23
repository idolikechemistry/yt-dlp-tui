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

/// 🎯 程式進入點
#[tokio::main]
async fn main() {
    println!("🚀 yt-dlp-tui v{}", env!("CARGO_PKG_VERSION"));
    if let Err(e) = run().await {
        eprintln!("\n❌ [執行錯誤]: {}", e);
        for cause in e.chain().skip(1) {
            eprintln!(" 原因: {}", cause);
        }
        process::exit(1);
    }
}

/// 🎯 互動式模式下，獲取影片的進階下載規格選項 (多國字幕選擇 & MKV 高畫質解析度選擇)
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

/// 🎯 核心控制流排程
async fn run() -> Result<()> {
    let args = Args::parse();

    // =====================================================================
    // 1. 優先攔截一鍵自體更新 --update 參數
    // =====================================================================
    if args.update {
        setup::update_app()?;
        return Ok(());
    }

    // =====================================================================
    // 2. 優先處理 Shell 自動補全腳本產生器
    // =====================================================================
    if let Some(generator) = args.generator {
        let mut cmd = Args::command();
        let name = cmd.get_name().to_string();
        clap_complete::generate(generator, &mut cmd, name, &mut std::io::stdout());
        return Ok(());
    }

    // =====================================================================
    // 3. 驗證命令行參數邏輯是否合規 (例如防止純音訊配置 mp4 輸出)
    // =====================================================================
    args.validate()?;

    // =====================================================================
    // 4. 初始化應用程式環境與設定檔
    // =====================================================================
    let (app_config_dir, config) = setup::init_config()?;
    let config_file_path = app_config_dir.join("config.toml");

    // =====================================================================
    // 5. 優先攔截 --config 偏好設定引導指令
    // =====================================================================
    if args.config {
        setup::interactive_config_setup(&config_file_path, config)?;
        println!("👋 設定已完成，您可以重新執行程式來套用新設定。");
        return Ok(());
    }

    // =====================================================================
    // 6. 執行系統關鍵依賴套件檢查 (yt-dlp, ffmpeg, ffprobe, danmaku2ass)
    // =====================================================================
    setup::check_dependencies()?;

    // =====================================================================
    // 7. 依賴套件過期安全提醒 (偵測本地 yt-dlp 年齡是否超過 30 天，若過期則印出醒目警告框)
    // =====================================================================
    if let Some(reminder_box) = setup::check_yt_dlp_update_need(30) {
        println!("\n{}\n", reminder_box);
    }

    // =====================================================================
    // 8. 決策最終下載存檔資料夾、任務暫存目錄與 Cookie 儲存目錄
    // =====================================================================
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

    // =====================================================================
    // 9. 決策雙模下載機制 (全自動模式 vs. 互動式 TUI 模式)
    // =====================================================================
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

    // =====================================================================
    // 10. 迭代處理每一個下載網址
    // =====================================================================
    for input_url in input_urls {
        println!("\n▶️ 開始處理網址: {}", input_url);
        let site_target = parser::extract_site_name(&input_url);

        // 🎯 智慧警報：如果是 Bilibili 影片下載，且使用者沒有傳入實體 Cookie 參數
        if site_target == "bilibili" && media_type != 1 && !is_silent && args.cookie.is_none() {
            // 檢查 Cookie 設定夾是否已有 B站專用 cookie 檔
            let expected_filename = "cookie_bilibili.txt";
            let target_file = resolved_cookie_dir.join(expected_filename);
            if !target_file.exists() {
                println!("=================================================================");
                println!("📺 偵測到 Bilibili 影片下載任務：");
                println!("⚠️  【畫質受限警報】B站限制「未登入 / 無 Cookie」使用者僅能下載最低畫質 (360p/480p)！");
                println!("💡 建議提醒：");
                println!("   1. 您可以在下載失敗時，透過彈出的「錯誤恢復選單」自動套用瀏覽器 Cookie 解鎖最高 1080p/4K 畫質。");
                println!("   2. 或者，您也可以繼續下載，但將只能取得最低畫質影像 (純音訊不受此畫質限制)。");
                println!("=================================================================\n");
            }
        }

        // 探測網址：獲取影片清單、判斷是否為 PlayList，並偵測是否為受限內容 (Age / Member only)
        let (mut valid_videos, is_playlist, has_restricted) =
            parser::scan_url(&input_url, args.force_cookie, &site_target)?;

        // 載入專屬 Cookie (手動傳入優先 -> 尋找設定目錄 site 專用檔 -> 阻斷等待匯入)
        let cookie_args = setup::handle_cookies(
            &site_target,
            has_restricted,
            &args.cookie,
            &resolved_cookie_dir,
            is_silent,
        )?;

        // 若成功套用 Cookie 且為播放清單，重新掃描以發掘先前因權限被隱藏的專屬/會員影片
        if !cookie_args.is_empty() && is_playlist {
            valid_videos =
                parser::rescan_with_cookies(&input_url, &cookie_args, valid_videos.len())?;
        }

        // 決定輸出終點資料夾（若是播放清單，會在此層自動建立以「清單標題」命名的專屬子目錄）
        let final_target_dir =
            utils::prepare_output_dir(&final_download_dir, &input_url, &cookie_args, is_playlist);

        // 🎯 初始化 Markdown 下載報告日誌，並寫入基本 Metadata 與標頭
        LogManager::init_log(&final_target_dir, &input_url);
        LogManager::log_event(
            &final_target_dir,
            "INFO",
            &format!("掃描完畢，準備下載 {} 個項目", valid_videos.len()),
        );

        // 構建 yt-dlp 下載核心基礎參數列表
        let dl_args = utils::build_download_args(media_type, &target_ext, &input_url, &cookie_args);

        // 互動式下載前置選項設定 (多國字幕勾選、MKV 高畫質清單選擇等)
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
            failed_tasks: Vec::new(), // ✅ 已將欄位修正為 failed_tasks
        };

        // =====================================================================
        // 11. 啟動並行下載與錯誤恢復重試迴圈
        // =====================================================================
        loop {
            if session.pending_videos.is_empty() {
                break;
            }

            // 非同步派發執行序任務
            let failed_videos = proc::execute_download_session(
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

            // 下載完美成功，或是全自動化命令行模式（命令行模式不觸發互動恢復），直接退出迴圈
            if failed_videos.is_empty() || is_silent {
                break;
            }

            // 🎯 紀錄攔截到的失敗軌跡至 Markdown 報表中
            LogManager::log_event(
                &final_target_dir,
                "WARN",
                &format!(
                    "進入錯誤攔截，共 {} 個項目下載失敗，觸發恢復選單",
                    failed_videos.len()
                ),
            );

            // 彈出錯誤恢復選單
            match ui::prompt_error_recovery(failed_videos.len()) {
                ui::ErrorRecoveryChoice::Browser => {
                    // 🎯 智慧型防撞：優先採用 config.toml 中配置的自訂慣用瀏覽器優先順序
                    let browser = if !config.preferred_browsers.is_empty() {
                        if config.preferred_browsers.len() == 1 {
                            config.preferred_browsers[0].clone()
                        } else {
                            inquire::Select::new(
                                "請選擇您有登入該網站、並想自動提取 Cookie 的瀏覽器：",
                                config.preferred_browsers.clone(),
                            )
                            .prompt()
                            .unwrap()
                        }
                    } else {
                        ui::select_browser(&config.preferred_browsers)
                    };

                    current_cookie_args = vec!["--cookies-from-browser".into(), browser.clone()];
                    LogManager::log_event(
                        &final_target_dir,
                        "INFO",
                        &format!("使用者選擇：自動套用 {} 瀏覽器 Cookie 進行重試", browser),
                    );

                    let failed_items: Vec<parser::VideoItem> = failed_videos.iter().map(|t| t.video.clone()).collect();
                    session = proc::DownloadSession {
                        pending_videos: failed_items,
                        failed_tasks: Vec::new(), // ✅ 修正欄位與型別
                    };
                    println!("🔄 正在套用瀏覽器 Cookie 重新嘗試下載...");
                }
                ui::ErrorRecoveryChoice::Manual => {
                    let new_cookie =
                        setup::wait_for_manual_cookie(&resolved_cookie_dir, &site_target)?;
                    if new_cookie.is_empty() {
                        LogManager::log_event(
                            &final_target_dir,
                            "WARN",
                            "使用者未提供有效 Cookie，放棄重試",
                        );
                        println!("❌ 未提供有效的 Cookie，放棄重試。");
                        break;
                    }
                    current_cookie_args = new_cookie;
                    LogManager::log_event(
                        &final_target_dir,
                        "INFO",
                        "使用者選擇：手動匯入 Cookie 進行重試",
                    );

                    let failed_items: Vec<parser::VideoItem> = failed_videos.iter().map(|t| t.video.clone()).collect();
                    session = proc::DownloadSession {
                        pending_videos: failed_items,
                        failed_tasks: Vec::new(), // ✅ 修正欄位與型別
                    };
                    println!("🔄 正在使用手動 Cookie 重新嘗試下載...");
                }
                ui::ErrorRecoveryChoice::Abort => {
                    LogManager::log_event(
                        &final_target_dir,
                        "INFO",
                        "使用者選擇：放棄失敗項目並結束",
                    );
                    println!("👋 已放棄其餘失敗項目。");
                    break;
                }
            }
        }
    }
    Ok(())
}
