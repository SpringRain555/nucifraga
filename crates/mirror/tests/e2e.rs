//! 端到端：真的 Runner＋真的 Mirror，在 %TEMP% 底下的亂數資料夾裡跑（tempfile 會在結束時清掉自己建的資料夾）。
//! 每一個測試對應 REQ 裡的一條驗收條件。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use nucifraga_core::engine::{Applied, Engine, Failure, JobPaths, sha256_file};
use nucifraga_core::error::Code;
use nucifraga_core::locks::LockGroup;
use nucifraga_core::plan::Job;
use nucifraga_core::report::{Outcome, Report};
use nucifraga_core::run::{Confirmation, Decider, LockDecision, RunOptions, Runner};
use nucifraga_mirror::Mirror;

struct Answers {
    confirm: bool,
    lock: LockDecision,
}

impl Decider for Answers {
    fn confirm(&self, _: &Confirmation) -> bool {
        self.confirm
    }
    fn locked(&self, _: &[LockGroup]) -> LockDecision {
        self.lock.clone()
    }
}

const YES: Answers = Answers {
    confirm: true,
    lock: LockDecision::Skip { remember: false },
};
const NO: Answers = Answers {
    confirm: false,
    lock: LockDecision::Cancel,
};

struct Fx {
    _dir: tempfile::TempDir,
    src: PathBuf,
    dest: PathBuf,
    profile: PathBuf,
    fallback: PathBuf,
}

fn json_str(p: &Path) -> String {
    serde_json::to_string(&p.display().to_string()).unwrap()
}

impl Fx {
    fn new() -> Fx {
        Self::with_options("")
    }

    fn with_options(extra: &str) -> Fx {
        let dir = tempfile::Builder::new()
            .prefix("nucifraga-e2e-")
            .tempdir()
            .unwrap();
        let src = dir.path().join("src");
        let dest = dir.path().join("dest");
        fs::create_dir_all(&src).unwrap();
        let profile = dir.path().join("weekly.json");
        let fallback = dir.path().join("fallback");
        fs::write(
            &profile,
            format!(
                "{{\n  \"schemaVersion\": 1,\n  \"destination\": {{ \"root\": {} }},\n  \"roots\": [ {{ \"path\": {}, \"name\": \"Docs\" }} ]{extra}\n}}\n",
                json_str(&dest),
                json_str(&src)
            ),
        )
        .unwrap();
        Fx {
            _dir: dir,
            src,
            dest,
            profile,
            fallback,
        }
    }

    fn write(&self, rel: &str, content: &str) {
        let p = ext(&self.src.join(rel));
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    fn run_with(&self, engine: &dyn Engine, answers: &Answers, cancel: Arc<AtomicBool>) -> Report {
        let sink = |_e: nucifraga_core::run::Event| {};
        Runner::new(
            engine,
            answers,
            &sink,
            cancel,
            RunOptions {
                fallback_report_dir: Some(self.fallback.clone()),
            },
        )
        .run(&self.profile)
    }

    fn run(&self, answers: &Answers) -> Report {
        self.run_with(&Mirror, answers, Arc::new(AtomicBool::new(false)))
    }

    fn current(&self, rel: &str) -> PathBuf {
        ext(&self.dest.join("current").join("Docs").join(rel))
    }

    fn versions_dir(&self) -> PathBuf {
        self.dest.join("_versions")
    }
}

fn ext(p: &Path) -> PathBuf {
    nucifraga_core::longpath::extended(p)
}

fn codes(r: &Report) -> Vec<Code> {
    r.issues.iter().map(|i| i.code).collect()
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap()
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![ext(dir)];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.file_type().unwrap().is_dir() {
                stack.push(e.path())
            } else {
                out.push(e.path())
            }
        }
    }
    out
}

