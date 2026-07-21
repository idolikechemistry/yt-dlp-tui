use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

/// 🎯 畫質格式結構
#[derive(Debug, Clone)]
pub struct VideoFormat {
    pub format_id: String,
    pub height: u32,
    pub vcodec: String,
    pub ext: String,
}

/// 🎯 整合後的探測結果結構 (對應 UI 顯示所需之中介資料)
#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub langs: Vec<String>,
    pub formats: Vec<VideoFormat>,
}

/// 🎯 影片項目結構，容納新功能資料以利 UI 隔離
#[derive(Debug, Clone)]
pub struct VideoItem {
    pub title: String,
    pub url: String,
    pub metadata: Option<VideoMetadata>,
    pub chosen_langs: Vec<String>,
    pub chosen_format: Option<String>,
}

/// 🌐 根據網址特徵解析出目標網站名稱，用於自動配對專用 Cookie 檔案
pub fn extract_site_name(url: &str) -> String {
    let url_lower = url.to_lowercase();
    if url_lower.contains("youtube.com") || url_lower.contains("youtu.be") || url_lower.contains("googleusercontent.com") {
        return "youtube".into();
    }
    if url_lower.contains("bilibili.com") || url_lower.contains("b23.tv") {
        return "bilibili".into();
    }
    if url_lower.contains("twitter.com") || url_lower.contains("x.com") {
        return "twitter".into();
    }
    if url_lower.contains("facebook.com") || url_lower.contains("fb.watch") {
        return "facebook".into();
    }
    if url_lower.contains("instagram.com") {
        return "instagram".into();
    }
    
    // 預設解析域名
    url_lower
        .split('/')
        .nth(2)
        .and_then(|d| d.split('.').rev().nth(1))
        .unwrap_or("unknown")
        .to_string()
}

/// 🔍 掃描目標網址，輕量解析出是否為播放清單、包含哪些影片項目，以及是否需要登入 Cookie
pub fn scan_url(
    input_url: &str,
    force_cookie: bool,
    site_target: &str,
) -> Result<(Vec<VideoItem>, bool, bool)> {
    println!("🔍 正在分析網址資訊...");
    
    let output = Command::new("yt-dlp")
        .args([
            "--flat-playlist",
            "--skip-download",
            "--print",
            "playlist:%(playlist_title)s",
            "--print",
            "item:%(title)s|%(webpage_url)s",
            "--ignore-errors",
            "--no-warnings",
            input_url,
        ])
        .output()
        .context("執行 yt-dlp 解析清單失敗，請確認網路與 yt-dlp 是否就緒")?;

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr).to_lowercase();
    
    let mut valid_videos = Vec::new();
    let mut is_playlist = false;
    
    // 透過 stderr 分析是否需要登入權限 (Cookie)
    let mut has_restricted = force_cookie 
        || stderr_str.contains("sign in") 
        || stderr_str.contains("login") 
        || stderr_str.contains("cookie") 
        || stderr_str.contains("登錄") 
        || stderr_str.contains("private");

    for line in stdout_str.lines() {
        if let Some(pl_title) = line.strip_prefix("playlist:") {
            if pl_title != "NA" && !pl_title.is_empty() && pl_title != "null" {
                is_playlist = true;
            }
        } else if let Some(item) = line.strip_prefix("item:") {
            if item.contains("[Private video]") || item.contains("[Deleted video]") || item.contains("Private") {
                has_restricted = true;
            } else if let Some((title, url)) = item.rsplit_once('|') {
                valid_videos.push(VideoItem {
                    title: title.to_string(),
                    url: url.to_string(),
                    metadata: None,
                    chosen_langs: Vec::new(),
                    chosen_format: None,
                });
            }
        }
    }

    // 若掃描結果為空，退化為處理單一影片模式
    if valid_videos.is_empty() {
        valid_videos.push(VideoItem {
            title: "Video".to_string(),
            url: input_url.to_string(),
            metadata: None,
            chosen_langs: Vec::new(),
            chosen_format: None,
        });
    }

    // Bilibili 限制內容通常需要 Cookie 才能抓取 1080p 以上畫質，設為預設防範
    if site_target == "bilibili" {
        has_restricted = true;
    }

    print_analysis_report(
        site_target,
        is_playlist,
        valid_videos.len(),
        has_restricted,
        force_cookie,
    );

    Ok((valid_videos, is_playlist, has_restricted))
}

