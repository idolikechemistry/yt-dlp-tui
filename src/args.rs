use clap::{Parser, ValueEnum};

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum MediaType {
    /// 🎧 純音訊下載
    #[value(alias = "1")]
    Audio = 1,

    /// 🔕 無聲影片下載
    #[value(alias = "2")]
    VideoOnly = 2,

    /// 🎥 有聲影片下載
    #[value(alias = "3")]
    Video = 3,
}

#[derive(Parser, Debug)]
#[command(
    name = "yt-dlp-tui",
    version = env!("CARGO_PKG_VERSION"),
    about = "簡化 yt-dlp 複雜參數的雙模下載器，自動合併多國字幕與 Bilibili 彈幕，並支援自動 Cookie 重試。",
    long_about = None
)]
pub struct Args {
    /// 貼上要下載的影片或播放清單網址 (支援多個，以空格隔開)
    #[arg(short, long, num_args = 1..)]
    pub url: Option<Vec<String>>,

    /// 指定下載類型 (1:音訊, 2:無聲影片, 3:有聲影片)
    #[arg(short, long, value_enum)]
    pub media_type: Option<MediaType>,

    /// 指定輸出格式 (音訊: mp3/m4a, 影片: mp4/mkv)
    #[arg(short, long)]
    pub format: Option<String>,

    /// 指定輸出路徑 (預設為系統 Downloads 資料夾)
    #[arg(short, long)]
    pub output: Option<String>,

    /// 手動指定特定的 Cookie 實體檔案路徑
    #[arg(short, long)]
    pub cookie: Option<String>,

    /// 🛠️ 開啟互動式設定選單，配置下載資料夾與自訂瀏覽器列表
    #[arg(long)]
    pub config: bool,

    /// 🍪 強制調用設定資料夾中已儲存的 Cookie 檔案 (不進行受限內容偵測)
    #[arg(long = "fc", alias = "force-cookie")]
    pub force_cookie: bool,

    /// 📁 開啟專屬的設定與 Cookie 儲存資料夾路徑 (便於放入實體 cookie.txt)
    #[arg(long = "open-config")]
    pub open_config: bool,

    /// 🔄 檢查並自動更新至最新版本
    #[arg(long)]
    pub update: bool,

    /// 🎯 產生 Shell 自動補全腳本 (設定 hide = true 讓它不在一般 --help 中顯示)
    #[arg(long = "generate-completion", value_enum, hide = true)]
    pub generator: Option<clap_complete::Shell>,
}

impl Args {
    /// 檢查是否有提供網址、類型與格式 (用於判斷是否進入自動化模式)
    pub fn is_fully_automated(&self) -> bool {
        self.url.is_some() && self.media_type.is_some() && self.format.is_some()
    }

    /// 驗證參數邏輯是否合法
    pub fn validate(&self) -> anyhow::Result<()> {
        if let (Some(mt), Some(fmt)) = (self.media_type, &self.format) {
            let fmt = fmt.to_lowercase();
            match mt {
                MediaType::Audio => {
                    if fmt != "mp3" && fmt != "m4a" {
                        anyhow::bail!("❌ 格式 '{}' 與音訊類型不匹配。請使用 mp3 或 m4a。", fmt);
                    }
                }
                _ => {
                    if fmt != "mp4" && fmt != "mkv" {
                        anyhow::bail!("❌ 格式 '{}' 與影片類型不匹配。請使用 mp4 或 mkv。", fmt);
                    }
                }
            }
        }
        Ok(())
    }
}