#[test]
fn first_run_copies_everything_and_the_manifest_verifies() {
    let fx = Fx::new();
    fx.write("a.txt", "alpha");
    fx.write("sub/deeper/b.txt", "beta");
    let r = fx.run(&YES);
    assert_eq!(r.outcome, Outcome::Completed, "{:?}", r.issues);
    assert_eq!(r.counts.copied, 2);
    assert_eq!(read(&fx.current("sub/deeper/b.txt")), "beta");

    // 清單裡的雜湊與備份碟上的檔案相符（REQ-0005：不靠 nucifraga 也能驗）
    let report_dir = r.written_to.clone().unwrap();
    let manifest = read(&report_dir.join("manifest.jsonl"));
    assert_eq!(manifest.lines().count(), 2);
    for line in manifest.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let file = fx.current(v["path"].as_str().unwrap());
        assert_eq!(sha256_file(&file).unwrap(), v["sha256"].as_str().unwrap());
    }
    assert!(report_dir.join("report.json").exists() && report_dir.join("summary.txt").exists());

    // 第一次執行：建立標記，並把 id 寫回模板（只動那一個值）
    let text = read(&fx.profile);
    assert!(text.contains("\"destinationId\""));
    assert!(fx.dest.join(".nucifraga/destination.json").exists());
    // 修改時間跟來源一樣
    let src_m = fs::metadata(fx.src.join("a.txt"))
        .unwrap()
        .modified()
        .unwrap();
    let dst_m = fs::metadata(fx.current("a.txt"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(src_m, dst_m);
}

#[test]
fn second_run_only_transfers_changes_and_keeps_old_versions() {
    let fx = Fx::new();
    fx.write("keep.txt", "same");
    fx.write("edit.txt", "v1");
    fx.write("gone.txt", "bye");
    assert_eq!(fx.run(&YES).outcome, Outcome::Completed);

    std::thread::sleep(Duration::from_millis(20));
    fx.write("edit.txt", "v2-longer");
    fx.write("new.txt", "hi");
    fs::remove_file(fx.src.join("gone.txt")).unwrap();
    let r = fx.run(&YES);
    assert_eq!(r.outcome, Outcome::Completed, "{:?}", r.issues);
    assert_eq!(
        (r.counts.copied, r.counts.updated, r.counts.unchanged),
        (1, 1, 1)
    );
    assert_eq!(r.counts.versioned, 2, "edit 的舊版＋gone");
    assert_eq!(read(&fx.current("edit.txt")), "v2-longer");
    assert!(!fx.current("gone.txt").exists());

    let old: Vec<String> = files_under(&fx.versions_dir())
        .iter()
        .map(|p| read(p))
        .collect();
    assert!(
        old.contains(&"v1".to_string()) && old.contains(&"bye".to_string()),
        "{old:?}"
    );
}

#[test]
fn touched_but_identical_file_does_not_create_an_old_version() {
    let fx = Fx::new();
    fx.write("a.txt", "same content");
    fx.run(&YES);
    let f = fs::File::options()
        .write(true)
        .open(fx.src.join("a.txt"))
        .unwrap();
    f.set_modified(SystemTime::now() + Duration::from_secs(60))
        .unwrap();
    drop(f);
    let r = fx.run(&YES);
    assert_eq!(r.counts.verified, 1);
    assert_eq!(r.counts.versioned, 0);
    assert!(!fx.versions_dir().exists());
}

#[test]
fn long_paths_are_backed_up_and_reported() {
    let fx = Fx::new();
    let seg = "x".repeat(60);
    let rel = format!("{seg}1/{seg}2/{seg}3/{seg}4/long-file-name-that-pushes-past-the-limit.txt");
    fx.write(&rel, "deep");
    let r = fx.run(&YES);
    assert_eq!(r.outcome, Outcome::CompletedWithWarnings);
    assert!(codes(&r).contains(&Code::Path001), "{:?}", codes(&r));
    let dest_file = fx.current(&rel);
    assert!(nucifraga_core::longpath::display(&dest_file).len() > 260);
    assert_eq!(
        read(&dest_file),
        "deep",
        "超過 260 字元的檔案照樣要備份成功（REQ-0005）"
    );
}

#[test]
fn path_policy_block_stops_before_writing_anything() {
    let fx = Fx::with_options(",\n  \"nucifraga\": { \"pathPolicy\": \"block\" }");
    let seg = "y".repeat(80);
    fx.write(&format!("{seg}1/{seg}2/{seg}3/f.txt"), "deep");
    let r = fx.run(&YES);
    assert_eq!(r.outcome, Outcome::Failed);
    assert!(!fx.dest.join("current").exists());
}

/// 傳到第 N 個檔案就要求取消的 engine。
struct CancelAfter {
    n: usize,
    seen: AtomicUsize,
    cancel: Arc<AtomicBool>,
}

impl Engine for CancelAfter {
    fn format(&self) -> &'static str {
        "mirror"
    }
    fn apply(&self, job: &Job, paths: &JobPaths, verify: bool) -> Result<Applied, Failure> {
        if self.seen.fetch_add(1, Ordering::SeqCst) + 1 >= self.n {
            self.cancel.store(true, Ordering::SeqCst);
        }
        Mirror.apply(job, paths, verify)
    }
}

