//! 前端能呼叫的全部命令（ADR-0007）。每一個都只收必要的參數，路徑一律在這裡驗證：
//! 能碰的只有「模板資料夾裡的或已經記住的外部模板」與「那份模板的備份碟底下」。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use nucifraga_core::error::Code;
use nucifraga_core::fsops::{self, VersionStamp};
use nucifraga_core::jsonedit::quote;
use nucifraga_core::library;
use nucifraga_core::profile::{self, PathPolicy, Profile, RootSpec};
use nucifraga_core::report::{self, Issue};
use nucifraga_core::run::LockDecision;
use nucifraga_core::schedule::{self, Schedule};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::runctl::{self, Active, Answer};
use crate::settings::{self, ProfileRef, Settings};

pub struct AppState {
    pub settings_file: PathBuf,
    pub settings: Mutex<Settings>,
    pub settings_was_broken: bool,
    /// 第一次使用時建議的模板資料夾（`文件\Nucifraga`）。
    pub suggested_folder: PathBuf,
    pub fallback_reports: PathBuf,
    pub run: Arc<Mutex<Option<Active>>>,
    /// 模板編輯畫面有沒有未儲存的修改：有的話關視窗前先問（ui-workflows「模板編輯」）。
    pub unsaved: AtomicBool,
    /// 這次啟動帶的 `--run <模板>`（排程開的）；前端畫好第一個畫面後拿走。
    pub launch_request: Mutex<Option<String>>,
    /// 啟動時同步工作排程器遇到的問題（`APP-007`），顯示在首頁。
    pub schedule_issues: Mutex<Vec<Issue>>,
}

type R<T> = Result<T, Issue>;

fn internal(e: impl ToString) -> Issue {
    Issue::new(Code::Eng002, &[("detail", &e.to_string())])
}

fn profiles_folder(state: &AppState) -> R<Option<PathBuf>> {
    Ok(state
        .settings
        .lock()
        .map_err(internal)?
        .profiles_folder
        .as_ref()
        .map(PathBuf::from))
}

/// 只接受模板資料夾裡的、或已經記住的外部模板 —— 前端不能拿任意路徑來讀寫。
fn known_profile(state: &AppState, path: &str) -> R<Profile> {
    let in_folder = profiles_folder(state)?.is_some_and(|f| library::contains(&f, Path::new(path)));
    let external = state
        .settings
        .lock()
        .map_err(internal)?
        .profiles
        .iter()
        .any(|p| p.path == path);
    if !in_folder && !external {
        return Err(internal("這份模板不在清單裡"));
    }
    profile::load(Path::new(path))
}

