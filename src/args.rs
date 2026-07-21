use clap::{Parser, ValueEnum};
use clap_complete::Shell;

/// 媒體下載類型定義
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum MediaType {
    /// 純音訊下載
    #[value(alias = "1")]
    Audio = 1,
    /// 無聲影片下載
    #[value(alias = "2")]
    VideoOnly = 2,
    /// 有聲影片下載 (最高畫質合併)
    #[value(alias = "3")]
    Video = 3,
}

/// yt-dlp-tui - 萬能終端機影音與彈幕下載器 (TUI / CLI 雙模)
#[derive(Parser, Debug)]
#[command(
    name = "yt-dlp-tui",
    version,
    about = "簡化 yt-dlp 複雜參數的雙模下載器，自動合併多國字幕與 Bilibili 彈幕，並支援自動 Cookie 重試。",
    long_about = "yt-dlp-tui 是一個為大眾與進階使用者設計的影音下載工具。它在未提供完整參數時會自動開啟直覺、友善的互動選單 (TUI)；在提供網址、類型、格式參數時，則進入完全自動化的靜默指令模式，非常適合用於排程指令碼 (Cron job) 或自動化腳本。"
)]
pub struct Args {
    /// 貼上要下載的影片或播放清單網址 (支援多個，以空格隔開)
    #[arg(short, long, num_args = 1.., value_name = "URL")]
    pub url: Option<Vec<String>>,

    /// 指定下載類型 (1:音訊, 2:無聲影片, 3:有聲影片)
    #[arg(short, long, value_enum, value_name = "TYPE")]
    pub media_type: Option<MediaType>,

    /// 指定輸出格式 (音訊: mp3/m4a, 影片: mp4/mkv)
    #[arg(short, long, value_name = "FORMAT")]
    pub format: Option<String>,

    /// 指定輸出路徑 (預設為系統 Downloads 資料夾)
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<String>,

    /// 手動指定特定的 Cookie 實體檔案路徑
    #[arg(short, long, value_name = "COOKIE_FILE")]
    pub cookie: Option<String>,

    /// 🛠️ 開啟互動式設定選單，配置下載資料夾與自訂瀏覽器列表
    #[arg(long, alias = "config")]
    pub config: bool,

    /// 🍪 強制調用設定資料夾中已儲存的 Cookie 檔案 (不進行受限內容偵測)
    #[arg(long = "fc", alias = "force-cookie")]
    pub force_cookie: bool,

    /// 📁 開啟專屬的設定與 Cookie 儲存資料夾路徑 (便於放入實體 cookie.txt)
    #[arg(long = "open-config")]
    pub open_config: bool,

    /// 🎯 產生 Shell 自動補全腳本 (支援 bash, zsh, fish, powershell，此選項預設隱藏)
    #[arg(long = "generate-completion", value_enum, hide = true)]
    pub generator: Option<Shell>,
}

impl Args {
    /// 檢查是否有提供網址、類型與格式 (用於判斷是否進入全自動靜默模式)
    pub fn is_fully_automated(&self) -> bool {
        self.url.is_some() && self.media_type.is_some() && self.format.is_some()
    }

    /// 驗證命令列參數的內在邏輯是否合法，防範使用者傳入不匹配的格式
    pub fn validate(&self) -> anyhow::Result<()> {
        if let (Some(mt), Some(fmt)) = (self.media_type, &self.format) {
            let fmt = fmt.to_lowercase();
            match mt {
                MediaType::Audio => {
                    if fmt != "mp3" && fmt != "m4a" {
                        anyhow::bail!(
                            "❌ 參數不匹配：當下載類型為【音訊】時，格式只能指定為 mp3 或 m4a，不支援「{}」。",
                            fmt
                        );
                    }
                }
                _ => {
                    if fmt != "mp4" && fmt != "mkv" {
                        anyhow::bail!(
                            "❌ 參數不匹配：當下載類型為【影片】時，格式只能指定為 mp4 或 mkv，不支援「{}」。",
                            fmt
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
