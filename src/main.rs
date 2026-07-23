mod args;
mod config;
mod parser;
mod proc;
mod setup;
mod ui;
mod utils;

use anyhow::Result;
use args::Args;
use clap::Parser;
use std::process;

/// 🎯 程式進入點
#[tokio::main]
async fn main() {
    println!("yt-dlp-tui v{}", env!("CARGO_PKG_VERSION"));
    if let Err(e) = run().await {
        eprintln!("\n❌ [執行錯誤]: {}", e);
        for cause in e.chain().skip(1) {
            eprintln!("   原因: {}", cause);
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

    // 1. 啟動時自動檢查更新（自動化 CLI 模式會自動跳過）
    setup::check_and_prompt_update(args.is_fully_automated()).ok();

    // 2. 支援手動強制更新
    if args.update {
        setup::update_app()?;
        return Ok(());
    }

    // 3. 載入偏好設定
    let (config_path, config) = setup::init_config()?;

    // 4. 解析 Cookie 目錄
    let resolved_cookie_dir = if config.cookie_dir.is_empty() {
        config_path.parent().unwrap().to_path_buf()
    } else {
        std::path::PathBuf::from(&config.cookie_dir)
    };

    // 5. 獲取使用者輸入 (解構出 urls, media_type, target_ext)
    let (urls, media_type, target_ext) = ui::get_user_input(&args)?;

    let mut session = proc::DownloadSession {
        pending_videos: Vec::new(),
        failed_tasks: Vec::new(),
    };

    for url in &urls {
        let site_name = parser::extract_site_name(url);
        // 取得 Cookie 參數
        let cookie_args = setup::handle_cookies(
            &site_name,
            false,
            &args.cookie,
            &resolved_cookie_dir,
            args.is_fully_automated(),
        )?;

        // 🎯 修正核心錯誤：將 args.fc 修正為正確的 args.force_cookie
        let (mut videos, _is_playlist, _restricted) = parser::scan_url(url, args.force_cookie, &site_name)?;
        
        setup_download_options(&mut videos, &cookie_args, args.is_fully_automated(), media_type, &target_ext);
        session.pending_videos.extend(videos);
    }

    if !session.pending_videos.is_empty() {
        let dl_args = utils::build_download_args(media_type, &target_ext, "", &[]);
        let target_dir = if config.download_dir.is_empty() {
            dirs::download_dir().unwrap_or_default()
        } else {
            std::path::PathBuf::from(&config.download_dir)
        };

        proc::LogManager::init_log(&target_dir, &urls.join(" "));

        // 呼叫下載核心
        let _failed_tasks = proc::execute_download_session(
            session,
            urls.len() > 1,
            media_type,
            target_ext,
            dl_args,
            Vec::new(),
            target_dir,
            resolved_cookie_dir.join("tmp"),
            config.max_concurrent_downloads,
        ).await?;
    }

    Ok(())
}
