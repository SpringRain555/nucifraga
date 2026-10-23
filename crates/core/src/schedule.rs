//! 排程（ADR-0010）：Windows 工作排程器在指定時間**打開 app**（`--run <模板>`），備份在畫面上跑。
//!
//! 排程設定存在模板裡（`nucifraga.schedule`），跟著模板走；工作排程器裡的工作是它的投影：
//! app 每次啟動都依模板重建 `\nucifraga\` 底下的工作，對不到模板的移除（[`reconcile`]）。
//! 這個資料夾只放 nucifraga 自己建的工作，名稱由模板路徑雜湊而來，不碰別的。

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Code;
use crate::report::Issue;

/// 模板裡的 `nucifraga.schedule`。`days` 空的＝每天。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    /// `HH:MM`，24 小時制，本機時間。
    pub time: String,
    /// `mon`…`sun`；空的＝每天。
    #[serde(default)]
    pub days: Vec<String>,
}

/// 模板裡的寫法 → 工作排程器 XML 的元素名。
pub const DAYS: [(&str, &str); 7] = [
    ("mon", "Monday"),
    ("tue", "Tuesday"),
    ("wed", "Wednesday"),
    ("thu", "Thursday"),
    ("fri", "Friday"),
    ("sat", "Saturday"),
    ("sun", "Sunday"),
];

/// 工作排程器裡 nucifraga 專用的資料夾。
pub const TASK_FOLDER: &str = r"\nucifraga\";

impl Schedule {
    /// 回傳不合法的原因（給 `CFG-007`）。
    pub fn validate(&self) -> Result<(), String> {
        let ok_time = self.time.len() == 5
            && self.time.as_bytes()[2] == b':'
            && self.time[..2].parse::<u8>().is_ok_and(|h| h < 24)
            && self.time[3..].parse::<u8>().is_ok_and(|m| m < 60);
        if !ok_time {
            return Err(format!(
                "時間要寫成 HH:MM（24 小時制），現在是「{}」",
                self.time
            ));
        }
        if let Some(bad) = self.days.iter().find(|d| !DAYS.iter().any(|(k, _)| k == d)) {
            return Err(format!("星期要寫成 mon…sun，現在有「{bad}」"));
        }
        Ok(())
    }
}

/// 一份模板對應的工作名稱：`\nucifraga\<模板路徑的雜湊>`。路徑不分大小寫（Windows）。
pub fn task_name(profile_file: &Path) -> String {
    let lower = profile_file.to_string_lossy().to_lowercase();
    let hash = Sha256::digest(lower.as_bytes());
    let hex: String = hash.iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("{TASK_FOLDER}{hex}")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 工作排程器的 XML（Task Scheduler schema 1.2）。`start_date` 是 `YYYY-MM-DD`：從那天起生效。
///
/// - 以登入中的使用者身分、一般權限執行（`InteractiveToken`、`LeastPrivilege`）：要開得出視窗
/// - 錯過的（電腦關著）開機後補跑（`StartWhenAvailable`）；用電池也跑；不限執行時間
/// - 已經在跑就不再開一個（`IgnoreNew`）—— app 自己也是單一實例
pub fn task_xml(
    s: &Schedule,
    title: &str,
    exe: &Path,
    profile_file: &Path,
    start_date: &str,
) -> String {
    let trigger = if s.days.is_empty() {
        "<ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>".to_string()
    } else {
        let days: String = DAYS
            .iter()
            .filter(|(k, _)| s.days.iter().any(|d| d == k))
            .map(|(_, v)| format!("<{v} />"))
            .collect();
        format!(
            "<ScheduleByWeek><DaysOfWeek>{days}</DaysOfWeek><WeeksInterval>1</WeeksInterval></ScheduleByWeek>"
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>{desc}</Description>
  </RegistrationInfo>
  <Triggers>
    <CalendarTrigger>
      <StartBoundary>{start_date}T{time}:00</StartBoundary>
      <Enabled>true</Enabled>
      {trigger}
    </CalendarTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>--run "{profile}"</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        desc = xml_escape(&format!("nucifraga 排程備份：{title}")),
        time = s.time,
        exe = xml_escape(&exe.display().to_string()),
        profile = xml_escape(&profile_file.display().to_string()),
    )
}

/// 從 `--run <模板>` 這種命令列參數拿出模板路徑（排程開 app、或第二次啟動交棒時）。
pub fn run_request<I, S>(args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a.as_ref() == "--run" {
            return it.next().map(|p| p.as_ref().to_string());
        }
    }
    None
}

fn schedule_issue(detail: impl std::fmt::Display) -> Issue {
    Issue::new(Code::App007, &[("detail", &detail.to_string())])
}

/// 一份要排的模板。
pub struct Wanted<'a> {
    pub profile_file: &'a Path,
    pub title: &'a str,
    pub schedule: &'a Schedule,
}

