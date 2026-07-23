use crate::parser::VideoItem;
use chrono::Local;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use regex::Regex;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as AsyncCommand;
use tokio::sync::Semaphore;

// =====================================================================
// 1. 錯誤處理與日誌系統 (精準分類錯誤，對接動態排除重試邏輯)
// =====================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum DLMediaError {
    /// 🔐 權限/登入/年齡限制錯誤（觸發黑名單排除重試機制）
    AuthError(String),
    /// 🌐 實體網路連線問題（不排除瀏覽器，直接原參數重試）
    NetworkError(String),
    /// 📂 檔案下載完成但損毀或遺失
    FileCorruption(String),
    /// 未知或非預期錯誤
    Unknown(String),
}

impl std::fmt::Display for DLMediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthError(msg) => write!(f, "帳號權限或年齡限制問題 (需 Cookie): {}", msg),
            Self::NetworkError(msg) => write!(f, "網路連線或逾時問題 (與 Cookie 無關): {}", msg),
            Self::FileCorruption(msg) => write!(f, "檔案損毀或無法定位下載檔: {}", msg),
            Self::Unknown(msg) => write!(f, "發生未知的非預期錯誤: {}", msg),
        }
    }
}

// 🎯 全域靜態鎖，用以儲存本批次任務派發時建立的動態日誌檔名
static CURRENT_LOG_NAME: Mutex<Option<String>> = Mutex::new(None);

pub struct LogManager;

impl LogManager {
    /// 初始化 Markdown 報表標頭（動態加入任務派發時的時間戳）
    pub fn init_log(target_dir: &Path, target_url: &str) {
        // 以下載任務派發時的時間為準
        let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("download_session-{}.md", ts);

        // 將此檔名安全地登記到全域變數中
        if let Ok(mut guard) = CURRENT_LOG_NAME.lock() {
            *guard = Some(filename.clone());
        }

        let log_path = target_dir.join(&filename);
        if !log_path.exists() {
            let header = format!(
                "# yt-dlp-tui 任務執行紀錄\n\n**目標網址**：`{}`\n**啟動時間**：{}\n\n### 執行軌跡\n\n",
                target_url,
                Local::now().format("%Y-%m-%d %H:%M:%S")
            );
            let _ = fs::write(&log_path, header);
        }
    }

    /// 取得當前工作階段的日誌檔名，若全域變數未初始化則退回預設值
    pub fn get_log_filename() -> String {
        if let Ok(guard) = CURRENT_LOG_NAME.lock() {
            if let Some(ref name) = *guard {
                return name.clone();
            }
        }
        "download_session.md".to_string()
    }

    /// 寫入標準 Markdown 格式的單行日誌
    pub fn log_event(target_dir: &Path, level: &str, msg: &str) {
        let filename = Self::get_log_filename();
        let log_path = target_dir.join(&filename);
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        let entry = format!("* **[{}]** [{}] {}\n", ts, level, msg);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = file.write_all(entry.as_bytes());
        }
    }

    /// 過濾 yt-dlp 的標準錯誤並寫入日誌
    pub fn filter_and_log(target_dir: &Path, line: &str) {
        let re = Regex::new(r"\[(youtube|download|error)\]").unwrap();
        if re.is_match(line) {
            let level = if line.to_lowercase().contains("error") { "ERROR" } else { "INFO" };
            Self::log_event(target_dir, level, line);
        }
    }
}

// =====================================================================
// 2. 狀態管理與單向數據流結構
// =====================================================================

#[derive(Clone)]
pub struct DownloadTask {
    pub video: VideoItem,
    pub is_playlist: bool,
    pub media_type: u8,
    pub target_ext: String,
    pub dl_args: Vec<String>,
    pub cookie_args: Vec<String>,
    pub target_dir: PathBuf,
    pub tmp_dir: PathBuf,
}

#[derive(Clone)]
pub struct FailedTask {
    pub video: VideoItem,
}

pub struct TaskResult {
    pub success: bool,
    pub video: VideoItem,
    pub error: Option<DLMediaError>,
}

pub struct DownloadSession {
    pub pending_videos: Vec<VideoItem>,
    pub failed_tasks: Vec<FailedTask>,
}

// =====================================================================
// 3. 媒體處理輔助函式 (字幕淨化、FFmpeg 無損封裝)
// =====================================================================