fn save_settings(state: &AppState, s: &Settings) -> R<()> {
    settings::save(&state.settings_file, s)
        .map_err(|e| Issue::new(Code::Cfg008, &[("detail", &e.to_string())]))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileCard {
    path: String,
    title: String,
    destination: Option<String>,
    roots: Vec<String>,
    destination_connected: bool,
    /// 這顆備份碟上現在有一次備份在跑（這個 app 或別的程式，例如排程的 CLI）。
    busy: bool,
    /// 上一次沒有正常結束（強制關閉、當機、拔碟），而且還沒有被下一次備份收尾。
    interrupted: bool,
    /// 資料夾外的模板：可以「從清單移除」；資料夾裡的要移除就是把檔案搬走。
    external: bool,
    schedule: Option<Schedule>,
    error: Option<Issue>,
    last: Option<serde_json::Value>,
}

fn card(path: &str, external: bool) -> ProfileCard {
    match profile::load(Path::new(path)) {
        Ok(p) => {
            let connected = nucifraga_core::run::destination_available(&p.destination);
            let busy = connected && fsops::run_in_progress(&p.destination);
            let interrupted =
                connected && !busy && !report::unfinished_runs(&p.destination).is_empty();
            ProfileCard {
                path: path.into(),
                title: p.title.clone(),
                destination: Some(p.destination.display().to_string()),
                roots: p
                    .roots
                    .iter()
                    .map(|r| r.path.display().to_string())
                    .collect(),
                destination_connected: connected,
                busy,
                interrupted,
                external,
                schedule: p.raw.nucifraga.schedule.clone(),
                error: None,
                last: if connected {
                    report::list_reports(&p.destination, 1).into_iter().next()
                } else {
                    None
                },
            }
        }
        Err(i) => ProfileCard {
            path: path.into(),
            title: Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            destination: None,
            roots: Vec::new(),
            destination_connected: false,
            busy: false,
            interrupted: false,
            external,
            schedule: None,
            error: Some(i),
            last: None,
        },
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Home {
    /// `None` ＝ 第一次使用，還沒選模板資料夾。
    profiles_folder: Option<String>,
    suggested_folder: String,
    /// 模板資料夾找不到（`APP-004`）；外部模板照樣列出。
    folder_issue: Option<Issue>,
    schedule_issues: Vec<Issue>,
    profiles: Vec<ProfileCard>,
    settings_was_broken: bool,
    running: bool,
}

#[tauri::command]
pub fn home(state: State<'_, AppState>) -> R<Home> {
    let s = state.settings.lock().map_err(internal)?.clone();
    let folder = s.profiles_folder.as_ref().map(PathBuf::from);
    let mut profiles = Vec::new();
    let mut folder_issue = None;
    if let Some(f) = &folder {
        match library::list(f) {
            Ok(files) => {
                profiles.extend(files.iter().map(|p| card(&p.display().to_string(), false)))
            }
            Err(i) => folder_issue = Some(i),
        }
    }
    for p in &s.profiles {
        // 外部模板後來被放進了模板資料夾：上面已經列過。
        if folder
            .as_ref()
            .is_some_and(|f| library::contains(f, Path::new(&p.path)))
        {
            continue;
        }
        profiles.push(card(&p.path, true));
    }
    Ok(Home {
        profiles_folder: s.profiles_folder.clone(),
        suggested_folder: state.suggested_folder.display().to_string(),
        folder_issue,
        schedule_issues: state.schedule_issues.lock().map_err(internal)?.clone(),
        profiles,
        settings_was_broken: state.settings_was_broken,
        running: state.run.lock().map_err(internal)?.is_some(),
    })
}

fn now_text() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

fn remember(state: &AppState, path: &str) -> R<()> {
    let mut s = state.settings.lock().map_err(internal)?;
    if !s.profiles.iter().any(|p| p.path == path) {
        s.profiles.push(ProfileRef {
            path: path.into(),
            added_at: now_text(),
        });
        let snapshot = s.clone();
        drop(s);
        save_settings(state, &snapshot)?;
    }
    Ok(())
}

/// 「加入現有的模板檔」：讀不懂就不加入，直接回那個 CFG-*。
/// 本來就在模板資料夾裡的不另記 —— 首頁已經會列出它。
#[tauri::command]
pub fn add_profile(state: State<'_, AppState>, path: String) -> R<()> {
    profile::load(Path::new(&path))?;
    if profiles_folder(&state)?.is_some_and(|f| library::contains(&f, Path::new(&path))) {
        return Ok(());
    }
    remember(&state, &path)
}

/// 「建立新模板」：存進模板資料夾，檔名由名稱產生。回傳新模板的路徑。
#[tauri::command]
pub fn create_profile(
    state: State<'_, AppState>,
    title: String,
    destination: String,
    roots: Vec<String>,
) -> R<String> {
    let folder = profiles_folder(&state)?.ok_or_else(|| internal("還沒選模板資料夾"))?;
    library::ensure(&folder)?;
    let path = library::new_path(&folder, &title);
    profile::create(&path, &title, &destination, &roots)?;
    Ok(path.display().to_string())
}

/// 選定（或更換）模板資料夾。`move_existing`：把舊資料夾第一層的模板一起搬過去，
/// 同名的不覆蓋、留在原處（`APP-005`）。回傳沒搬成的那幾則。
#[tauri::command]
pub fn set_profiles_folder(
    state: State<'_, AppState>,
    path: String,
    move_existing: bool,
) -> R<Vec<Issue>> {
    if state.run.lock().map_err(internal)?.is_some() {
        return Err(internal("備份進行中，不能更換模板資料夾"));
    }
    let new = PathBuf::from(&path);
    library::ensure(&new)?;
    let mut issues = Vec::new();
    if move_existing
        && let Some(old) = profiles_folder(&state)?
        && !library::same_folder(&old, &new)
    {
        match library::move_all(&old, &new) {
            Ok(m) => issues.extend(m.issues),
            Err(i) => issues.push(i),
        }
    }
    let mut s = state.settings.lock().map_err(internal)?;
    s.profiles_folder = Some(path);
    let snapshot = s.clone();
    drop(s);
    save_settings(&state, &snapshot)?;
    // 模板的路徑變了，排程的工作也要跟著換（名稱由路徑而來）。
    reconcile_schedules(&state);
    Ok(issues)
}

#[tauri::command]
pub fn open_profiles_folder(state: State<'_, AppState>) -> R<()> {
    let folder = profiles_folder(&state)?.ok_or_else(|| internal("還沒選模板資料夾"))?;
    if !folder.is_dir() {
        return Err(Issue::new(
            Code::App004,
            &[("path", &folder.display().to_string())],
        ));
    }
    explorer(&folder)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    version: &'static str,
    identifier: String,
    settings_file: String,
    fallback_reports: String,
    background_mode: bool,
}

/// 設定頁的「關於與診斷」。
#[tauri::command]
pub fn diagnostics(app: AppHandle, state: State<'_, AppState>) -> Diagnostics {
    Diagnostics {
        version: nucifraga_core::VERSION,
        identifier: app.config().identifier.clone(),
        settings_file: state.settings_file.display().to_string(),
        fallback_reports: state.fallback_reports.display().to_string(),
        background_mode: state
            .settings
            .lock()
            .map(|s| s.background_mode)
            .unwrap_or(true),
    }
}

/// 設定頁「背景模式」。下一次備份起生效；正在跑的那一次不變。
#[tauri::command]
pub fn set_background_mode(state: State<'_, AppState>, on: bool) -> R<()> {
    let mut s = state.settings.lock().map_err(internal)?;
    s.background_mode = on;
    let snapshot = s.clone();
    drop(s);
    save_settings(&state, &snapshot)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppFolder {
    Settings,
    Reports,
}

/// 打開 app 自己的資料夾（設定檔所在、找不到備份碟時的報告）。只有這兩個，前端不能指定路徑。
#[tauri::command]
pub fn open_app_folder(state: State<'_, AppState>, which: AppFolder) -> R<()> {
    let dir = match which {
        AppFolder::Settings => state
            .settings_file
            .parent()
            .ok_or_else(|| internal("設定檔沒有上層資料夾"))?
            .to_path_buf(),
        AppFolder::Reports => state.fallback_reports.clone(),
    };
    // 兩個都是 app 自己的資料夾：還沒用過就先建，免得按了沒反應。
    std::fs::create_dir_all(&dir).map_err(internal)?;
    explorer(&dir)
}

#[tauri::command]
pub fn set_unsaved(state: State<'_, AppState>, unsaved: bool) {
    state.unsaved.store(unsaved, Ordering::Relaxed);
}

/// 使用者在「有未儲存的修改」那一問選了離開之後呼叫。
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn explorer(target: &Path) -> R<()> {
    std::process::Command::new("explorer.exe")
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(internal)
}

/// 從清單移除（**不刪模板檔**）。
#[tauri::command]
pub fn forget_profile(state: State<'_, AppState>, path: String) -> R<()> {
    // 不再顯示的模板，它的排程也不該再跑。
    schedule::remove(Path::new(&path))?;
    let mut s = state.settings.lock().map_err(internal)?;
    s.profiles.retain(|p| p.path != path);
    let snapshot = s.clone();
    drop(s);
    save_settings(&state, &snapshot)
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RootForm {
    path: String,
    name: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileForm {
    title: String,
    destination: String,
    roots: Vec<RootForm>,
    exclude_dir_names: Vec<String>,
    exclude_globs: Vec<String>,
    block_long_paths: bool,
    verify_after_copy: bool,
    always_skip_locked: Vec<String>,
    schedule: Option<Schedule>,
}

#[tauri::command]
pub fn get_profile(state: State<'_, AppState>, path: String) -> R<ProfileForm> {
    let p = known_profile(&state, &path)?;
    let raw = &p.raw;
    Ok(ProfileForm {
        title: p.title.clone(),
        destination: raw.destination.root.clone(),
        roots: raw
            .roots
            .iter()
            .map(|r| RootForm {
                path: r.path.clone(),
                name: r.name.clone(),
            })
            .collect(),
        exclude_dir_names: raw.exclude_dir_names.clone(),
        exclude_globs: raw.nucifraga.exclude_globs.clone(),
        block_long_paths: raw.nucifraga.path_policy == PathPolicy::Block,
        verify_after_copy: raw.nucifraga.verify_after_copy,
        always_skip_locked: raw.nucifraga.always_skip_locked.clone(),
        schedule: raw.nucifraga.schedule.clone(),
    })
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("一定序列化得了")
}

/// 只把**改過的**欄位寫回去（ADR-0006），整份驗過才寫入。
#[tauri::command]
pub fn save_profile(state: State<'_, AppState>, path: String, form: ProfileForm) -> R<()> {
    let p = known_profile(&state, &path)?;
    let raw = &p.raw;
    let mut edits: Vec<(Vec<&str>, String)> = Vec::new();
    if form.title != p.title {
        edits.push((vec!["nucifraga", "title"], quote(&form.title)));
    }
    if form.destination != raw.destination.root {
        edits.push((vec!["destination", "root"], quote(&form.destination)));
    }
    let old_roots: Vec<RootForm> = raw
        .roots
        .iter()
        .map(|r| RootForm {
            path: r.path.clone(),
            name: r.name.clone(),
        })
        .collect();
    if form.roots != old_roots {
        // 原本的 pathCheck（backup-policy.json 的欄位）照樣保留。
        let specs: Vec<RootSpec> = form
            .roots
            .iter()
            .map(|r| RootSpec {
                path: r.path.clone(),
                name: r.name.clone().filter(|n| !n.trim().is_empty()),
                path_check: raw
                    .roots
                    .iter()
                    .find(|o| o.path.eq_ignore_ascii_case(&r.path))
                    .and_then(|o| o.path_check),
            })
            .collect();
        edits.push((vec!["roots"], json(&specs)));
    }
    if form.exclude_dir_names != raw.exclude_dir_names {
        edits.push((vec!["excludeDirNames"], json(&form.exclude_dir_names)));
    }
    let o = &raw.nucifraga;
    if form.exclude_globs != o.exclude_globs {
        edits.push((vec!["nucifraga", "excludeGlobs"], json(&form.exclude_globs)));
    }
    if form.block_long_paths != (o.path_policy == PathPolicy::Block) {
        edits.push((
            vec!["nucifraga", "pathPolicy"],
            quote(if form.block_long_paths {
                "block"
            } else {
                "warn"
            }),
        ));
    }
    if form.verify_after_copy != o.verify_after_copy {
        edits.push((
            vec!["nucifraga", "verifyAfterCopy"],
            json(&form.verify_after_copy),
        ));
    }
    if form.always_skip_locked != o.always_skip_locked {
        edits.push((
            vec!["nucifraga", "alwaysSkipLocked"],
            json(&form.always_skip_locked),
        ));
    }
    let schedule_changed = form.schedule != o.schedule;
    if schedule_changed {
        edits.push((vec!["nucifraga", "schedule"], json(&form.schedule)));
    }
    if edits.is_empty() {
        return Ok(());
    }
    profile::save_edits(Path::new(&path), &edits)?;
    // 模板先存好；工作排程器沒同步成功時回 APP-007，排程設定照樣在模板裡，下次啟動會再試。
    if schedule_changed || form.title != p.title {
        apply_schedule(Path::new(&path), &form.title, form.schedule.as_ref())?;
    }
    Ok(())
}

fn today() -> String {
    time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
        .date()
        .to_string()
}

fn apply_schedule(profile_file: &Path, title: &str, s: Option<&Schedule>) -> R<()> {
    match s {
        Some(s) => {
            let exe = std::env::current_exe().map_err(internal)?;
            let w = schedule::Wanted {
                profile_file,
                title,
                schedule: s,
            };
            schedule::install(&w, &exe, &today())
        }
        None => schedule::remove(profile_file),
    }
}

/// 讓工作排程器跟目前看得到的模板一致（啟動時、換模板資料夾後）。
pub fn reconcile_schedules(state: &AppState) {
    let s = match state.settings.lock() {
        Ok(s) => s.clone(),
        Err(_) => return,
    };
    let mut paths: Vec<PathBuf> = s
        .profiles_folder
        .as_ref()
        .and_then(|f| library::list(Path::new(f)).ok())
        .unwrap_or_default();
    for p in &s.profiles {
        let pb = PathBuf::from(&p.path);
        if !paths.iter().any(|q| library::same_folder(q, &pb)) {
            paths.push(pb);
        }
    }
    let loaded: Vec<Profile> = paths.iter().filter_map(|p| profile::load(p).ok()).collect();
    let wanted: Vec<schedule::Wanted> = loaded
        .iter()
        .filter_map(|p| {
            p.raw.nucifraga.schedule.as_ref().map(|s| schedule::Wanted {
                profile_file: &p.file,
                title: &p.title,
                schedule: s,
            })
        })
        .collect();
    let issues = match std::env::current_exe() {
        Ok(exe) => schedule::reconcile(&wanted, &exe, &today()),
        Err(e) => vec![internal(e)],
    };
    if let Ok(mut g) = state.schedule_issues.lock() {
        *g = issues;
    }
}

/// 排程開的 app（`--run <模板>`）：前端畫好第一個畫面後拿走，之後就沒有了。
#[tauri::command]
pub fn take_launch_request(state: State<'_, AppState>) -> R<Option<String>> {
    Ok(state.launch_request.lock().map_err(internal)?.take())
}

/// DST-003 的「改用這顆碟」：清掉模板記住的備份碟，下次執行時重新認碟。
#[tauri::command]
pub fn reset_destination(state: State<'_, AppState>, path: String) -> R<()> {
    known_profile(&state, &path)?;
    profile::write_value(Path::new(&path), &["nucifraga", "destinationId"], "null")
}

#[tauri::command]
pub fn start_run(app: AppHandle, state: State<'_, AppState>, path: String) -> R<()> {
    known_profile(&state, &path)?;
    let background = state.settings.lock().map_err(internal)?.background_mode;
    runctl::start(
        app,
        PathBuf::from(path),
        state.fallback_reports.clone(),
        background,
        state.run.clone(),
    )
    .map_err(internal)
}

fn send(state: &AppState, answer: Answer) -> R<()> {
    let g = state.run.lock().map_err(internal)?;
    let active = g.as_ref().ok_or_else(|| internal("目前沒有在備份"))?;
    active.answers.send(answer).map_err(internal)
}

#[tauri::command]
pub fn answer_confirmation(state: State<'_, AppState>, yes: bool) -> R<()> {
    send(&state, Answer::Confirm(yes))
}

/// 備份碟沒接上時的回答：`retry` ＝「已接上，繼續」。
#[tauri::command]
pub fn answer_destination(state: State<'_, AppState>, retry: bool) -> R<()> {
    send(&state, Answer::Destination(retry))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LockAnswer {
    Retry,
    Skip,
    SkipAndRemember,
    Cancel,
}

#[tauri::command]
pub fn answer_locks(state: State<'_, AppState>, answer: LockAnswer) -> R<()> {
    let d = match answer {
        LockAnswer::Retry => LockDecision::Retry,
        LockAnswer::Skip => LockDecision::Skip { remember: false },
        LockAnswer::SkipAndRemember => LockDecision::Skip { remember: true },
        LockAnswer::Cancel => LockDecision::Cancel,
    };
    send(&state, Answer::Lock(d))
}

/// 取消：做完手上這一個檔案就停。正在等使用者回答的話，一併當成「不要」。
#[tauri::command]
pub fn cancel_run(state: State<'_, AppState>, then_quit: bool) -> R<()> {
    let g = state.run.lock().map_err(internal)?;
    if let Some(a) = g.as_ref() {
        a.cancel.store(true, Ordering::Relaxed);
        if then_quit {
            a.quit_when_done.store(true, Ordering::Relaxed);
        }
        let _ = a.answers.send(Answer::Confirm(false));
        let _ = a.answers.send(Answer::Lock(LockDecision::Cancel));
        let _ = a.answers.send(Answer::Destination(false));
    }
    Ok(())
}

#[tauri::command]
pub fn history(state: State<'_, AppState>, path: String) -> R<Vec<serde_json::Value>> {
    let p = known_profile(&state, &path)?;
    Ok(report::list_reports(&p.destination, 100))
}

#[tauri::command]
pub fn versions(state: State<'_, AppState>, path: String) -> R<Vec<VersionStamp>> {
    let p = known_profile(&state, &path)?;
    Ok(fsops::list_versions(&p.destination))
}

/// 「永久刪除這個時間點」。**只在使用者在確認對話框按下按鈕後呼叫**；所有檢查在 fsops 裡。
#[tauri::command]
pub fn delete_version(state: State<'_, AppState>, path: String, stamp: String) -> R<VersionStamp> {
    let p = known_profile(&state, &path)?;
    let id = p
        .destination_id
        .clone()
        .ok_or_else(|| Issue::new(Code::Eng010, &[("path", &stamp)]))?;
    fsops::delete_version_stamp(&p.destination, &id, &stamp)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Place {
    Current,
    Report,
    Version,
}

/// 用檔案總管打開備份碟上的一個資料夾。只接受那份模板備份碟底下的三種位置。
#[tauri::command]
pub fn open_folder(
    state: State<'_, AppState>,
    path: String,
    place: Place,
    stamp: Option<String>,
) -> R<()> {
    let p = known_profile(&state, &path)?;
    let stamp_ok = |s: &Option<String>| {
        s.as_deref()
            .filter(|s| fsops::is_stamp(s))
            .map(str::to_string)
    };
    let target = match place {
        Place::Current => p.destination.join("current"),
        Place::Report => p
            .destination
            .join("_reports")
            .join(stamp_ok(&stamp).ok_or_else(|| internal("時間點格式不對"))?),
        Place::Version => p
            .destination
            .join("_versions")
            .join(stamp_ok(&stamp).ok_or_else(|| internal("時間點格式不對"))?),
    };
    if !target.exists() {
        return Err(Issue::new(
            Code::Dst001,
            &[("path", &target.display().to_string())],
        ));
    }
    explorer(&target)
}
