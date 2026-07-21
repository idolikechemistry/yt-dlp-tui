use std::fs;
use std::path::PathBuf;

/// 🚀 構建 yt-dlp 的下載指令參數
/// 負責將使用者的下載設定與優化參數轉換為 yt-dlp 命令列引數陣列
pub fn build_download_args(
    media_type: u8,
    target_ext: &str,
    _input_url: &str,
    _cookie_args: &[String],
) -> Vec<String> {
    // 預設的高效能與高相容性通用參數
    let mut dl_args: Vec<String> = vec![
        "--remote-components".into(), // 允許下載遠端破解組件
        "ejs:github".into(),          // 指定來源為官方推薦的 GitHub
        "--newline".into(),           // 強制換行輸出，便於 proc.rs 中的非同步進度條解析
        "--progress".into(),          // 輸出詳細下載進度
        "--no-colors".into(),         // 禁用 ANSI 著色，防止控制台控制字元干擾進度條解析
        "--no-warnings".into(),       // 忽略非致命警告
        "--ignore-errors".into(),     // 遇到播放清單中個別失敗項目時不中斷
        "--no-overwrites".into(),     // 不覆蓋同名檔案
        "--embed-thumbnail".into(),   // 嵌入影片封面/縮圖
        "--embed-metadata".into(),    // 嵌入媒體中介資料 (Metadata)
        "--embed-chapters".into(),    // 嵌入章節資訊
        "--convert-thumbnails".into(), // 自動將縮圖轉為相容性佳的 JPG 格式
        "jpg".into(),
        "--restrict-filenames".into(), // 限制檔名僅使用安全字元，防範不同作業系統讀取異常
        "--sponsorblock-remove".into(), // 自動移除業配、片頭片尾，節省硬碟空間與封裝頻寬
        "sponsor,intro,outro".into(),
    ];

    if media_type == 1 {
        // =====================================================================
        // 🎧 純音訊下載模式
        // =====================================================================
        dl_args.extend(vec![
            "--extract-audio".into(),
            "--audio-format".into(),
            target_ext.into(),
        ]);

        if target_ext == "mp3" {
            dl_args.extend(vec![
                "--audio-quality".into(),
                "320k".into(), // MP3 固定強制最高 320k 音質
                "-f".into(),
                "bestaudio".into(),
            ]);
        } else {
            // M4A：優先抓取高品質原生 m4a 容器流，避免二次轉碼造成的損耗
            dl_args.extend(vec!["-f".into(), "bestaudio[ext=m4a]/bestaudio".into()]);
        }
    } else {
        // =====================================================================
        // 🎥 影片下載模式 (包含無聲或有聲)
        // =====================================================================
        dl_args.extend(vec!["--merge-output-format".into(), target_ext.into()]);

        if target_ext == "mkv" {
            // MKV：封裝相容性極佳，直接抓取物理最高畫質流（包含 AV1/VP9 等高效率編碼）
            dl_args.extend(vec!["-f".into(), "bv*+ba/best".into()]);
        } else {
            // MP4：專注於極致的主流硬體相容性
            // 💡 核心優化：限制視訊編碼為 H.264 (AVC) 且音訊為 AAC (m4a)
            // 這能確保 ffmpeg 封裝時僅需進行軌道複製 (copy 模式)，省去耗時耗 CPU 的解碼重編碼過程！
            dl_args.extend(vec![
                "-f".into(),
                "bv*[vcodec^=avc]+ba[ext=m4a]/best[ext=mp4]/best".into(),
            ]);
        }
    }

    dl_args
}

/// 📂 準備輸出資料夾
/// 若為播放清單，會自動調用輕量 yt-dlp 探測標題，清理非法字元並建立子資料夾
pub fn prepare_output_dir(
    base_dir: &PathBuf,
    input_url: &str,
    cookie_args: &[String],
    is_pl: bool,
) -> PathBuf {
    let mut dir = base_dir.clone();
    let _ = fs::create_dir_all(&dir);

    if is_pl {
        // 僅執行極輕量的 metadata 探測 (skip-download) 來獲取播放清單名稱
        let title = std::process::Command::new("yt-dlp")
            .args(cookie_args)
            .args([
                "--print",
                "playlist_title",
                "--no-warnings",
                "--playlist-items",
                "1", // 僅探測第一個項目，將網絡耗時壓到最低
                "--skip-download",
                input_url,
            ])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|t| !t.is_empty() && t != "NA" && t != "null")
            .unwrap_or_else(|| "Playlist".into());

        // 清理資料夾名稱中的非法安全字元，防止作業系統建立目錄失敗
        let safe_title = clean_filename(&title);
        dir = dir.join(safe_title);
        let _ = fs::create_dir_all(&dir);
    }

    dir
}

/// 🧽 輔助工具：清洗檔名中的作業系統非法/敏感字元
pub fn clean_filename(name: &str) -> String {
    name.replace(&['/', '\\', ':', '*', '?', '"', '<', '>', '|'][..], "_")
}