pub fn process_external_subtitles(
    tmp_dir: &Path,
    ts: &str,
    final_name: &str,
    target_dir: &Path,
    media_type: u8,
) {
    if let Ok(entries) = fs::read_dir(tmp_dir) {
        let re_vtt = Regex::new(r"tmp_.*\.vtt$").unwrap();
        let re_tags = Regex::new(r"<[^>]*>").unwrap();
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                if re_vtt.is_match(file_name) && file_name.contains(ts) && !file_name.contains(".clean.vtt") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        let cleaned = re_tags.replace_all(&content, "");
                        let clean_path = path.with_extension("clean.vtt");
                        let _ = fs::write(&clean_path, cleaned.to_string());
                        if media_type == 1 {
                            let lang_suffix = file_name.split('.').rev().nth(1).unwrap_or("sub");
                            if let Some(ext) = Path::new(final_name).extension().and_then(|e| e.to_str()) {
                                let final_vtt_name = final_name.replace(ext, &format!("{}.vtt", lang_suffix));
                                let _ = fs::rename(&clean_path, target_dir.join(final_vtt_name));
                            }
                        }
                    }
                }
            }
        }
    }
}

async fn get_video_duration(path: &Path) -> Option<f64> {
    let output = AsyncCommand::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            path.to_str().unwrap_or_default(),
        ])
        .output()
        .await
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
}

pub async fn merge_subs_and_danmaku(
    tmp_dir: &Path,
    ts: &str,
    video_path: &Path,
    final_path: &Path,
    pb: ProgressBar,
) -> bool {
    let mut sub_files: Vec<(PathBuf, String, String)> = Vec::new();
    if let Ok(entries) = fs::read_dir(tmp_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&format!("tmp_{}", ts)) && (name.ends_with(".ass") || name.ends_with(".clean.vtt")) {
                let parts: Vec<&str> = name.split('.').collect();
                let raw_lang = if name.ends_with(".clean.vtt") && parts.len() >= 4 {
                    parts[parts.len() - 3].to_string()
                } else if parts.len() >= 3 {
                    parts[parts.len() - 2].to_string()
                } else {
                    "und".to_string()
                };
                let (iso_lang, display_title) = match raw_lang.as_str() {
                    "zh-Hant" | "zh-TW" | "zh-HK" => ("chi", "正體中文"),
                    "zh-Hans" | "zh-CN" | "zh" => ("zho", "簡體中文"),
                    "en" | "en-US" | "en-GB" => ("eng", "English"),
                    "ja" => ("jpn", "日本語"),
                    "ko" => ("kor", "한국어"),
                    "danmaku" => ("cmn", "中文彈幕"),
                    _ => ("und", raw_lang.as_str()),
                };
                sub_files.push((
                    entry.path(),
                    display_title.to_string(),
                    iso_lang.to_string(),
                ));
            }
        }
    }
    if sub_files.is_empty() {
        return false;
    }
    sub_files.sort_by(|a, b| a.1.cmp(&b.1));
    let total_duration = get_video_duration(video_path).await.unwrap_or(1.0);
    let mut cmd = AsyncCommand::new("ffmpeg");
    cmd.arg("-loglevel").arg("error");
    cmd.arg("-hide_banner");
    cmd.arg("-progress").arg("-").arg("-nostats");
    cmd.arg("-i").arg(video_path);
    for (sub_path, _, _) in &sub_files {
        cmd.arg("-i").arg(sub_path);
    }
    cmd.arg("-c:v").arg("copy").arg("-c:a").arg("copy");
    if final_path.extension().and_then(|e| e.to_str()) == Some("mp4") {
        cmd.arg("-c:s").arg("mov_text");
        cmd.arg("-movflags").arg("+use_metadata_tags");
    } else {
        cmd.arg("-c:s").arg("copy");
    }
    cmd.arg("-map").arg("0");
    for i in 1..=sub_files.len() {
        cmd.arg("-map").arg(format!("{}", i));
    }
    for (i, (_, title, iso)) in sub_files.iter().enumerate() {
        cmd.arg(format!("-metadata:s:s:{}", i)).arg(format!("language={}", iso));
        cmd.arg(format!("-metadata:s:s:{}", i)).arg(format!("title={}", title));
        cmd.arg(format!("-metadata:s:s:{}", i)).arg(format!("handler_name={}", title));
        cmd.arg(format!("-metadata:s:s:{}", i)).arg(format!("name={}", title));
    }
    cmd.arg("-y").arg(final_path);
    cmd.stdout(Stdio::piped());
    if let Ok(mut child) = cmd.spawn() {
        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if line.starts_with("out_time_us=") {
                    if let Ok(us) = line.replace("out_time_us=", "").trim().parse::<f64>() {
                        let current_sec = us / 1_000_000.0;
                        let pct = (current_sec / total_duration * 100.0) as u64;
                        pb.set_position(pct.min(100));
                        pb.set_message(format!("封裝中... {}%", pct.min(100)));
                    }
                }
            }
        }
        child.wait().await.map_or(false, |s| s.success())
    } else {
        false
    }
}

pub fn get_video_resolution(path: &Path) -> Option<String> {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height",
            "-of", "csv=s=x:p=0",
            path.to_str().unwrap_or_default(),
        ])
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn cleanup_tmps(session_tmp_dir: &Path) {
    if session_tmp_dir.exists() {
        let _ = fs::remove_dir_all(session_tmp_dir);
    }
}

