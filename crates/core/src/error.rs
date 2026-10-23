//! 錯誤碼。
//!
//! **訊息與建議動作的正本是 `docs/architecture/error-codes.md`**：這裡在編譯時把那份表
//! `include_str!` 進來解析，所以使用者看到的字只有一份。Rust 這邊只定義碼的集合（`Code`），
//! 測試會檢查兩邊的集合相同 —— 新增一碼要兩邊一起改，少改一邊測試就紅。

use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;

use serde::{Serialize, Serializer};

const TABLE: &str = include_str!("../../../docs/architecture/error-codes.md");

/// 一則問題的嚴重度。意思見 error-codes.md 開頭的表。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Fatal,
    Error,
    Warning,
    Info,
}

macro_rules! codes {
    ($($variant:ident => $text:literal),* $(,)?) => {
        /// 所有錯誤碼。字串形式（`"DST-001"`）就是文件、報告與 UI 上看到的樣子。
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Code { $($variant),* }

        impl Code {
            pub const ALL: &'static [Code] = &[$(Code::$variant),*];

            pub fn as_str(self) -> &'static str {
                match self { $(Code::$variant => $text),* }
            }
        }
    };
}

codes! {
    Cfg001 => "CFG-001", Cfg002 => "CFG-002", Cfg003 => "CFG-003", Cfg004 => "CFG-004",
    Cfg005 => "CFG-005", Cfg006 => "CFG-006", Cfg007 => "CFG-007", Cfg008 => "CFG-008",
    Dst001 => "DST-001", Dst002 => "DST-002", Dst003 => "DST-003", Dst004 => "DST-004",
    Dst005 => "DST-005", Dst006 => "DST-006", Dst007 => "DST-007", Dst008 => "DST-008",
    Src001 => "SRC-001", Src002 => "SRC-002", Src003 => "SRC-003", Src004 => "SRC-004",
    Src005 => "SRC-005", Src006 => "SRC-006", Src007 => "SRC-007",
    Path001 => "PATH-001", Path002 => "PATH-002", Path003 => "PATH-003",
    Eng001 => "ENG-001", Eng002 => "ENG-002", Eng003 => "ENG-003", Eng004 => "ENG-004",
    Eng010 => "ENG-010",
    Phn001 => "PHN-001", Phn002 => "PHN-002", Phn003 => "PHN-003", Phn004 => "PHN-004",
    Phn005 => "PHN-005",
    App001 => "APP-001", App002 => "APP-002", App003 => "APP-003", App004 => "APP-004",
    App005 => "APP-005", App006 => "APP-006",
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Code {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// 錯誤碼表裡的一列。字串直接指向編譯進來的文件，不另外配置。
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub level: Level,
    pub message: &'static str,
    pub action: &'static str,
}

fn table() -> &'static HashMap<&'static str, Entry> {
    static PARSED: OnceLock<HashMap<&'static str, Entry>> = OnceLock::new();
    PARSED.get_or_init(|| parse_table(TABLE))
}

/// 解析 `| \`XXX-NNN\` | 層級 | 訊息 | 建議動作 |` 這種列；其他列一律忽略。
fn parse_table(doc: &'static str) -> HashMap<&'static str, Entry> {
    let mut map = HashMap::new();
    for line in doc.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let Some((code, rest)) = rest.split_once("` |") else {
            continue;
        };
        let cells: Vec<&str> = rest.split(" | ").map(str::trim).collect();
        if cells.len() < 3 {
            continue;
        }
        let action = cells[2].trim_end_matches('|').trim();
        map.insert(
            code,
            Entry {
                level: parse_level(cells[0]),
                message: cells[1],
                action,
            },
        );
    }
    map
}

/// 層級欄以英文字開頭，後面可以有註解（例：`warning（pathPolicy: block 時 fatal）`）。
fn parse_level(cell: &str) -> Level {
    if cell.starts_with("fatal") {
        Level::Fatal
    } else if cell.starts_with("error") {
        Level::Error
    } else if cell.starts_with("warning") {
        Level::Warning
    } else {
        Level::Info
    }
}

impl Code {
    /// 這一碼在 error-codes.md 裡的那一列。測試保證每一碼都有。
    pub fn entry(self) -> Entry {
        table()[self.as_str()]
    }

    /// 把訊息裡的 `{name}` 換成實際的值。沒給的佔位符原樣留著，比悄悄變成空字串好查。
    pub fn render_message(self, args: &[(&str, &str)]) -> String {
        render(self.entry().message, args)
    }

    pub fn render_action(self, args: &[(&str, &str)]) -> String {
        render(self.entry().action, args)
    }
}

fn render(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.replace('`', "");
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn code_set_matches_the_document() {
        let in_code: BTreeSet<&str> = Code::ALL.iter().map(|c| c.as_str()).collect();
        let in_doc: BTreeSet<&str> = table().keys().copied().collect();
        let only_code: Vec<_> = in_code.difference(&in_doc).collect();
        let only_doc: Vec<_> = in_doc.difference(&in_code).collect();
        assert!(
            only_code.is_empty() && only_doc.is_empty(),
            "錯誤碼與 docs/architecture/error-codes.md 不一致。只在程式裡：{only_code:?}；只在文件裡：{only_doc:?}"
        );
    }

    #[test]
    fn every_entry_has_a_message_and_an_action() {
        for code in Code::ALL {
            let e = code.entry();
            assert!(!e.message.is_empty(), "{code} 沒有訊息");
            assert!(!e.action.is_empty(), "{code} 沒有建議動作");
        }
    }

    #[test]
    fn levels_are_read_from_the_document() {
        assert_eq!(Code::Dst001.entry().level, Level::Fatal);
        assert_eq!(Code::Dst004.entry().level, Level::Error);
        assert_eq!(Code::Path001.entry().level, Level::Warning);
        assert_eq!(Code::Src006.entry().level, Level::Info);
    }

    #[test]
    fn placeholders_are_filled_and_unknown_ones_are_kept() {
        let msg = Code::Dst002.render_message(&[("need", "3.2 GB")]);
        assert!(msg.contains("3.2 GB"));
        assert!(msg.contains("{free}"), "沒給的佔位符要原樣保留：{msg}");
    }

    #[test]
    fn serializes_as_the_dashed_string() {
        assert_eq!(
            serde_json::to_string(&Code::Path003).unwrap(),
            "\"PATH-003\""
        );
    }
}
