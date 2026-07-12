mod config;
mod executor;
mod setup;

use inquire::{Confirm, Select, Text};
use uuid::Uuid;

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

/// 用來打包所有使用者選定的參數，準備傳遞給 executor
pub struct AppConfig {
    pub urls: Vec<String>,
    pub base_args: Vec<String>,
    pub need_subs: bool,
    pub need_thumbnail: bool,
    pub download_dir: String,
    pub temp_dir: String,
}

fn main() {
    // 1. 初始化設定檔與目錄
    let config_manager = config::ConfigManager::load_or_create();
    let final_download_dir = config_manager.get_final_download_dir();

    // 生成該次任務專屬的唯一暫存目錄: .tmp/<UUID>
    let task_uuid = Uuid::new_v4().to_string();
    let task_temp_dir = config_manager.config_dir.join(".tmp").join(&task_uuid);
    std::fs::create_dir_all(&task_temp_dir).expect("無法建立本次任務的暫存目錄");

    // 2. 環境防呆檢查
    if !setup::check_environment() {
        std::process::exit(1);
    }

    println!("\n歡迎使用 yt-dlp TUI 下載工具 🚀");
    println!("📦 檔案將儲存至: {}", final_download_dir);
    println!("(設定檔路徑: {})\n", config_manager.config_dir.display());

    // 3. 步驟一：輸入網址
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

    // 4. 步驟二：選擇大分類 (音訊 vs 影片)
    let category_options = vec![
        "🎵 下載純音訊 (自動提取與轉檔)",
        "🎬 下載有聲影片 (自動合併最高畫質與音質)",
        "🔕 下載純影片 (無聲素材)",
    ];

    let selected_category = Select::new("請選擇您要下載的媒體類型：", category_options)
        .prompt()
        .unwrap();

    let mut base_args: Vec<String> = Vec::new();

    // 5. 步驟三：根據大分類，給予細部格式或畫質選項
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
            DownloadOption {
                display_name: "FLAC (無損音質，檔案較大)",
                args: vec!["-x", "--audio-format", "flac"],
            },
            DownloadOption {
                display_name: "WAV (無損未壓縮，適合剪輯)",
                args: vec!["-x", "--audio-format", "wav"],
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
                display_name: "最高畫質 (強制限制最高 1080p)",
                args: vec!["-f", "bestvideo+bestaudio", "-S", "res:1080"],
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

    // 6. 步驟四：附加功能詢問 (字幕)
    let mut need_subs = false;
    if !selected_category.starts_with("🎵") {
        need_subs = Confirm::new("是否需要下載並嵌入字幕 (包含自動生成字幕)？")
            .with_default(false)
            .prompt()
            .unwrap();
    }

    // 7. 步驟五：附加功能詢問 (封面)
    let need_thumbnail = Confirm::new("是否需要將影片封面嵌入為媒體縮圖？")
        .with_default(true)
        .prompt()
        .unwrap();

    // 8. 打包設定準備執行
    let app_config = AppConfig {
        urls,
        base_args,
        need_subs,
        need_thumbnail,
        download_dir: final_download_dir,
        temp_dir: task_temp_dir.to_string_lossy().to_string(),
    };

    println!("\n========================================");
    println!("✅ 設定完成！即將開始下載任務...");
    println!("========================================\n");

    // 呼叫 executor 模組來實際執行 yt-dlp
    executor::execute_download(app_config);
}