#[cfg(windows)]
mod os {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Output};

    use super::*;

    /// 不要讓 schtasks 閃出一個主控台視窗。
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn schtasks(args: &[&str]) -> std::io::Result<Output> {
        Command::new("schtasks.exe")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    }

    fn failed(o: &Output) -> String {
        // schtasks 的訊息是系統語系的編碼（中文 Windows 是 Big5），這裡只求看得到離開碼與大概。
        let text = String::from_utf8_lossy(&o.stderr);
        format!(
            "schtasks 離開碼 {}：{}",
            o.status.code().unwrap_or(-1),
            text.trim()
        )
    }

    /// 建立或取代一份模板的工作。
    pub fn install(w: &Wanted, exe: &Path, start_date: &str) -> Result<(), Issue> {
        let xml = task_xml(w.schedule, w.title, exe, w.profile_file, start_date);
        // schtasks /XML 要 UTF-16（含 BOM）。暫存檔名含亂數，用完就刪這一個檔案。
        let tmp = std::env::temp_dir().join(format!(
            "nucifraga-task-{}.xml",
            uuid::Uuid::new_v4().simple()
        ));
        let mut bytes = vec![0xFF, 0xFE];
        for u in xml.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        std::fs::write(&tmp, bytes).map_err(schedule_issue)?;
        let name = task_name(w.profile_file);
        let tmp_text = tmp.display().to_string();
        let out = schtasks(&["/Create", "/TN", &name, "/XML", &tmp_text, "/F"]);
        let _ = std::fs::remove_file(&tmp);
        let out = out.map_err(schedule_issue)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(schedule_issue(failed(&out)))
        }
    }

    /// 移除一份模板的工作；本來就沒有不算錯。
    pub fn remove(profile_file: &Path) -> Result<(), Issue> {
        remove_by_name(&task_name(profile_file))
    }

    fn remove_by_name(name: &str) -> Result<(), Issue> {
        if !exists(name) {
            return Ok(());
        }
        let out = schtasks(&["/Delete", "/TN", name, "/F"]).map_err(schedule_issue)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(schedule_issue(failed(&out)))
        }
    }

    fn exists(name: &str) -> bool {
        schtasks(&["/Query", "/TN", name]).is_ok_and(|o| o.status.success())
    }

    /// `\nucifraga\` 底下現有的工作名稱。
    pub fn installed() -> Vec<String> {
        let Ok(out) = schtasks(&["/Query", "/FO", "CSV", "/NH"]) else {
            return Vec::new();
        };
        let mut names: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.split(',').next())
            .map(|n| n.trim_matches('"').to_string())
            .filter(|n| n.starts_with(TASK_FOLDER))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// 讓工作排程器跟模板一致：每份有排程的模板都重建它的工作，`\nucifraga\` 底下對不到模板的移除。
    pub fn reconcile(wanted: &[Wanted], exe: &Path, start_date: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let keep: Vec<String> = wanted.iter().map(|w| task_name(w.profile_file)).collect();
        for w in wanted {
            if let Err(i) = install(w, exe, start_date) {
                issues.push(i);
            }
        }
        for name in installed() {
            if !keep.contains(&name)
                && let Err(i) = remove_by_name(&name)
            {
                issues.push(i);
            }
        }
        issues
    }
}

