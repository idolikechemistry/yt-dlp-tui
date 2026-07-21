use crate::args::MediaType;
use crate::parser::VideoFormat;
use anyhow::Result;
use inquire::{
    ui::{RenderConfig, Styled},
    MultiSelect, Select as InquireSelect, Text,
};

/// 錯誤恢復的選擇項目
pub enum ErrorRecoveryChoice {
    Browser,
    Manual,
    Abort,
}

/// 估算字串在終端機中的視覺寬度 (考量 CJK 全形字元佔 2 格，ASCII 佔 1 格)
fn display_width(s: &str) -> usize {
    s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
}

/// 將字串以空白填滿至指定的視覺寬度
fn pad_line(s: &str, target_width: usize) -> String {
    let current_w = display_width(s);
    if current_w >= target_width {
        s.to_string()
    } else {
        let padding = " ".repeat(target_width - current_w);
        format!("{}{}", s, padding)
    }
}

/// 互動式取得使用者輸入
/// 回傳型別為 Vec 以支援多連結批量輸入
pub fn get_user_input(args: &crate::args::Args) -> Result<(Vec<String>, u8, String)> {
    // 1. 取得網址 (支援多個，以空格隔開)
    let urls = match &args.url {
        Some(u) => u.clone(),
        None => {
            let input = Text::new("> 請貼上影片或播放清單網址 (多個網址請用空格隔開)：")
                .prompt()
                .unwrap_or_default();
            // 清洗並切割字串成陣列
            input.split_whitespace().map(|s| s.to_string()).collect()
        }
    };

    if urls.is_empty() {
        anyhow::bail!("[Error] 未輸入任何網址。");
    }

    // 2. 取得下載類型
    let media_type_enum = match args.media_type {
        Some(t) => t,
        None => {
            let types = vec![
                "1. 音訊 (自動提取最高音質轉檔)",
                "2. 無聲影片 (僅下載最高畫質影片素材)",
                "3. 有聲影片 (預設最高畫質影音合併)",
            ];
            let selection = InquireSelect::new("> 請選擇您要下載的媒體類型：", types)
                .prompt()
                .unwrap_or("3. 🎥 有聲影片 (預設最高畫質影音合併 [推薦])");

            if selection.starts_with('1') {
                MediaType::Audio
            } else if selection.starts_with('2') {
                MediaType::VideoOnly
            } else {
                MediaType::Video
            }
        }
    };

    // 3. 取得格式
    let format = match &args.format {
        Some(f) => f.clone(),
        None => {
            let formats = match media_type_enum {
                MediaType::Audio => vec!["m4a", "mp3"],
                _ => vec![
                    "mp4 (高相容性：強制鎖定 H.264/AAC 編碼，保證所有裝置流暢播放)",
                    "mkv (解鎖 4K/8K 畫質：保留最高原始視訊與音訊格式封裝)",
                ],
            };
            let selection = InquireSelect::new("> 請選擇下載的輸出封裝格式：", formats)
                .prompt()
                .unwrap_or("mp4");

            // 清洗字串：只取第一個空格前的部分 (例如 "mp4" 或 "mkv")
            selection
                .split_whitespace()
                .next()
                .unwrap_or("mp4")
                .to_string()
        }
    };

    // 回傳時將 Enum 轉為 u8 給底層邏輯使用
    Ok((urls, media_type_enum as u8, format))
}

/// 下載完成後的總結報告 (動態畫出外框，解決 CJK 字串與長路徑導致外框凸出的問題)
pub fn print_summary(success: usize, fail: usize, duration: &str, path: &str) {
    let title_line = "  yt-dlp-tui 任務執行總結  ".to_string();
    let duration_line = format!("  總體耗時：{}", duration);
    let stats_line = format!("  任務統計：成功 {} 個 / 失敗 {} 個", success, fail);
    let path_line = format!("  儲存路徑：{}", path);

    // 計算這幾行中最大的視覺寬度，並給予一個最小預設寬度 (例如 54) 確保基本美觀
    let max_content_w = [
        display_width(&title_line),
        display_width(&duration_line),
        display_width(&stats_line),
        display_width(&path_line),
    ]
    .iter()
    .copied()
    .max()
    .unwrap_or(54);

    let box_width = max_content_w.max(54);

    // 根據最長的一行，動態重繪頂部、中部與底部外框
    let top_border = format!("┌{}┐", "─".repeat(box_width));
    let middle_border = format!("├{}┤", "─".repeat(box_width));
    let bottom_border = format!("└{}┘", "─".repeat(box_width));

    println!("\n{}", top_border);
    println!("│{}│", pad_line(&title_line, box_width));
    println!("{}", middle_border);
    println!("│{}│", pad_line(&duration_line, box_width));
    println!("│{}│", pad_line(&stats_line, box_width));
    println!("│{}│", pad_line(&path_line, box_width));
    println!("{}", bottom_border);

    if fail > 0 {
        println!(
            "\n[Warning] 有 {} 個項目下載失敗。您可以查看儲存路徑下的 [download_session.md] 獲取詳細錯誤日誌。",
            fail
        );
    }
}

