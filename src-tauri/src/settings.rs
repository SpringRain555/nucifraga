//! app 設定 `%APPDATA%\<identifier>\settings.json`：只記「模板在哪」，不複製模板內容（ADR-0006）。
//!
//! 模板分兩種：放在模板資料夾裡的（首頁直接列出資料夾內容，不另記清單），
//! 與資料夾外的外部模板（`profiles`，逐一記路徑）。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "one")]
    pub schema_version: u32,
    /// 模板資料夾。`None` ＝ 還沒選過，首頁之前先出現「第一次使用」畫面。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiles_folder: Option<String>,
    /// 外部模板（資料夾外的模板檔）。
    #[serde(default)]
    pub profiles: Vec<ProfileRef>,
    /// 背景模式（設定頁）：預設開。舊的設定檔沒有這個欄位，讀進來也是開。
    #[serde(default = "yes")]
    pub background_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema_version: 1,
            profiles_folder: None,
            profiles: Vec::new(),
            background_mode: true,
        }
    }
}

fn one() -> u32 {
    1
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRef {
    pub path: String,
    pub added_at: String,
}

pub struct Loaded {
    pub settings: Settings,
    /// 設定檔壞掉、已改名保留（`APP-003`）。
    pub was_broken: bool,
}

pub fn load(file: &Path) -> Loaded {
    match fs::read_to_string(file) {
        Err(_) => Loaded {
            settings: Settings {
                schema_version: 1,
                ..Settings::default()
            },
            was_broken: false,
        },
        Ok(text) => match serde_json::from_str::<Settings>(&text) {
            Ok(settings) => Loaded {
                settings,
                was_broken: false,
            },
            Err(_) => {
                let stamp = time::OffsetDateTime::now_utc().unix_timestamp();
                let _ = fs::rename(file, file.with_extension(format!("broken-{stamp}.json")));
                Loaded {
                    settings: Settings {
                        schema_version: 1,
                        ..Settings::default()
                    },
                    was_broken: true,
                }
            }
        },
    }
}

pub fn save(file: &Path, s: &Settings) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    // 先寫暫存檔再改名：寫到一半當掉也不會留下半份設定檔。
    let tmp: PathBuf = file.with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_string_pretty(s).expect("設定一定序列化得了"),
    )?;
    fs::rename(tmp, file)
}
