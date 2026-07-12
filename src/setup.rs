use std::env::consts::OS;
use std::process::Command;

/// 核心環境檢查函式：檢查所有必要與建議的依賴套件。
/// 如果缺少關鍵套件，會印出錯誤提示與安裝指南，並回傳 `false`。
pub fn check_environment() -> bool {
    let mut all_passed = true;

    println!("========================================");
    println!("           核心依賴套件檢查              ");
    println!("========================================");

    // 1. 檢查 Python 環境 (yt-dlp 執行基礎)
    match check_command("python3", &["--version"]) {
        Ok(output) => println!("✅ Python3 已安裝: {}", output.trim()),
        Err(_) => match check_command("python", &["--version"]) {
            Ok(output) => println!("✅ Python 已安裝: {}", output.trim()),
            Err(_) => {
                eprintln!("❌ 錯誤: 系統未偵測到 Python 3.10 或以上版本！");
                print_install_hint("python");
                all_passed = false;
            }
        },
    }

    // 2. 檢查 yt-dlp
    match check_command("yt-dlp", &["--version"]) {
        Ok(version) => println!("✅ yt-dlp 已安裝 (版本: {})", version.trim()),
        Err(_) => {
            eprintln!("❌ 錯誤: 找不到 `yt-dlp` 執行檔！");
            print_install_hint("yt-dlp");
            all_passed = false;
        }
    }

    println!("\n========================================");
    println!("         後處理與功能依賴檢查            ");
    println!("========================================");

    // 3. 檢查 ffmpeg
    let has_ffmpeg = match check_command("ffmpeg", &["-version"]) {
        Ok(_) => {
            println!("✅ ffmpeg 已安裝 (影音合併與封面/字幕功能就緒)");
            true
        }
        Err(_) => {
            eprintln!("⚠️ 警告: 找不到 `ffmpeg`！");
            print_install_hint("ffmpeg");
            false
        }
    };

    // 4. 檢查 ffprobe (通常與 ffmpeg 綁定)
    match check_command("ffprobe", &["-version"]) {
        Ok(_) => println!("✅ ffprobe 已安裝"),
        Err(_) => {
            if has_ffmpeg {
                eprintln!(
                    "⚠️ 警告: 找到 ffmpeg 但找不到 `ffprobe`！部分進階 Metadata 分析功能可能會受限。"
                );
            }
        }
    }

    println!("========================================");

    if !all_passed {
        eprintln!("\n🚨 啟動失敗：請依照上述提示安裝缺少的核心套件後，重新執行本程式。");
        return false;
    }

    if !has_ffmpeg {
        println!("\n💡 提示：建議安裝 ffmpeg 以解鎖最高畫質合併、字幕與封面嵌入等完整功能。");
    }

    true
}

/// 輔助函式：根據作業系統提供對應的安裝指令提示
fn print_install_hint(tool: &str) {
    println!("   👉 安裝建議：");
    match tool {
        "python" => match OS {
            "macos" => println!("      請在終端機執行: brew install python"),
            "windows" => println!(
                "      請至 Microsoft Store 搜尋安裝 Python 3.10 以上版本，或至 python.org 下載安裝檔。"
            ),
            _ => println!("      Ubuntu/Debian 請執行: sudo apt install python3"),
        },
        "yt-dlp" => {
            println!("      yt-dlp 支援多種安裝方式（二進位檔、pip 或第三方套件管理員）。");
            match OS {
                "macos" => println!("      請在終端機執行: brew install yt-dlp"),
                "windows" => {
                    println!("      如果已安裝 Python，請開啟 PowerShell 執行: pip install yt-dlp")
                }
                _ => println!("      Linux 如果已安裝 Python，請執行: pip3 install yt-dlp"),
            }
            println!("      詳細安裝方式可參考官方 wiki。");
        }
        "ffmpeg" => {
            match OS {
                "macos" => println!("      請在終端機執行: brew install ffmpeg"),
                "windows" => println!(
                    "      建議至官方推薦的 yt-dlp/FFmpeg-Builds 專案下載預先編譯好的 Windows 版本並加入環境變數 PATH 中。"
                ),
                _ => println!("      Ubuntu/Debian 請執行: sudo apt install ffmpeg"),
            }
            println!(
                "      !! 注意 !! 您需要的是 ffmpeg 系統二進位執行檔，絕對不是名為 ffmpeg 的 Python 套件。"
            );
        }
        _ => {}
    }
    println!(); // 留白換行讓版面乾淨
}

/// 輔助函式：透過執行特定指令與參數來測試工具是否存在
fn check_command(cmd: &str, args: &[&str]) -> Result<String, std::io::Error> {
    let output = Command::new(cmd).args(args).output()?;

    if output.status.success() {
        let stdout_str = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(stdout_str)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "指令執行回傳失敗狀態碼",
        ))
    }
}