/// 畫質選擇選單
pub fn select_resolution(formats: &[VideoFormat]) -> Option<String> {
    let mut options_raw: Vec<&VideoFormat> = formats.iter().filter(|f| f.height > 1080).collect();
    if options_raw.is_empty() {
        return None;
    }
    if let Some(fhd) = formats
        .iter()
        .filter(|f| f.height <= 1080)
        .max_by_key(|f| f.height)
    {
        options_raw.push(fhd);
    }
    options_raw.sort_by(|a, b| b.height.cmp(&a.height));
    options_raw.dedup_by(|a, b| a.height == b.height);

    let display_options: Vec<String> = options_raw
        .iter()
        .map(|f| {
            let quality_label = match f.height {
                4320 => "8K Ultra HD (極致震撼畫質)",
                2160 => "4K Ultra HD (精細超高解析度)",
                1440 => "2K Quad HD (細緻高解析度)",
                1080 => "1080p Full HD (標準主流高畫質)",
                _ => "高畫質視訊",
            };
            format!(
                "{} - {}p (編碼: {}, 格式: {})",
                quality_label, f.height, f.vcodec, f.ext
            )
        })
        .collect();

    let ans = InquireSelect::new(
        "[Config] 偵測到高畫質選項，請選擇您偏好的解析度：",
        display_options.clone(),
    )
    .prompt()
    .ok()?;

    let idx = display_options.iter().position(|x| x == &ans)?;
    Some(options_raw[idx].format_id.clone())
}

/// 提供使用者選擇要下載的語言
pub fn select_subtitles(available_langs: &[String]) -> Vec<String> {
    let mut options = Vec::new();
    if available_langs
        .iter()
        .any(|l| l.contains("zh") || l.contains("chi"))
    {
        options.push("1. 中文 (正體/簡體/彈幕)");
    }
    if available_langs.iter().any(|l| l.starts_with("en")) {
        options.push("2. 英文 (English)");
    }
    if available_langs
        .iter()
        .any(|l| l.starts_with("ja") || l.starts_with("jpn"))
    {
        options.push("3. 日文 (日本語)");
    }
    if options.is_empty() {
        return vec![];
    }

    let render_config = RenderConfig::default()
        .with_selected_checkbox(Styled::new("[✓]"))
        .with_unselected_checkbox(Styled::new("[ ]"));

    let ans = MultiSelect::new(
        "[Config] 偵測到可用字幕，請選擇要保留的語言 (按【空白鍵 Space】勾選，【Enter】確認)：",
        options,
    )
    .with_render_config(render_config)
    .prompt()
    .unwrap_or_default();

    let mut selected_langs = Vec::new();
    for a in ans {
        match a {
            "1. 中文 (正體/簡體/彈幕)" => selected_langs.extend(vec![
                "zh-Hant".into(),
                "zh-TW".into(),
                "zh-HK".into(),
                "zh-Hans".into(),
                "zh".into(),
                "chi".into(),
                "danmaku".into(),
            ]),
            "2. 英文 (English)" => {
                selected_langs.extend(vec!["en".into(), "en-US".into(), "en-GB".into()])
            }
            "3. 日文 (日本語)" => selected_langs.extend(vec!["ja".into(), "jpn".into()]),
            _ => {}
        }
    }
    selected_langs
}

/// 發生錯誤時的攔截選單
pub fn prompt_error_recovery(fail_count: usize) -> ErrorRecoveryChoice {
    println!("\n=================================================");
    println!(
        "[Warning] 偵測到 {} 個任務下載失敗 (可能因權限或年齡限制)",
        fail_count
    );
    let options = vec![
        "1. 自動套用瀏覽器 Cookie (推薦，可破解年齡限制)",
        "2. 自行匯入 Cookie 檔案",
        "3. 放棄失敗項目並結束程式",
    ];

    let selection = InquireSelect::new("請問要如何處理失敗的任務？", options)
        .prompt()
        .unwrap_or_else(|_| "3. 放棄失敗項目並結束程式");

    match selection {
        "1. 自動套用瀏覽器 Cookie (推薦，可破解年齡限制)" => {
            ErrorRecoveryChoice::Browser
        }
        "2. 自行匯入 Cookie 檔案" => ErrorRecoveryChoice::Manual,
        _ => ErrorRecoveryChoice::Abort,
    }
}

/// 選擇要爬取的瀏覽器
pub fn select_browser(configured_browsers: &[String]) -> String {
    if configured_browsers.is_empty() {
        return "chrome".to_string();
    }

    let ans = InquireSelect::new(
        "[Config] 請選擇您有登入該網站帳號的瀏覽器：",
        configured_browsers.to_vec(),
    )
    .prompt()
    .unwrap_or_else(|_| configured_browsers[0].clone());

    ans
}
