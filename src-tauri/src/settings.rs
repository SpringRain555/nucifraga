//! app 設定 `%APPDATA%\<identifier>\settings.json`：只記「模板在哪」，不複製模板內容（ADR-0006）。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "one")]
    pub schema_version: u32,
    #[serde(default)]
    pub profiles: Vec<ProfileRef>,
}

fn one() -> u32 {
    1
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
                profiles: Vec::new(),
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
                        profiles: Vec::new(),
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