#[cfg(windows)]
pub use os::{install, installed, reconcile, remove};

#[cfg(not(windows))]
pub fn install(_: &Wanted, _: &Path, _: &str) -> Result<(), Issue> {
    Err(schedule_issue("排程目前只支援 Windows"))
}
#[cfg(not(windows))]
pub fn remove(_: &Path) -> Result<(), Issue> {
    Ok(())
}
#[cfg(not(windows))]
pub fn installed() -> Vec<String> {
    Vec::new()
}
#[cfg(not(windows))]
pub fn reconcile(_: &[Wanted], _: &Path, _: &str) -> Vec<Issue> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(time: &str, days: &[&str]) -> Schedule {
        Schedule {
            time: time.into(),
            days: days.iter().map(|d| d.to_string()).collect(),
        }
    }

    #[test]
    fn validate_time_and_days() {
        assert!(s("21:00", &[]).validate().is_ok());
        assert!(s("00:05", &["mon", "sun"]).validate().is_ok());
        assert!(s("24:00", &[]).validate().is_err());
        assert!(s("9:00", &[]).validate().is_err());
        assert!(s("21:60", &[]).validate().is_err());
        assert!(s("21:00", &["monday"]).validate().is_err());
    }

    #[test]
    fn task_name_ignores_case_and_stays_in_our_folder() {
        let a = task_name(Path::new(r"D:\Data\整體專案備份.json"));
        let b = task_name(Path::new(r"d:\data\整體專案備份.JSON"));
        assert_eq!(a, b);
        assert!(a.starts_with(TASK_FOLDER));
        assert_ne!(a, task_name(Path::new(r"D:\Data\照片.json")));
    }

    #[test]
    fn xml_has_the_trigger_and_escapes_paths() {
        let x = task_xml(
            &s("21:30", &["wed", "mon"]),
            "A & B",
            Path::new(r"C:\Program Files\nucifraga\nucifraga.exe"),
            Path::new(r"D:\Data\a&b.json"),
            "2026-10-24",
        );
        assert!(x.contains("<StartBoundary>2026-10-24T21:30:00</StartBoundary>"));
        // 依星期順序，不依模板裡寫的順序
        assert!(x.contains("<DaysOfWeek><Monday /><Wednesday /></DaysOfWeek>"));
        assert!(x.contains("A &amp; B"));
        assert!(x.contains(r#"--run "D:\Data\a&amp;b.json""#));
        assert!(x.contains("<StartWhenAvailable>true</StartWhenAvailable>"));
        let daily = task_xml(
            &s("06:00", &[]),
            "x",
            Path::new("x"),
            Path::new("y"),
            "2026-10-24",
        );
        assert!(daily.contains("<DaysInterval>1</DaysInterval>"));
    }

    #[test]
    fn run_request_reads_the_path_after_the_flag() {
        assert_eq!(
            run_request(["app.exe", "--run", r"D:\x.json"]),
            Some(r"D:\x.json".to_string())
        );
        assert_eq!(run_request(["app.exe"]), None);
        assert_eq!(run_request(["app.exe", "--run"]), None);
    }

    /// 真的在工作排程器建一個工作、讀回來、再刪掉（手動跑：會動到這台電腦的工作排程器）。
    /// 開始日期設在 2099 年，不會真的觸發。
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn install_and_remove_a_real_task() {
        let profile = std::env::temp_dir().join(format!(
            "nucifraga-schedule-test-{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        let sched = s("03:00", &["sun"]);
        let w = Wanted {
            profile_file: &profile,
            title: "排程測試",
            schedule: &sched,
        };
        install(
            &w,
            Path::new(r"C:\Windows\System32\notepad.exe"),
            "2099-01-01",
        )
        .unwrap();
        let name = task_name(&profile);
        assert!(
            installed().contains(&name),
            "{name} 應該在 {:?}",
            installed()
        );
        remove(&profile).unwrap();
        assert!(!installed().contains(&name));
    }
}