/// 🔄 當套用 Cookie 後，重新對網址進行深度掃描，用以解鎖先前被隱藏或會員專屬的影片項目
pub fn rescan_with_cookies(
    input_url: &str,
    cookie_args: &[String],
    original_total: usize,
) -> Result<Vec<VideoItem>> {
    println!("🔄 正在透過 Cookie 驗證並重新掃描清單...");
    
    let output = Command::new("yt-dlp")
        .args(cookie_args)
        .args([
            "--flat-playlist",
            "--skip-download",
            "--print",
            "item:%(title)s|%(webpage_url)s",
            "--ignore-errors",
            "--no-warnings",
            input_url,
        ])
        .output()
        .context("透過 Cookie 重新解析清單失敗")?;

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let mut new_videos = Vec::new();

    for line in stdout_str.lines() {
        if let Some(item) = line.strip_prefix("item:") {
            if item.contains("[Private video]") || item.contains("[Deleted video]") {
                continue;
            }
            if let Some((title, url)) = item.rsplit_once('|') {
                new_videos.push(VideoItem {
                    title: title.to_string(),
                    url: url.to_string(),
                    metadata: None,
                    chosen_langs: Vec::new(),
                    chosen_format: None,
                });
            }
        }
    }

    if new_videos.is_empty() {
        new_videos.push(VideoItem {
            title: "Video".to_string(),
            url: input_url.to_string(),
            metadata: None,
            chosen_langs: Vec::new(),
            chosen_format: None,
        });
    }

    let new_total = new_videos.len();
    if new_total > original_total {
        println!("--------------------------------------------------");
        println!(
            "🔓 解鎖成功！透過 Cookie 發現了 {} 部先前被隱藏或會員專屬的內容！",
            new_total - original_total
        );
        println!("--------------------------------------------------");
    }

    Ok(new_videos)
}

/// 📊 在終端機中印出網址前置分析的統計簡報，幫助用戶確認目前掃描狀態
fn print_analysis_report(site: &str, is_pl: bool, count: usize, restricted: bool, forced: bool) {
    println!("--------------------------------------------------");
    println!("📡 來源網站：{}", site);
    println!(
        "📋 內容類型：{}",
        if is_pl {
            format!("【播放清單】(包含 {} 部內容)", count)
        } else {
            "【單一內容】".into()
        }
    );
    let status = if forced {
        "⚠️ 強制調用 Cookie 模式"
    } else if restricted {
        "⚠️ 偵測到限制/高畫質內容 (需要 Cookie 解鎖)"
    } else {
        "🔓 公開內容"
    };
    println!("🔒 權限狀態：{}", status);
    println!("--------------------------------------------------");
}

/// 🎯 深度探測單一影片流的進階 Metadata（包含影片擁有的外掛字幕軌、可用畫質解析度格式等）
pub fn probe_video_info(url: &str, cookie_args: &[String]) -> Result<VideoMetadata> {
    let is_bilibili = url.contains("bilibili.com") || url.contains("b23.tv");
    
    let output = Command::new("yt-dlp")
        .args(cookie_args)
        .args(["--dump-json", "--no-warnings", "--skip-download", url])
        .output()
        .context("無法獲取影片的進階 Metadata，請確認 yt-dlp 能正常解析此網址")?;

    let mut langs = Vec::new();
    let mut formats = Vec::new();

    if let Ok(json) = serde_json::from_slice::<Value>(&output.stdout) {
        // 1. 探測外掛字幕軌與自動生成字幕
        for sub_type in ["subtitles", "automatic_captions"] {
            if let Some(subs) = json.get(sub_type).and_then(|s| s.as_object()) {
                for lang in subs.keys() {
                    langs.push(lang.clone());
                }
            }
        }

        // 2. 探測影片可用的畫面解析度格式
        if let Some(fmts) = json.get("formats").and_then(|f| f.as_array()) {
            for f in fmts {
                let vcodec = f.get("vcodec").and_then(|v| v.as_str()).unwrap_or("none");
                let height = f.get("height").and_then(|h| h.as_u64());
                let ext = f.get("ext").and_then(|e| e.as_str()).unwrap_or("");
                
                // 排除無畫面(純音訊)或無效的流
                if vcodec != "none" && height.is_some() && ext != "mhtml" {
                    formats.push(VideoFormat {
                        format_id: f.get("format_id")
                            .and_then(|fid| fid.as_str())
                            .unwrap_or("")
                            .to_string(),
                        height: height.unwrap() as u32,
                        vcodec: vcodec.to_string(),
                        ext: ext.to_string(),
                    });
                }
            }
        }
    }

    // 若為 Bilibili，強制追加彈幕(danmaku)虛擬軌道，以便後續調用 danmaku2ass 封裝
    if is_bilibili {
        langs.push("danmaku".into());
    }

    langs.sort();
    langs.dedup();

    Ok(VideoMetadata { langs, formats })
}
