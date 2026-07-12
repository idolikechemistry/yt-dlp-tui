use crate::AppConfig;
use std::process::{Command, Stdio};

pub fn execute_download(config: AppConfig) {
    let mut cmd = Command::new("yt-dlp");

    // 1. 載入基本參數
    cmd.args(config.base_args);

    // 2. 處理路徑選項 (重要)
    // 透過 -P 參數分別指定主下載目錄與獨立的暫存目錄
    cmd.arg("-P").arg(&config.download_dir);
    cmd.arg("-P").arg(format!("temp:{}", config.temp_dir)); // 將所有中間檔限制於 UUID 專屬資料夾內

    // 3. 處理字幕與封面選項
    if config.need_subs {
        cmd.arg("--write-subs");
        cmd.arg("--write-auto-subs");
    }
    if config.need_thumbnail {
        cmd.arg("--embed-thumbnail");
    }

    // 4. 載入網址
    cmd.args(config.urls);

    // 5. 設定標準輸出並啟動
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    match cmd.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) => {
                if status.success() {
                    println!("\n🎉 恭喜！所有下載任務已順利完成！");
                } else {
                    eprintln!("\n⚠️ 提醒：下載過程似乎發生了錯誤或有部分檔案遺失。");
                }
            }
            Err(e) => eprintln!("\n❌ 系統錯誤：等待 yt-dlp 執行結束時發生異常: {}", e),
        },
        Err(e) => eprintln!("\n❌ 啟動失敗：無法呼叫 yt-dlp 指令: {}", e),
    }

    // 6. 任務結束：優雅清理專屬暫存目錄
    // 無論下載成功或失敗，直接將該 UUID 資料夾整個拔掉，確保不留垃圾
    let _ = std::fs::remove_dir_all(&config.temp_dir);
}