// =====================================================================
// 4. 核心工作流調度 (execute_task & 雙監聽機制)
// =====================================================================

async fn execute_task(
    task: DownloadTask,
    semaphore: Arc<Semaphore>,
    multi_progress: Arc<MultiProgress>,
    total: usize,
    idx: usize,
) -> TaskResult {
    let permit = semaphore.acquire_owned().await.unwrap();
    let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
    // 建立極致安全的隔離沙盒資料夾
    let session_tmp_dir = task.tmp_dir.join(format!("{}_pid{}_{}", ts, std::process::id(), idx));
    let _ = fs::create_dir_all(&session_tmp_dir);
    LogManager::log_event(
        &task.target_dir,
        "INFO",
        &format!("開始處理任務：{}", task.video.title),
    );
    let safe_title = task.video.title.replace(&['/', '\\', ':', '*', '?', '"', '<', '>', '|'][..], "_");
    let final_name = if task.is_playlist {
        format!("{:02}-{}_{}.{}", idx + 1, safe_title, ts, task.target_ext)
    } else {
        format!("{}_{}.{}", safe_title, ts, task.target_ext)
    };
    let final_path = task.target_dir.join(&final_name);
    let pb = multi_progress.add(ProgressBar::new(100));
    pb.set_style(
        ProgressStyle::with_template(
            "{prefix:.bold.dim} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {msg}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    pb.set_prefix(format!("[{}/{}]", idx + 1, total));
    pb.set_message("準備下載...");
    let mut current_dl_args = task.dl_args.clone();
    // 注入使用者選擇的字幕/語言參數
    if !task.video.chosen_langs.is_empty() {
        current_dl_args.push("--write-subs".into());
        current_dl_args.push("--write-auto-subs".into());
        current_dl_args.push("--sub-langs".into());
        current_dl_args.push(task.video.chosen_langs.join(","));
    }
    if let Some(ref vid_id) = task.video.chosen_format {
        if let Some(f_idx) = current_dl_args.iter().position(|x| x == "-f") {
            current_dl_args[f_idx + 1] = format!("{}+bestaudio/best", vid_id);
        }
    } else if task.media_type != 1 && task.target_ext == "mp4" {
        pb.println("採用 MP4：自動下載最高相容畫質。");
    }
    let tmp_output_template = format!("{}/tmp_{}.%(ext)s", session_tmp_dir.to_string_lossy(), ts);
    current_dl_args.push("-o".into());
    current_dl_args.push(tmp_output_template);
    current_dl_args.push(task.video.url.clone());
    let mut debug_args = current_dl_args.clone();
    debug_args.retain(|arg| arg != "--no-warnings");
    let mut child = AsyncCommand::new("yt-dlp")
        .current_dir(&session_tmp_dir)
        .args(&task.cookie_args)
        .args(&debug_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("執行 yt-dlp 失敗");
    let target_dir_clone = task.target_dir.clone();
    let stderr_accumulator = Arc::new(tokio::sync::Mutex::new(String::new()));
    let stderr_accum_clone = Arc::clone(&stderr_accumulator);
    // 1. Stdout 監聽線程 (進度條解析)
    if let Some(stdout) = child.stdout.take() {
        let pb_clone = pb.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if line.contains("[download]") && line.contains("%") {
                    if let Some(pct_str) = line.split_whitespace().find(|s| s.contains("%")) {
                        if let Ok(pct) = pct_str.replace('%', "").parse::<f64>() {
                            pb_clone.set_position(pct as u64);
                        }
                    }
                    pb_clone.set_message(line.replace("[download]", "").trim().to_string());
                }
            }
        });
    }
    // 2. Stderr 監聽線程 (日誌與精準錯誤分類累積)
    if let Some(stderr) = child.stderr.take() {
        let dir_clone = target_dir_clone.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                LogManager::filter_and_log(&dir_clone, &line);
                let mut accum = stderr_accum_clone.lock().await;
                accum.push_str(&line);
                accum.push('\n');
            }
        });
    }
    let status = child.wait().await.unwrap_or_else(|_| panic!("等待 yt-dlp 失敗"));
    let mut success = false;
    let mut error = None;
    let mut downloaded_path_str = String::new();
    if status.success() {
        if let Ok(entries) = fs::read_dir(&session_tmp_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                    if file_name.starts_with(&format!("tmp_{}", ts))
                        && !file_name.ends_with(".vtt")
                        && !file_name.ends_with(".ass")
                        && !file_name.ends_with(".srt")
                    {
                        downloaded_path_str = path.to_string_lossy().to_string();
                        break;
                    }
                }
            }
        }
    } else {
        // 🎯 核心重構：讀取 stderr 累積器進行精準錯誤識別
        let err_content = stderr_accumulator.lock().await.to_lowercase();
        let is_auth_issue = err_content.contains("sign in")
            || err_content.contains("login")
            || err_content.contains("cookie")
            || err_content.contains("登錄")
            || err_content.contains("private")
            || err_content.contains("age-restricted")
            || err_content.contains("403");
        if is_auth_issue {
            error = Some(DLMediaError::AuthError(
                "偵測到需要帳號登入、Cookie 或年齡驗證限制內容".into(),
            ));
        } else if err_content.contains("timeout") || err_content.contains("connection") || err_content.contains("unable to download webpage") {
            error = Some(DLMediaError::NetworkError(
                "網路連線超時或與目標伺服器建立連線失敗".into(),
            ));
        } else {
            error = Some(DLMediaError::Unknown(
                if err_content.trim().is_empty() {
                    "發生未知的非預期錯誤".into()
                } else {
                    err_content
                }
            ));
        }
    }
    let mut final_res_info = String::new();
    if status.success() && !downloaded_path_str.is_empty() {
        let downloaded_file = PathBuf::from(downloaded_path_str);
        process_external_subtitles(
            &session_tmp_dir,
            &ts,
            &final_name,
            &task.target_dir,
            task.media_type,
        );
        pb.set_position(0);
        pb.set_message("正在執行封裝...");
        let merged = if task.media_type != 1 {
            merge_subs_and_danmaku(
                &session_tmp_dir,
                &ts,
                &downloaded_file,
                &final_path,
                pb.clone(),
            )
            .await
        } else {
            false
        };
        if !merged {
            let _ = fs::rename(&downloaded_file, &final_path);
            pb.set_position(100);
        }
        if task.media_type != 1 {
            final_res_info = get_video_resolution(&final_path).map_or("".into(), |r| format!(" [畫質: {}]", r));
        }
        pb.println(format!("儲存成功：{}{}", final_name, final_res_info));
        pb.finish_and_clear();
        LogManager::log_event(
            &task.target_dir,
            "SUCCESS",
            &format!("檔案已儲存: {}", final_name),
        );
        success = true;
    } else {
        if status.success() && downloaded_path_str.is_empty() {
            error = Some(DLMediaError::FileCorruption(
                "下載完成但無法在暫存區定位媒體檔案".into(),
            ));
        }
        if let Some(ref e) = error {
            LogManager::log_event(&task.target_dir, "ERROR", &e.to_string());
        }
        pb.println(format!("下載失敗：{}", task.video.title));
        pb.finish_and_clear();
    }
    cleanup_tmps(&session_tmp_dir);
    drop(permit);
    TaskResult {
        success,
        video: task.video,
        error,
    }
}

