//! 一次執行：狀態機與編排。狀態與轉移的正本是 `docs/architecture/state-machines.md` §1，
//! [`allowed`] 是那張圖的程式版本，[`Runner::set`] 每次轉移都檢查它。

use std::collections::HashSet;
use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use time::OffsetDateTime;
use time::macros::format_description;

use crate::catalog::{Catalog, Entry};
use crate::engine::{Applied, Engine, JobPaths};
use crate::error::{Code, Level};
use crate::fsops::{self, Marker};
use crate::locks::{self, LockGroup};
use crate::longpath::{self, utf16_len};
use crate::plan::{self, Action, MassVersioning, Plan};
use crate::profile::{self, PathPolicy, Profile};
use crate::report::{Counts, Issue, ManifestWriter, Outcome, Report, format_bytes};
use crate::scan::{self, RootScan};
use crate::sys;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RunState {
    Idle,
    Preflight,
    Scanning,
    Planning,
    AwaitingConfirmation,
    CheckingLocks,
    AwaitingLockDecision,
    Transferring,
    Finalizing,
    Completed,
    CompletedWithWarnings,
    Failed,
    Cancelled,
}

impl RunState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            RunState::Completed
                | RunState::CompletedWithWarnings
                | RunState::Failed
                | RunState::Cancelled
        )
    }
}

/// state-machines.md §1 的每一條箭頭。
pub fn allowed(from: RunState, to: RunState) -> bool {
    use RunState::*;
    matches!(
        (from, to),
        (Idle, Preflight)
            | (Preflight, Scanning)
            | (Preflight, AwaitingConfirmation)
            | (Preflight, Finalizing)
            | (AwaitingConfirmation, Preflight)
            | (AwaitingConfirmation, CheckingLocks)
            | (AwaitingConfirmation, Finalizing)
            | (Scanning, Planning)
            | (Scanning, Finalizing)
            | (Planning, AwaitingConfirmation)
            | (Planning, CheckingLocks)
            | (Planning, Finalizing)
            | (CheckingLocks, Transferring)
            | (CheckingLocks, AwaitingLockDecision)
            | (CheckingLocks, Finalizing)
            | (AwaitingLockDecision, CheckingLocks)
            | (AwaitingLockDecision, Transferring)
            | (AwaitingLockDecision, Finalizing)
            | (Transferring, Finalizing)
            | (Finalizing, Completed)
            | (Finalizing, CompletedWithWarnings)
            | (Finalizing, Failed)
            | (Finalizing, Cancelled)
    )
}

