use crate::AppConfig;
use std::process::Stdio;
use tokio::process::Command; // 改用 tokio 的非同步 Command

/// 執行下載任務的核心函式（非同步版本）
pub async fn execute_download(config: AppConfig) {
    let mut cmd = Command::new("yt-dlp");

    // 1. 載入基本參數
    cmd.args(config.base_args);

    // 2. 處理路徑選項：使用 dl-media 的隔離暫存目錄設計
    cmd.arg("-P").arg(&config.download_dir);
    cmd.arg("-P").arg(format!("temp:{}", config.temp_dir));

    // 3. 處理字幕與封面選項
    if config.need_subs {
        cmd.arg("--write-subs");
        cmd.arg("--write-auto-subs");
    }
    if config.need_thumbnail {
        cmd.arg("--embed-thumbnail");
    }

    // 4. 載入網址 (這裡每次只會傳入單一網址)
    cmd.args(config.urls);

    // 5. 設定輸出 (目前直接印到終端機，進度條會並行顯示)
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    // 6. 啟動並「非同步等待」執行結果
    match cmd.spawn() {
        Ok(mut child) => match child.wait().await {
            Ok(status) => {
                if !status.success() {
                    eprintln!("\n⚠️ 提醒：任務似乎發生了錯誤或有部分檔案遺失。");
                }
            }
            Err(e) => eprintln!("\n❌ 系統錯誤：等待 yt-dlp 執行結束時發生異常: {}", e),
        },
        Err(e) => eprintln!("\n❌ 啟動失敗：無法呼叫 yt-dlp 指令: {}", e),
    }

    // 7. 任務結束：優雅清理專屬暫存目錄 (無論成功或失敗都會執行)
    let _ = std::fs::remove_dir_all(&config.temp_dir);
}
