use crate::args::MediaType;
use crate::parser::VideoFormat;
use anyhow::{Context, Result};
use inquire::{
    ui::{RenderConfig, Styled},
    MultiSelect, Select as InquireSelect, Text,
};

/// 🛠️ 定義錯誤恢復的選擇項目列舉
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorRecoveryChoice {
    Browser,
    Manual,
    Abort,
}

/// 📥 1. 互動式取得使用者輸入 (網址、下載類型、輸出格式)
/// 支援多連結批量輸入，格式統一且提示友善。
pub fn get_user_input(args: &crate::args::Args) -> Result<(Vec<String>, u8, String)> {
    // A. 取得網址 (若 CLI 參數未提供，則彈出互動提示)
    let urls = match &args.url {
        Some(u) => u.clone(),
        None => {
            let prompt_msg = "🔗 請貼上影片或播放清單網址 (多個網址請以「空白」隔開)：";
            let input = Text::new(prompt_msg)
                .with_help_message("範例：https://youtube.com/... https://bilibili.com/...")
                .prompt()
                .context("讀取網址輸入失敗")?;

            input
                .split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        }
    };

    if urls.is_empty() {
        anyhow::bail!("❌ 錯誤：未輸入任何有效的網址。");
    }

    // B. 取得下載類型 (音訊、無聲、有聲)
    let media_type_enum = match args.media_type {
        Some(t) => t,
        None => {
            let options = vec![
                "🎥 有聲影片 (預設最高畫質影音合併 ✨ 一般觀看最推薦)",
                "🎧 純音訊下載 (自動提取高音質並轉檔為 MP3/M4A)",
                "🔕 無聲影片素材 (僅下載影像軌道，不保留聲音)",
            ];

            let selection = InquireSelect::new("🎬 請選擇您要下載的媒體類型：", options)
                .with_starting_cursor(0)
                .prompt()
                .context("選擇下載類型失敗")?;

            if selection.starts_with("🎥") {
                MediaType::Video
            } else if selection.starts_with("🎧") {
                MediaType::Audio
            } else {
                MediaType::VideoOnly
            }
        }
    };

    // C. 取得封裝格式
    let format = match &args.format {
        Some(f) => f.to_lowercase(),
        None => {
            let formats = match media_type_enum {
                MediaType::Audio => vec![
                    "m4a (✨ Apple 裝置與通用 AAC 編碼推薦，速度極快且無損)",
                    "mp3 (相容性最佳的傳統音訊格式，預設高位元率 320k)",
                ],
                _ => vec![
                    "mp4 (✨ 高相容性：強制鎖定 H.264/AAC 編碼，保證所有裝置流暢播放)",
                    "mkv (極致畫質：解鎖 4K/8K 與多軌高壓縮編碼，如 AV1/VP9)",
                ],
            };

            let selection = InquireSelect::new("📦 請選擇下載的輸出封裝格式：", formats)
                .with_starting_cursor(0)
                .prompt()
                .context("選擇輸出格式失敗")?;

            // 擷取前置名稱（如 "mp4", "mkv", "mp3", "m4a"）
            selection
                .split_whitespace()
                .next()
                .unwrap_or("mp4")
                .to_string()
        }
    };

    Ok((urls, media_type_enum as u8, format))
}

/// 🔍 2. 智慧高畫質解析度手動選單
/// 將晦澀的編碼去技術化，幫助大眾使用者依據情境做出直覺選擇。
pub fn select_resolution(formats: &[VideoFormat]) -> Option<String> {
    // 篩選畫質大於 1080p 的進階選項
    let mut high_res_options: Vec<&VideoFormat> = formats.iter().filter(|f| f.height > 1080).collect();
    if high_res_options.is_empty() {
        return None;
    }

    // 將最優的 1080p 本身也加入作為基準線
    if let Some(fhd) = formats
        .iter()
        .filter(|f| f.height <= 1080)
        .max_by_key(|f| f.height)
    {
        high_res_options.push(fhd);
    }

    // 依高度排序並去重
    high_res_options.sort_by(|a, b| b.height.cmp(&a.height));
    high_res_options.dedup_by(|a, b| a.height == b.height);

    // 建構白話且去技術化的顯示字串
    let display_options: Vec<String> = high_res_options
        .iter()
        .map(|f| {
            let readable_name = match f.height {
                4320 => "8K Ultra HD (極致震撼畫質)",
                2160 => "4K Ultra HD (精細超高解析度)",
                1440 => "2K Quad HD (細緻 2K 畫質)",
                1080 => "1080p Full HD (標準主流高畫質)",
                _ => "High Definition (高清規格)",
            };
            format!(
                "📺 {:<30} [格式: {}, 編碼: {}]",
                readable_name, f.ext, f.vcodec
            )
        })
        .collect();

    let ans = InquireSelect::new(
        "✨ 偵測到有高畫質流可用！請為 MKV 封裝選擇目標解析度：",
        display_options.clone(),
    )
    .prompt()
    .ok()?;

    let idx = display_options.iter().position(|x| x == &ans)?;
    Some(high_res_options[idx].format_id.clone())
}

