mod config;
mod executor;
mod setup;

use inquire::{Confirm, Select, Text};

// 必須加上 Clone 才能在多線程中分配給不同的任務
#[derive(Clone)]
pub struct DownloadOption {
    pub display_name: &'static str,
    pub args: Vec<&'static str>,
}

impl std::fmt::Display for DownloadOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name)
    }
}

#[derive(Clone)]
pub struct AppConfig {
    pub urls: Vec<String>,
    pub base_args: Vec<String>,
    pub need_subs: bool,
    pub need_thumbnail: bool,
    pub download_dir: String,
    pub temp_dir: String,
}

// 將 main 宣告為 tokio 的非同步進入點
#[tokio::main]
async fn main() {
    let config_manager = config::ConfigManager::load_or_create();
    let final_download_dir = config_manager.get_final_download_dir();

    if !setup::check_environment() {
        std::process::exit(1);
    }

    println!("\n歡迎使用 yt-dlp TUI 下載工具 🚀");
    println!("📦 檔案將儲存至: {}", final_download_dir);
    println!("(設定檔路徑: {})\n", config_manager.config_dir.display());

    let url_input = Text::new("請輸入影片或播放清單網址 (多個網址請以「空格」隔開):")
        .prompt()
        .unwrap();

    let urls: Vec<String> = url_input
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    if urls.is_empty() {
        eprintln!("❌ 錯誤：未輸入任何網址。");
        std::process::exit(1);
    }

    let category_options = vec![
        "🎵 下載純音訊 (自動提取與轉檔)",
        "🎬 下載有聲影片 (自動合併最高畫質與音質)",
        "🔕 下載純影片 (無聲素材)",
    ];

    let selected_category = Select::new("請選擇您要下載的媒體類型：", category_options)
        .prompt()
        .unwrap();

    let mut base_args: Vec<String> = Vec::new();

    if selected_category.starts_with("🎵") {
        let audio_options = vec![
            DownloadOption {
                display_name: "MP3 (相容性最高，最常用)",
                args: vec!["-x", "--audio-format", "mp3"],
            },
            DownloadOption {
                display_name: "M4A (Apple 裝置與 iTunes 推薦)",
                args: vec!["-x", "--audio-format", "m4a"],
            },
        ];
        let selected_audio = Select::new("請選擇目標音訊格式：", audio_options)
            .prompt()
            .unwrap();
        base_args.extend(selected_audio.args.iter().map(|&s| s.to_string()));
    } else if selected_category.starts_with("🎬") {
        let video_options = vec![
            DownloadOption {
                display_name: "最高畫質 (自動封裝為 MKV 或 WebM)",
                args: vec!["-f", "bestvideo+bestaudio"],
            },
            DownloadOption {
                display_name: "高相容性 MP4 (自動篩選 MP4 容器格式)",
                args: vec!["-f", "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]"],
            },
        ];
        let selected_video = Select::new("請選擇目標影片畫質與格式：", video_options)
            .prompt()
            .unwrap();
        base_args.extend(selected_video.args.iter().map(|&s| s.to_string()));
    } else {
        base_args.push("-f".to_string());
        base_args.push("bestvideo".to_string());
    }

    let mut need_subs = false;
    if !selected_category.starts_with("🎵") {
        need_subs = Confirm::new("是否需要下載並嵌入字幕 (包含自動生成字幕)？")
            .with_default(false)
            .prompt()
            .unwrap();
    }

    let need_thumbnail = Confirm::new("是否需要將影片封面嵌入為媒體縮圖？")
        .with_default(true)
        .prompt()
        .unwrap();

    println!("\n========================================");
    println!("✅ 設定完成！即將開始並行下載任務...");
    println!("========================================\n");

    // --- 多線程並行派發邏輯開始 ---
    let mut task_handles = Vec::new();

    // 針對每一個輸入的網址，產生獨立的設定檔與暫存目錄
    for (idx, url) in urls.into_iter().enumerate() {
        // 呼叫我們剛剛在 config.rs 寫好的安全隔離暫存目錄產生器
        let temp_dir = config_manager
            .create_isolated_tmp_dir(idx)
            .expect("無法建立任務暫存目錄");

        let task_config = AppConfig {
            urls: vec![url], // 讓這個任務專心下載它負責的這一個網址
            base_args: base_args.clone(),
            need_subs,
            need_thumbnail,
            download_dir: final_download_dir.clone(),
            temp_dir: temp_dir.to_string_lossy().to_string(),
        };

        // 將任務丟進 tokio 執行緒池中並行處理
        let handle = tokio::spawn(async move {
            executor::execute_download(task_config).await;
        });

        task_handles.push(handle);
    }

    // 等待所有線程的下載任務都順利結束
    for handle in task_handles {
        let _ = handle.await;
    }

    println!("\n🎉 恭喜！所有下載任務已順利完成！");
}