/// 要使用者表態的事。CLI 沒有人可以問，用固定的保守答案（state-machines.md「沒有人可以問的時候」）。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Confirmation {
    /// 數量異常的「移到舊版本」（ENG-004）。
    MassVersioning { items: Vec<MassVersioning> },
    /// 模板還沒記錄備份碟，但這顆碟已經是別的模板的目的地（data-model.md §3）。
    #[serde(rename_all = "camelCase")]
    AdoptDestination {
        destination: String,
        destination_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockDecision {
    /// 使用者關好了，重新檢查。
    Retry,
    /// 略過這些檔案；`remember` 時寫進模板的 `alwaysSkipLocked`。
    Skip {
        remember: bool,
    },
    Cancel,
}

pub trait Decider: Sync {
    fn confirm(&self, c: &Confirmation) -> bool;
    fn locked(&self, groups: &[LockGroup]) -> LockDecision;
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub current: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum Event {
    State(RunState),
    Progress(Progress),
    Issue(Issue),
    Confirmation(Confirmation),
    Locked(Vec<LockGroup>),
}

pub struct RunOptions {
    /// 備份碟找不到時，報告寫到這裡（`<這裡>\<時間>\`）。
    pub fallback_report_dir: Option<PathBuf>,
}

pub struct Runner<'a> {
    engine: &'a dyn Engine,
    decider: &'a dyn Decider,
    sink: &'a (dyn Fn(Event) + Sync),
    cancel: Arc<AtomicBool>,
    options: RunOptions,
    state: RunState,
    issues: Vec<Issue>,
    counts: Counts,
    bytes_copied: u64,
    cancelled: bool,
    last_progress: Instant,
}

/// 預檢拿到的東西：之後每一步都要用。
struct Ready {
    profile: Profile,
    stamp: String,
    destination_id: String,
    catalog: Catalog,
    cluster: u64,
    free_bytes: Option<u64>,
    fat32: bool,
    _lock: File,
}

const PROGRESS_EVERY: Duration = Duration::from_millis(150);

impl<'a> Runner<'a> {
    pub fn new(
        engine: &'a dyn Engine,
        decider: &'a dyn Decider,
        sink: &'a (dyn Fn(Event) + Sync),
        cancel: Arc<AtomicBool>,
        options: RunOptions,
    ) -> Self {
        Runner {
            engine,
            decider,
            sink,
            cancel,
            options,
            state: RunState::Idle,
            issues: Vec::new(),
            counts: Counts::default(),
            bytes_copied: 0,
            cancelled: false,
            last_progress: Instant::now(),
        }
    }

    fn set(&mut self, to: RunState) {
        assert!(
            allowed(self.state, to),
            "不允許的狀態轉移：{:?} → {:?}（見 state-machines.md §1）",
            self.state,
            to
        );
        self.state = to;
        (self.sink)(Event::State(to));
    }

    fn issue(&mut self, i: Issue) {
        (self.sink)(Event::Issue(i.clone()));
        self.issues.push(i);
    }

    fn cancel_requested(&mut self) -> bool {
        if self.cancel.load(Ordering::Relaxed) {
            self.cancelled = true;
        }
        self.cancelled
    }

    fn progress(&mut self, p: Progress, force: bool) {
        if force || self.last_progress.elapsed() >= PROGRESS_EVERY {
            self.last_progress = Instant::now();
            (self.sink)(Event::Progress(p));
        }
    }

    /// 執行一份模板，回傳報告（失敗與取消也有報告）。
    pub fn run(mut self, profile_file: &Path) -> Report {
        let started = now();
        let stamp_base = format_stamp(started);
        self.set(RunState::Preflight);

        let mut title = profile_file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut dest: Option<PathBuf> = None;
        let mut stamp = stamp_base.clone();
        let mut ready: Option<Ready> = None;

        match self.preflight(profile_file, &stamp_base) {
            Ok(r) => {
                title = r.profile.title.clone();
                dest = Some(r.profile.destination.clone());
                stamp = r.stamp.clone();
                ready = Some(r);
            }
            Err(Some(i)) => self.issue(i),
            Err(None) => {} // 使用者拒絕或取消
        }

        let mut manifest: Option<ManifestWriter> = None;
        if let Some(r) = ready.as_mut() {
            self.body(r, &mut manifest);
        }

        // ── Finalizing：任何路徑都會經過這裡 ──
        if self.state != RunState::Finalizing {
            self.set(RunState::Finalizing);
        }
        if let Some(m) = manifest
            && let Err(e) = m.finish()
        {
            self.issue(
                Issue::new(
                    Code::Dst004,
                    &[("path", "manifest.jsonl"), ("detail", &e.to_string())],
                )
                .level(Level::Warning),
            );
        }
        if let Some(r) = ready.as_mut() {
            let _ = r.catalog.set_meta("last_run", &r.stamp);
            if let Err(e) = r.catalog.commit() {
                self.issue(Issue::new(Code::Eng002, &[("detail", &e.to_string())]));
            }
        }
        let outcome = self.outcome();
        let mut report = Report {
            schema_version: 1,
            run_id: stamp.clone(),
            profile: title,
            profile_path: profile_file.display().to_string(),
            destination: dest.as_ref().map(|d| d.display().to_string()),
            outcome,
            started_at: rfc3339(started),
            finished_at: rfc3339(now()),
            counts: std::mem::take(&mut self.counts),
            bytes_copied: self.bytes_copied,
            issues: std::mem::take(&mut self.issues),
            written_to: None,
        };
        // 報告寫在備份碟上；預檢沒過（碟找不到、模板壞掉）時寫到本機的備用位置。
        let target = match (&ready, &dest) {
            (Some(_), Some(d)) => Some(longpath::extended(&d.join("_reports").join(&stamp))),
            _ => self
                .options
                .fallback_report_dir
                .as_ref()
                .map(|d| d.join(&stamp)),
        };
        if let Some(t) = target
            && report.write_to(&t).is_err()
            && let Some(fb) = &self.options.fallback_report_dir
        {
            let _ = report.write_to(&fb.join(&stamp));
        }
        drop(ready); // 放開執行鎖與索引
        self.set(match outcome {
            Outcome::Completed => RunState::Completed,
            Outcome::CompletedWithWarnings => RunState::CompletedWithWarnings,
            Outcome::Failed => RunState::Failed,
            Outcome::Cancelled => RunState::Cancelled,
        });
        report
    }

    fn outcome(&self) -> Outcome {
        if self.cancelled {
            Outcome::Cancelled
        } else if self.issues.iter().any(Issue::is_fatal) {
            Outcome::Failed
        } else if self.counts.skipped > 0
            || self
                .issues
                .iter()
                .any(|i| matches!(i.level, Level::Error | Level::Warning))
        {
            Outcome::CompletedWithWarnings
        } else {
            Outcome::Completed
        }
    }

    /// `Err(Some)`：致命問題；`Err(None)`：使用者拒絕或取消（已經記好了）。
    fn preflight(&mut self, profile_file: &Path, stamp_base: &str) -> Result<Ready, Option<Issue>> {
        let profile = profile::load(profile_file).map_err(Some)?;
        let dest = profile.destination.clone();
        let dest_text = dest.display().to_string();

        if !destination_available(&dest) {
            return Err(Some(Issue::new(Code::Dst001, &[("path", &dest_text)])));
        }
        // 插錯碟要在**寫任何東西之前**擋下：否則別人的碟上會多出一個 .nucifraga 資料夾。
        if let Some(want) = &profile.destination_id
            && fsops::read_marker(&dest).is_none_or(|m| m.destination_id != *want)
        {
            return Err(Some(Issue::new(Code::Dst003, &[])));
        }
        let dest_ext = longpath::extended(&dest);
        fs::create_dir_all(dest_ext.join(fsops::MARKER_DIR))
            .map_err(|e| Some(dest_fatal(&e, &dest_text)))?;

        let lock = File::create(dest_ext.join(fsops::MARKER_DIR).join("run.lock"))
            .map_err(|e| Some(dest_fatal(&e, &dest_text)))?;
        if lock.try_lock().is_err() {
            return Err(Some(Issue::new(Code::Dst007, &[])));
        }

        let destination_id = self.resolve_destination(&profile)?;
        let stamp = fsops::unique_stamp(&dest, stamp_base);
        let opened = Catalog::open(&dest, &stamp).map_err(Some)?;
        if opened.rebuilt {
            self.issue(Issue::new(Code::Eng001, &[]));
        }
        fsops::clean_temp(&dest, &destination_id).map_err(Some)?;
        let vol = sys::volume_info(&dest);
        Ok(Ready {
            stamp,
            destination_id,
            catalog: opened.catalog,
            cluster: vol.as_ref().map_or(4096, |v| v.cluster_size),
            free_bytes: vol.as_ref().map(|v| v.free_bytes),
            fat32: vol
                .as_ref()
                .is_some_and(|v| v.file_system.to_uppercase().starts_with("FAT")),
            profile,
            _lock: lock,
        })
    }

    /// data-model.md §3 的四種情況。
    fn resolve_destination(&mut self, profile: &Profile) -> Result<String, Option<Issue>> {
        let dest = &profile.destination;
        let marker = fsops::read_marker(dest);
        match (&profile.destination_id, marker) {
            (Some(want), Some(m)) if *want == m.destination_id => Ok(m.destination_id),
            (Some(_), _) => Err(Some(Issue::new(Code::Dst003, &[]))),
            (None, None) => {
                let id = uuid::Uuid::new_v4().to_string();
                let m = Marker {
                    schema_version: 1,
                    destination_id: id.clone(),
                    created_at: rfc3339(now()),
                    layout: 1,
                    created_by: format!("nucifraga {}", crate::VERSION),
                };
                fsops::write_marker(dest, &m)
                    .map_err(|e| Some(dest_fatal(&e, &dest.display().to_string())))?;
                if let Err(i) = profile::write_destination_id(&profile.file, &id) {
                    self.issue(i);
                }
                Ok(id)
            }
            (None, Some(m)) => {
                let c = Confirmation::AdoptDestination {
                    destination: dest.display().to_string(),
                    destination_id: m.destination_id.clone(),
                };
                self.set(RunState::AwaitingConfirmation);
                (self.sink)(Event::Confirmation(c.clone()));
                if !self.decider.confirm(&c) {
                    self.cancelled = true;
                    return Err(None);
                }
                self.set(RunState::Preflight);
                if let Err(i) = profile::write_destination_id(&profile.file, &m.destination_id) {
                    self.issue(i);
                }
                Ok(m.destination_id)
            }
        }
    }

    fn body(&mut self, r: &mut Ready, manifest: &mut Option<ManifestWriter>) {
        if self.cancel_requested() {
            return;
        }
        // ── Scanning ──
        self.set(RunState::Scanning);
        let mut scans: Vec<RootScan> = Vec::new();
        let mut scanned = 0u64;
        for root in r.profile.roots.clone() {
            let mut issues = Vec::new();
            let s = scan::scan_root(&r.profile, &root, &self.cancel, &mut issues, &mut || {
                scanned += 1
            });
            for i in issues {
                self.issue(i);
            }
            scans.push(s);
            self.progress(
                Progress {
                    files_done: scanned,
                    current: Some(root.path.display().to_string()),
                    ..Default::default()
                },
                true,
            );
            if self.cancel_requested() {
                return;
            }
        }
        self.counts.scanned = scanned;

        // ── Planning ──
        self.set(RunState::Planning);
        let names: Vec<String> = r.profile.roots.iter().map(|x| x.name.clone()).collect();
        let mut previous = Vec::new();
        for n in &names {
            match r.catalog.load_root(n) {
                Ok(m) => previous.push(m),
                Err(e) => {
                    self.issue(Issue::new(Code::Eng002, &[("detail", &e.to_string())]));
                    return;
                }
            }
        }
        let mut plan = plan::build(&names, &scans, &previous);
        self.counts.unchanged = plan.unchanged;
        if r.fat32 {
            self.drop_fat32_oversize(r, &mut plan);
        }
        if !self.check_paths(r, &plan) || !self.check_space(r, &plan) {
            return;
        }
        if plan.is_empty() {
            return;
        }
        let mass = plan::guard(&plan, &r.profile.delete_guard);
        if !mass.is_empty() {
            let c = Confirmation::MassVersioning {
                items: mass.clone(),
            };
            self.set(RunState::AwaitingConfirmation);
            (self.sink)(Event::Confirmation(c.clone()));
            if !self.decider.confirm(&c) {
                for m in &mass {
                    let count = m.count.to_string();
                    let ratio = format!("{:.1}%", m.ratio * 100.0);
                    self.issue(
                        Issue::new(Code::Eng004, &[("count", &count), ("ratio", &ratio)])
                            .at(&m.root, ""),
                    );
                }
                self.cancelled = true;
                return;
            }
        }

        // ── CheckingLocks ──
        self.set(RunState::CheckingLocks);
        let skip = match self.check_locks(r, &plan) {
            Some(s) => s,
            None => return,
        };
        if self.cancel_requested() {
            return;
        }

        // ── Transferring ──
        self.set(RunState::Transferring);
        let dest = r.profile.destination.clone();
        match ManifestWriter::create(&longpath::extended(
            &dest.join("_reports").join(&r.stamp).join("manifest.jsonl"),
        )) {
            Ok(m) => *manifest = Some(m),
            Err(e) => {
                self.issue(dest_fatal(&e, &dest.display().to_string()));
                return;
            }
        }
        self.transfer(r, &plan, &skip, manifest.as_mut().expect("剛建好"));
    }

    fn drop_fat32_oversize(&mut self, r: &Ready, plan: &mut Plan) {
        const FAT32_MAX: u64 = 0xFFFF_FFFF;
        let names: Vec<String> = r.profile.roots.iter().map(|x| x.name.clone()).collect();
        let mut kept = Vec::with_capacity(plan.jobs.len());
        for j in plan.jobs.drain(..) {
            if j.action != Action::Remove && j.size > FAT32_MAX {
                self.counts.skipped += 1;
                self.issue(
                    Issue::new(Code::Dst006, &[("path", &j.rel)]).at(&names[j.root], &j.rel),
                );
            } else {
                kept.push(j);
            }
        }
        plan.jobs = kept;
    }

    /// 路徑預算（storage-layout.md）。回傳 false ＝ 致命（pathPolicy: block）。
    fn check_paths(&mut self, r: &Ready, plan: &Plan) -> bool {
        let dest = longpath::display(&longpath::extended(&r.profile.destination));
        let lim = r.profile.limits;
        let mut long_dirs: HashSet<String> = HashSet::new();
        let mut long_files: Vec<String> = Vec::new();
        for j in &plan.jobs {
            let name = &r.profile.roots[j.root].name;
            let rel_len = utf16_len(&j.rel);
            let parent_len = j.rel.rfind('/').map_or(0, |p| utf16_len(&j.rel[..p]) + 1);
            let mut prefixes = Vec::new();
            if j.action != Action::Remove {
                prefixes.push(utf16_len(&dest) + utf16_len(&format!("\\current\\{name}")));
            }
            if j.action != Action::Add {
                prefixes.push(
                    utf16_len(&dest) + utf16_len(&format!("\\_versions\\{}\\{name}", r.stamp)),
                );
            }
            for p in prefixes {
                if p + parent_len > lim.dir_path {
                    let dir = &j.rel[..j.rel.rfind('/').unwrap_or(0)];
                    long_dirs.insert(format!("{name}\\{}", dir.replace('/', "\\")));
                }
                // 給人看的樣本：「來源名\相對路徑」，用 Windows 的分隔符號。
                let shown = format!("{name}\\{}", j.rel.replace('/', "\\"));
                if p + 1 + rel_len > lim.file_path && long_files.last() != Some(&shown) {
                    long_files.push(shown);
                }
            }
        }
        let block = r.profile.path_policy == PathPolicy::Block;
        if !long_dirs.is_empty() {
            let count = long_dirs.len().to_string();
            let limit = lim.dir_path.to_string();
            let mut sample: Vec<_> = long_dirs.into_iter().collect();
            sample.sort();
            sample.truncate(20);
            let mut i = Issue::new(Code::Path001, &[("count", &count), ("limit", &limit)])
                .detail(sample.join("\n"));
            if block {
                i = i.level(Level::Fatal);
            }
            self.issue(i);
            if block {
                return false;
            }
        }
        if !long_files.is_empty() {
            let count = long_files.len().to_string();
            let limit = lim.file_path.to_string();
            long_files.truncate(20);
            self.issue(
                Issue::new(Code::Path002, &[("count", &count), ("limit", &limit)])
                    .detail(long_files.join("\n")),
            );
        }
        true
    }

    fn check_space(&mut self, r: &Ready, plan: &Plan) -> bool {
        let need = plan.bytes_needed(r.cluster);
        match r.free_bytes {
            Some(free) if need > free => {
                let (n, f) = (format_bytes(need), format_bytes(free));
                self.issue(Issue::new(Code::Dst002, &[("need", &n), ("free", &f)]));
                false
            }
            _ => true,
        }
    }

    /// 回傳要略過的計畫索引；`None` ＝ 取消。
    fn check_locks(&mut self, r: &Ready, plan: &Plan) -> Option<HashSet<usize>> {
        let mut skip = HashSet::new();
        let mut candidates: Vec<locks::Candidate> = plan
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, j)| j.action != Action::Remove)
            .map(|(i, j)| {
                let root = &r.profile.roots[j.root];
                (
                    i,
                    root.name.clone(),
                    j.rel.clone(),
                    scan::source_path(root, &j.rel),
                )
            })
            .collect();
        loop {
            let cancel = self.cancel.clone();
            let probed = locks::probe(&candidates, &cancel, &mut |done, total| {
                let p = Progress {
                    files_done: done as u64,
                    files_total: total as u64,
                    bytes_done: 0,
                    bytes_total: 0,
                    current: None,
                };
                self.progress(p, done == total);
            });
            let Some(groups) = probed else {
                self.cancelled = true;
                return None;
            };
            if groups.is_empty() {
                break;
            }
            // 模板裡「永遠略過」的不問。
            let (auto, ask): (Vec<_>, Vec<_>) = groups
                .into_iter()
                .flat_map(|g| {
                    let app = g.app;
                    g.files.into_iter().map(move |f| (app.clone(), f))
                })
                .partition(|(_, f)| r.profile.always_skip_locked.is_match(&f.rel));
            let mut apps: Vec<String> = Vec::new();
            for (app, f) in &auto {
                skip.insert(f.job);
                if !apps.contains(app) {
                    apps.push(app.clone());
                }
            }
            if ask.is_empty() {
                self.record_locked_skips(auto.len(), &apps);
                break;
            }
            let mut regrouped: Vec<LockGroup> = Vec::new();
            for (app, f) in ask {
                match regrouped.iter_mut().find(|g| g.app == app) {
                    Some(g) => g.files.push(f),
                    None => regrouped.push(LockGroup {
                        app,
                        files: vec![f],
                    }),
                }
            }
            self.set(RunState::AwaitingLockDecision);
            (self.sink)(Event::Locked(regrouped.clone()));
            match self.decider.locked(&regrouped) {
                LockDecision::Retry => {
                    // 只重試這次打不開的。其他檔案之後才被鎖住的，傳輸時報 SRC-LOCKED（ADR-0009）。
                    let again: HashSet<usize> = regrouped
                        .iter()
                        .flat_map(|g| g.files.iter().map(|f| f.job))
                        .chain(auto.iter().map(|(_, f)| f.job))
                        .collect();
                    candidates.retain(|c| again.contains(&c.0));
                    self.set(RunState::CheckingLocks);
                    skip.clear();
                    continue;
                }
                LockDecision::Cancel => {
                    self.cancelled = true;
                    return None;
                }
                LockDecision::Skip { remember } => {
                    let mut n = auto.len();
                    for g in &regrouped {
                        if !apps.contains(&g.app) {
                            apps.push(g.app.clone());
                        }
                        for f in &g.files {
                            skip.insert(f.job);
                            n += 1;
                        }
                    }
                    if remember {
                        let patterns: Vec<String> = regrouped
                            .iter()
                            .flat_map(|g| g.files.iter().map(|f| globset::escape(&f.rel)))
                            .collect();
                        if let Err(i) =
                            profile::append_always_skip_locked(&r.profile.file, &patterns)
                        {
                            self.issue(i);
                        }
                    }
                    self.record_locked_skips(n, &apps);
                    break;
                }
            }
        }
        Some(skip)
    }

    fn record_locked_skips(&mut self, n: usize, apps: &[String]) {
        if n == 0 {
            return;
        }
        self.counts.skipped += n as u64;
        let count = n.to_string();
        let apps = apps.join("、");
        self.issue(Issue::new(
            Code::Src003,
            &[("count", &count), ("apps", &apps)],
        ));
    }

    fn transfer(
        &mut self,
        r: &mut Ready,
        plan: &Plan,
        skip: &HashSet<usize>,
        manifest: &mut ManifestWriter,
    ) {
        let dest = r.profile.destination.clone();
        let current_base = longpath::extended(&dest.join("current"));
        let version_base = longpath::extended(&dest.join("_versions").join(&r.stamp));
        let temp_base = fsops::temp_dir(&dest);
        if let Err(e) = fs::create_dir_all(&temp_base) {
            self.issue(dest_fatal(&e, &dest.display().to_string()));
            return;
        }
        let mut p = Progress {
            files_total: (plan.jobs.len() - skip.len()) as u64,
            bytes_total: plan
                .jobs
                .iter()
                .enumerate()
                .filter(|(i, j)| !skip.contains(i) && j.action != Action::Remove)
                .map(|(_, j)| j.size)
                .sum(),
            ..Default::default()
        };
        for (n, j) in plan.jobs.iter().enumerate() {
            if skip.contains(&n) {
                continue;
            }
            if self.cancel_requested() {
                break;
            }
            let root = &r.profile.roots[j.root];
            let paths = JobPaths {
                source: scan::source_path(root, &j.rel),
                current: longpath::join_rel(&current_base.join(&root.name), &j.rel),
                version: longpath::join_rel(&version_base.join(&root.name), &j.rel),
                temp: temp_base.join(format!("{n}{}", fsops::TEMP_SUFFIX)),
            };
            p.current = Some(format!("{}\\{}", root.name, j.rel.replace('/', "\\")));
            self.progress(p.clone(), false);

            let result = self.engine.apply(j, &paths, r.profile.verify_after_copy);
            // 成功時暫存檔已經改名進 current\；只有失敗才可能留下（讀回不符、寫到一半）。
            if result.is_err() {
                let _ = fsops::delete_temp(&dest, &r.destination_id, &paths.temp);
            }
            let name = root.name.clone();
            match result {
                Ok(Applied::Copied { sha256, superseded }) => {
                    self.record(r, &name, j, &sha256, "copied", manifest);
                    if j.action == Action::Add {
                        self.counts.copied += 1
                    } else {
                        self.counts.updated += 1
                    }
                    if superseded {
                        self.counts.versioned += 1;
                    }
                    self.bytes_copied += j.size;
                }
                Ok(Applied::Unchanged { sha256 }) => {
                    self.record(r, &name, j, &sha256, "verified", manifest);
                    self.counts.verified += 1;
                }
                Ok(Applied::Versioned) => {
                    if let Err(e) = r.catalog.remove(&name, &j.key) {
                        self.issue(Issue::new(Code::Eng002, &[("detail", &e.to_string())]));
                    }
                    self.counts.versioned += 1;
                }
                Err(f) => {
                    let i = Issue::new(f.code, &[("path", &j.rel), ("detail", &f.detail)])
                        .at(&name, &j.rel)
                        .detail(f.detail.clone());
                    let fatal = i.is_fatal();
                    if !fatal {
                        self.counts.skipped += 1;
                        if i.level == Level::Error {
                            self.counts.errors += 1;
                        }
                    }
                    self.issue(i);
                    if fatal {
                        break;
                    }
                }
            }
            p.files_done += 1;
            if j.action != Action::Remove {
                p.bytes_done += j.size;
            }
        }
        p.current = None;
        self.progress(p, true);
    }

    fn record(
        &mut self,
        r: &mut Ready,
        root: &str,
        j: &plan::Job,
        sha: &str,
        action: &str,
        manifest: &mut ManifestWriter,
    ) {
        let e = Entry {
            rel: j.rel.clone(),
            size: j.size,
            mtime_ns: j.mtime_ns,
            sha256: Some(sha.to_string()),
        };
        if let Err(err) = r.catalog.upsert(root, &e, &r.stamp) {
            self.issue(Issue::new(Code::Eng002, &[("detail", &err.to_string())]));
        }
        if let Err(err) = manifest.record(root, &j.rel, j.size, sha, action) {
            self.issue(
                Issue::new(
                    Code::Dst004,
                    &[("path", "manifest.jsonl"), ("detail", &err.to_string())],
                )
                .level(Level::Warning),
            );
        }
    }
}