/// 💬 3. 字幕與彈幕軌道複選引導
/// 貼心標註熱鍵引導，完美驅動 FFmpeg 字幕無損封裝軌道。
pub fn select_subtitles(available_langs: &[String]) -> Vec<String> {
    let mut options = Vec::new();

    // 智慧特徵分類
    if available_langs
        .iter()
        .any(|l| l.contains("zh") || l.contains("chi") || l.contains("danmaku"))
    {
        options.push("🇨🇳 中文語系 (包含繁體、簡體與 CJK 影片彈幕軌道)");
    }
    if available_langs.iter().any(|l| l.starts_with("en")) {
        options.push("🇺🇸 英文語系 (English Subtitles)");
    }
    if available_langs
        .iter()
        .any(|l| l.starts_with("ja") || l.starts_with("jpn"))
    {
        options.push("🇯🇵 日文語系 (日本語歌詞/字幕)");
    }

    if options.is_empty() {
        return vec![];
    }

    // 統一渲染樣式，提示鍵盤動作
    let render_config = RenderConfig::default()
        .with_selected_checkbox(Styled::new("[✓]").with_fg(inquire::ui::Color::LightGreen))
        .with_unselected_checkbox(Styled::new("[ ]"));

    let ans = MultiSelect::new(
        "✨ 偵測到可提取的字幕/彈幕軌道！請勾選欲嵌入影片的語言：",
        options,
    )
    .with_help_message("💡 按【空白鍵 Space】可勾選或取消，按【確認鍵 Enter】完成並開始下載")
    .with_render_config(render_config)
    .prompt()
    .unwrap_or_default();

    let mut selected_langs = Vec::new();
    for a in ans {
        if a.starts_with("🇨🇳") {
            selected_langs.extend(vec![
                "zh-Hant".into(),
                "zh-TW".into(),
                "zh-HK".into(),
                "zh-Hans".into(),
                "zh".into(),
                "chi".into(),
                "danmaku".into(),
            ]);
        } else if a.starts_with("🇺🇸") {
            selected_langs.extend(vec!["en".into(), "en-US".into(), "en-GB".into()]);
        } else if a.starts_with("🇯🇵") {
            selected_langs.extend(vec!["ja".into(), "jpn".into()]);
        }
    }

    selected_langs
}

/// ⚠️ 4. 下載錯誤安全攔截與恢復選單
/// 將致命錯誤降級為交互事件，防止中斷大量下載會話。
pub fn prompt_error_recovery(fail_count: usize) -> ErrorRecoveryChoice {
    println!("\n=======================================================");
    println!("⚠️ 偵測到 {} 個影片下載任務失敗 (可能因權限受限或年齡限制限制)", fail_count);
    println!("=======================================================");

    let options = vec![
        "🌐 自動套用瀏覽器 Cookie (✨ 推薦：自動提取常用瀏覽器憑證)",
        "📂 自行匯入實體 Cookie 檔案 (適用於無痕視窗或受限帳號)",
        "❌ 放棄失敗項目並結束本輪下載任務",
    ];

    let selection = InquireSelect::new("請問您要如何處理並重試這些失敗的任務？", options)
        .with_starting_cursor(0)
        .prompt();

    match selection {
        Ok(ans) => {
            if ans.starts_with("🌐") {
                ErrorRecoveryChoice::Browser
            } else if ans.starts_with("📂") {
                ErrorRecoveryChoice::Manual
            } else {
                ErrorRecoveryChoice::Abort
            }
        }
        Err(_) => ErrorRecoveryChoice::Abort,
    }
}

/// 🌐 5. 瀏覽器自訂列表選擇器
/// 提供大眾熟知瀏覽器的動態選擇選單。
pub fn select_browser(configured_browsers: &[String]) -> String {
    let mut display_map = Vec::new();
    for b in configured_browsers {
        let name_with_emoji = match b.as_str() {
            "chrome" => "🌐 Google Chrome",
            "firefox" => "🦊 Mozilla Firefox",
            "safari" => "🧭 Apple Safari",
            "edge" => "🌀 Microsoft Edge",
            "brave" => "🦁 Brave Browser",
            "vivaldi" => "🔺 Vivaldi",
            "opera" => "🔴 Opera",
            _ => b.as_str(),
        };
        display_map.push(name_with_emoji);
    }

    let ans = InquireSelect::new(
        "✨ 請選擇您有登入該網站帳號、且可正常觀看的瀏覽器：",
        display_map.clone(),
    )
    .with_help_message("💡 提示：系統將自動解密並引用該瀏覽器的 Session 憑證來突破下載限制。")
    .prompt();

    match ans {
        Ok(selected) => {
            let idx = display_map.iter().position(|x| x == &selected).unwrap_or(0);
            configured_browsers[idx].clone()
        }
        Err(_) => "chrome".to_string(),
    }
}

/// 📊 6. 下載完成總結報表
/// 使用標準字元邊框，並在失敗時主動導引使用者除錯。
pub fn print_summary(success: usize, fail: usize, duration: &str, path: &str) {
    println!("\n┌─────────────────────────────────────────────────────┐");
    println!("│                🎉 影音下載任務執行總結              │");
    println!("├─────────────────────────────────────────────────────┤");
    println!("│  ⏱️  總體耗時：{:<38} │", duration);
    println!(
        "│  📊  任務統計：成功 {:<4} 個 / 失敗 {:<4} 個               │",
        success, fail
    );
    println!("│  📂  儲存路徑：{:<38} │", path);
    println!("└─────────────────────────────────────────────────────┘");

    if fail > 0 {
        println!("\n💡 【溫馨提示】");
        println!("部分影片下載未成功。儲存路徑下已生成詳細的任務追蹤 Markdown 報表。");
        println!("您可以隨時打開 📝 [download_session.md] 檔案，查看具體的錯誤代碼與除錯建議！\n");
    }
}