/// 提供給外部調用的工作流總入口
pub async fn execute_download_session(
    mut session: DownloadSession,
    is_playlist: bool,
    media_type: u8,
    target_ext: String,
    dl_args: Vec<String>,
    cookie_args: Vec<String>,
    target_dir: PathBuf,
    tmp_dir: PathBuf,
    max_concurrent: u32,
) -> anyhow::Result<Vec<FailedTask>> {
    let start_time = Instant::now();
    let total = session.pending_videos.len();
    let _ = fs::create_dir_all(&tmp_dir);
    let semaphore = Arc::new(Semaphore::new(max_concurrent as usize));
    let multi_progress = Arc::new(MultiProgress::new());
    let mut handles = vec![];
    for (idx, video) in session.pending_videos.into_iter().enumerate() {
        let task = DownloadTask {
            video,
            is_playlist,
            media_type,
            target_ext: target_ext.clone(),
            dl_args: dl_args.clone(),
            cookie_args: cookie_args.clone(),
            target_dir: target_dir.clone(),
            tmp_dir: tmp_dir.clone(),
        };
        let sem_clone = semaphore.clone();
        let mp_clone = multi_progress.clone();
        handles.push(tokio::spawn(async move {
            execute_task(task, sem_clone, mp_clone, total, idx).await
        }));
    }
    let mut success_count = 0;
    for handle in handles {
        if let Ok(result) = handle.await {
            if result.success {
                success_count += 1;
            } else {
                let err = result.error.unwrap_or(DLMediaError::Unknown("未知的執行異常".into()));
                println!("❌ [{}] 失敗原因: {}", result.video.title, err);
                session.failed_tasks.push(FailedTask {
                    video: result.video,
                });
            }
        }
    }
    let duration = start_time.elapsed();
    let time_str = format!(
        "{} 分 {} 秒",
        duration.as_secs() / 60,
        duration.as_secs() % 60
    );
    crate::ui::print_summary(
        success_count,
        session.failed_tasks.len(),
        &time_str,
        &target_dir.to_string_lossy(),
    );
    Ok(session.failed_tasks)
}