/// 寫備份碟時的錯誤：一律當成致命（預檢與建資料夾階段）。
fn dest_fatal(e: &std::io::Error, dest: &str) -> Issue {
    let code = crate::engine::classify_destination(e);
    let code = if matches!(code, Code::Dst001 | Code::Dst002) {
        code
    } else {
        Code::Dst004
    };
    Issue::new(code, &[("path", dest), ("detail", &e.to_string())])
        .detail(e.to_string())
        .level(Level::Fatal)
}

/// 備份碟有沒有接上：看**碟**在不在，不看目的地資料夾在不在 —— 第一次備份前那個資料夾本來就還沒建。
/// 首頁的「備份碟未接上」與預檢的 `DST-001` 用同一個判斷。
pub fn destination_available(dest: &Path) -> bool {
    volume_root(dest).is_some_and(|r| r.exists())
}

/// `E:\Backup\x` → `E:\`。網路路徑 → `\\server\share\`。
fn volume_root(p: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => out.push(c.as_os_str()),
            _ => break,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(longpath::extended(&out))
    }
}

fn now() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

fn format_stamp(t: OffsetDateTime) -> String {
    t.format(format_description!("[year][month][day]-[hour][minute]"))
        .expect("固定格式一定格式化得了")
}

fn rfc3339(t: OffsetDateTime) -> String {
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use RunState::*;

    #[test]
    fn every_terminal_state_is_reached_only_from_finalizing() {
        let all = [
            Idle,
            Preflight,
            Scanning,
            Planning,
            AwaitingConfirmation,
            CheckingLocks,
            AwaitingLockDecision,
            Transferring,
            Finalizing,
            Completed,
            CompletedWithWarnings,
            Failed,
            Cancelled,
        ];
        for from in all {
            for to in all.iter().copied().filter(|s| s.is_terminal()) {
                assert_eq!(allowed(from, to), from == Finalizing, "{from:?} → {to:?}");
            }
        }
    }

    #[test]
    fn every_non_terminal_state_can_reach_finalizing() {
        for s in [
            Preflight,
            Scanning,
            Planning,
            AwaitingConfirmation,
            CheckingLocks,
            AwaitingLockDecision,
            Transferring,
        ] {
            assert!(
                allowed(s, Finalizing),
                "{s:?} 必須能直接去收尾（取消、致命錯誤）"
            );
        }
    }

    #[test]
    fn terminal_states_are_dead_ends() {
        for from in [Completed, CompletedWithWarnings, Failed, Cancelled] {
            for to in [Idle, Preflight, Finalizing, Transferring] {
                assert!(!allowed(from, to));
            }
        }
    }

    #[test]
    fn stamp_is_minute_precision() {
        let t = time::macros::datetime!(2026-10-23 09:05:59 +08:00);
        assert_eq!(format_stamp(t), "20261023-0905");
    }

    #[test]
    fn volume_root_of_a_drive_path() {
        assert_eq!(
            volume_root(Path::new(r"E:\Backup\x")),
            Some(longpath::extended(Path::new(r"E:\")))
        );
        assert_eq!(volume_root(Path::new("relative")), None);
    }
}