#[test]
fn interrupted_run_resumes_by_running_again() {
    let fx = Fx::new();
    for i in 0..30 {
        fx.write(&format!("f{i:02}.txt"), &format!("content {i}"));
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let engine = CancelAfter {
        n: 10,
        seen: AtomicUsize::new(0),
        cancel: cancel.clone(),
    };
    let r1 = fx.run_with(&engine, &YES, cancel);
    assert_eq!(r1.outcome, Outcome::Cancelled);
    assert_eq!(r1.counts.copied, 10);

    let r2 = fx.run(&YES);
    assert_eq!(r2.outcome, Outcome::Completed, "{:?}", r2.issues);
    assert_eq!((r2.counts.copied, r2.counts.unchanged), (20, 10));
    assert_eq!(files_under(&fx.dest.join("current")).len(), 30);
    assert!(!fx.versions_dir().exists(), "續傳不可以產生任何舊版本");
    assert!(
        files_under(&fx.dest.join(".nucifraga/tmp")).is_empty(),
        "不可以留下暫存檔"
    );
}

#[test]
fn deleted_catalog_is_rebuilt_without_spurious_versions() {
    let fx = Fx::new();
    fx.write("a.txt", "a");
    fx.write("b/c.txt", "c");
    fx.run(&YES);
    fs::remove_file(fx.dest.join(".nucifraga/catalog.sqlite")).unwrap();
    let r = fx.run(&YES);
    assert!(codes(&r).contains(&Code::Eng001));
    assert_eq!(r.counts.verified, 2);
    assert_eq!(r.counts.versioned, 0);
    assert!(!fx.versions_dir().exists());
    // 下一次就回到正常的「沒有變更」
    let r3 = fx.run(&YES);
    assert_eq!((r3.outcome, r3.counts.unchanged), (Outcome::Completed, 2));
}

#[test]
fn wrong_disk_is_refused_before_writing_anything() {
    let fx = Fx::new();
    fx.write("a.txt", "a");
    fx.run(&YES);
    let other = fx.dest.with_file_name("other-disk");
    fs::create_dir_all(&other).unwrap();
    let text = read(&fx.profile).replace(
        &json_str(&fx.dest)[1..json_str(&fx.dest).len() - 1],
        &json_str(&other)[1..json_str(&other).len() - 1],
    );
    fs::write(&fx.profile, text).unwrap();
    let r = fx.run(&YES);
    assert_eq!(r.outcome, Outcome::Failed);
    assert_eq!(codes(&r), vec![Code::Dst003]);
    assert_eq!(
        fs::read_dir(&other).unwrap().count(),
        0,
        "別人的碟上不可以留下任何東西"
    );
    assert!(
        r.written_to.unwrap().starts_with(&fx.fallback),
        "備份碟不對時報告寫到本機的備用位置"
    );
}

#[test]
fn missing_source_root_does_not_move_anything_to_old_versions() {
    let fx = Fx::new();
    fx.write("a.txt", "a");
    fx.run(&YES);
    fs::rename(&fx.src, fx.src.with_file_name("src-moved")).unwrap();
    let r = fx.run(&YES);
    assert!(codes(&r).contains(&Code::Src001));
    assert_eq!(r.counts.versioned, 0);
    assert!(fx.current("a.txt").exists());
}

#[test]
fn mass_versioning_stops_and_asks() {
    let fx = Fx::new();
    for i in 0..100 {
        fx.write(&format!("f{i:03}.txt"), "x");
    }
    fx.run(&YES);
    for i in 0..60 {
        fs::remove_file(fx.src.join(format!("f{i:03}.txt"))).unwrap();
    }
    let declined = fx.run(&NO);
    assert_eq!(declined.outcome, Outcome::Cancelled);
    assert!(codes(&declined).contains(&Code::Eng004));
    assert_eq!(
        files_under(&fx.dest.join("current")).len(),
        100,
        "拒絕之後什麼都不動"
    );
    let accepted = fx.run(&YES);
    assert_eq!(accepted.counts.versioned, 60);
}

#[cfg(windows)]
#[test]
fn locked_files_are_skipped_and_can_be_remembered() {
    use std::os::windows::fs::OpenOptionsExt;
    let fx = Fx::new();
    fx.write("free.txt", "ok");
    fx.write("mail.pst", "busy");
    let held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(fx.src.join("mail.pst"))
        .unwrap();
    let r = fx.run(&Answers {
        confirm: true,
        lock: LockDecision::Skip { remember: true },
    });
    assert_eq!(r.outcome, Outcome::CompletedWithWarnings);
    assert!(codes(&r).contains(&Code::Src003));
    assert!(fx.current("free.txt").exists());
    assert!(!fx.current("mail.pst").exists());
    assert!(
        read(&fx.profile).contains("\"alwaysSkipLocked\": [\"mail.pst\"]"),
        "{}",
        read(&fx.profile)
    );
    // 記住之後就不再問：給一個「取消」的答案也照樣跑完
    let r2 = fx.run(&NO);
    assert_eq!(
        r2.outcome,
        Outcome::CompletedWithWarnings,
        "{:?}",
        r2.issues
    );
    drop(held);
}

#[test]
fn a_second_profile_must_be_allowed_to_share_a_disk() {
    let fx = Fx::new();
    fx.write("a.txt", "a");
    fx.run(&YES);
    let second = fx.profile.with_file_name("second.json");
    let other_src = fx.src.with_file_name("src2");
    fs::create_dir_all(&other_src).unwrap();
    fs::write(
        &second,
        format!(r#"{{"schemaVersion":1,"destination":{{"root":{}}},"roots":[{{"path":{},"name":"Other"}}]}}"#, json_str(&fx.dest), json_str(&other_src)),
    )
    .unwrap();
    let sink = |_e: nucifraga_core::run::Event| {};
    let run = |a: &Answers| {
        Runner::new(
            &Mirror,
            a,
            &sink,
            Arc::new(AtomicBool::new(false)),
            RunOptions {
                fallback_report_dir: None,
            },
        )
        .run(&second)
    };
    assert_eq!(run(&NO).outcome, Outcome::Cancelled);
    assert!(!read(&second).contains("destinationId"));
    assert_eq!(run(&YES).outcome, Outcome::Completed);
    assert!(read(&second).contains("destinationId"));
}
