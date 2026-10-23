//! 一次執行的問題（Issue）、報告與清單。格式見 data-model.md §5–6。

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Code, Level};

/// 一次執行中記下的一則錯誤或警告。訊息與建議動作在建立時就從錯誤碼表渲染好。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub code: Code,
    pub level: Level,
    pub message: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Issue {
    pub fn new(code: Code, args: &[(&str, &str)]) -> Self {
        Issue {
            code,
            level: code.entry().level,
            message: code.render_message(args),
            action: code.render_action(args),
            root: None,
            path: None,
            detail: None,
        }
    }

    pub fn at(mut self, root: &str, path: &str) -> Self {
        self.root = Some(root.to_string());
        self.path = Some(path.to_string());
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    pub fn is_fatal(&self) -> bool {
        self.level == Level::Fatal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Completed,
    CompletedWithWarnings,
    Failed,
    Cancelled,
}

impl Outcome {
    /// CLI 的離開碼（REQ-0001）。
    pub fn exit_code(self) -> i32 {
        match self {
            Outcome::Completed => 0,
            Outcome::CompletedWithWarnings => 1,
            Outcome::Failed => 2,
            Outcome::Cancelled => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Outcome::Completed => "完成",
            Outcome::CompletedWithWarnings => "完成但有警告",
            Outcome::Failed => "失敗",
            Outcome::Cancelled => "已取消",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub scanned: u64,
    pub unchanged: u64,
    pub copied: u64,
    pub updated: u64,
    pub versioned: u64,
    pub verified: u64,
    pub skipped: u64,
    pub errors: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema_version: u32,
    pub run_id: String,
    pub profile: String,
    pub profile_path: String,
    pub destination: Option<String>,
    pub outcome: Outcome,
    pub started_at: String,
    pub finished_at: String,
    pub counts: Counts,
    pub bytes_copied: u64,
    pub issues: Vec<Issue>,
    /// 報告實際寫到哪裡（備份碟找不到時是本機的備用位置）。
    #[serde(skip)]
    pub written_to: Option<PathBuf>,
}

impl Report {
    pub fn summary_text(&self) -> String {
        let c = &self.counts;
        let mut s = String::new();
        s.push_str(&format!(
            "nucifraga 備份報告\r\n\r\n模板：{}\r\n",
            self.profile
        ));
        if let Some(d) = &self.destination {
            s.push_str(&format!("備份碟：{d}\r\n"));
        }
        s.push_str(&format!(
            "開始：{}\r\n結束：{}\r\n結果：{}\r\n\r\n",
            self.started_at,
            self.finished_at,
            self.outcome.label()
        ));
        s.push_str(&format!(
            "掃描 {} 個檔案：新增 {}、更新 {}、移到舊版本 {}、沒有變更 {}、略過 {}\r\n寫入 {}\r\n",
            c.scanned,
            c.copied,
            c.updated,
            c.versioned,
            c.unchanged,
            c.skipped,
            format_bytes(self.bytes_copied)
        ));
        if self.issues.is_empty() {
            s.push_str("\r\n沒有任何問題。\r\n");
        } else {
            s.push_str(&format!("\r\n問題（{} 則）：\r\n", self.issues.len()));
            for i in &self.issues {
                s.push_str(&format!(
                    "\r\n[{}] {}\r\n    建議：{}\r\n",
                    i.code, i.message, i.action
                ));
                if let Some(p) = &i.path {
                    s.push_str(&format!(
                        "    檔案：{}\\{}\r\n",
                        i.root.as_deref().unwrap_or(""),
                        p.replace('/', "\\")
                    ));
                }
                if let Some(d) = &i.detail {
                    s.push_str(&format!("    細節：{d}\r\n"));
                }
            }
        }
        s
    }

    /// 寫 `report.json` 與 `summary.txt`。資料夾不存在就建。
    pub fn write_to(&mut self, dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let json = serde_json::to_string_pretty(self).expect("報告一定序列化得了");
        fs::write(dir.join("report.json"), json)?;
        // 摘要給 Windows 的記事本看：UTF-8 加 BOM，否則舊版記事本會當成 ANSI。
        let mut summary = vec![0xEF, 0xBB, 0xBF];
        summary.extend_from_slice(self.summary_text().as_bytes());
        fs::write(dir.join("summary.txt"), summary)?;
        self.written_to = Some(dir.to_path_buf());
        Ok(())
    }
}

/// 一行一個檔案的清單（data-model.md §5），邊執行邊寫。
pub struct ManifestWriter {
    out: BufWriter<File>,
}

#[derive(Serialize)]
struct ManifestLine<'a> {
    root: &'a str,
    path: &'a str,
    size: u64,
    sha256: &'a str,
    action: &'a str,
}

impl ManifestWriter {
    pub fn create(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(ManifestWriter {
            out: BufWriter::new(File::create(path)?),
        })
    }

    pub fn record(
        &mut self,
        root: &str,
        path: &str,
        size: u64,
        sha256: &str,
        action: &str,
    ) -> std::io::Result<()> {
        let line = ManifestLine {
            root,
            path,
            size,
            sha256,
            action,
        };
        serde_json::to_writer(&mut self.out, &line)?;
        self.out.write_all(b"\n")
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.out.flush()?;
        self.out.get_ref().sync_all()
    }
}

/// 一個備份碟上的歷次報告（`_reports\*\report.json`），新的在前。讀不懂的那一份略過，不讓整個清單壞掉。
pub fn list_reports(dest: &Path, limit: usize) -> Vec<serde_json::Value> {
    let dir = crate::longpath::extended(&dest.join("_reports"));
    let mut names: Vec<String> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| crate::fsops::is_stamp(n))
        .collect();
    names.sort_by(|a, b| b.cmp(a));
    names
        .into_iter()
        .filter_map(|n| fs::read_to_string(dir.join(&n).join("report.json")).ok())
        .filter_map(|t| serde_json::from_str(&t).ok())
        .take(limit)
        .collect()
}

/// 備份碟上「有那次的資料夾、卻沒有 `report.json`」的那幾次 —— 程式沒有正常結束
/// （強制關閉、當機、拔碟）。新的在前。正在跑的那一次也長這樣，所以呼叫前先確定沒有人在跑。
pub fn unfinished_runs(dest: &Path) -> Vec<String> {
    let dir = crate::longpath::extended(&dest.join("_reports"));
    let mut names: Vec<String> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| crate::fsops::is_stamp(n) && !dir.join(n).join("report.json").exists())
        .collect();
    names.sort_by(|a, b| b.cmp(a));
    names
}

/// 台灣慣用的大小格式：1.2 GB、350 KB。
pub fn format_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_takes_level_and_text_from_the_code_table() {
        let i = Issue::new(Code::Src001, &[("path", r"D:\x")]);
        assert_eq!(i.level, Level::Error);
        assert!(i.message.contains(r"D:\x"));
        assert!(!i.action.is_empty());
    }

    #[test]
    fn bytes_are_formatted_for_humans() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
