//! 模板：讀、驗、寫回。格式見 data-model.md §1；與 `backup-policy.json` 相容（ADR-0006）。

use std::fs;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

use crate::error::Code;
use crate::jsonedit;
use crate::report::Issue;

pub const SCHEMA_VERSION: u32 = 1;

// ── 檔案上的形狀（只用來反序列化；不認得的欄位一律忽略、保留在原文裡） ──

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileFile {
    pub schema_version: u32,
    pub destination: DestinationSpec,
    pub roots: Vec<RootSpec>,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub exclude_dir_names: Vec<String>,
    #[serde(default)]
    pub nucifraga: Options,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DestinationSpec {
    pub root: String,
    /// 舊做法的快照名；讀到但不使用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootSpec {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_check: Option<bool>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Limits {
    pub dir_path: usize,
    pub file_path: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            dir_path: 247,
            file_path: 259,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PathPolicy {
    Warn,
    Block,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    pub title: Option<String>,
    pub format: String,
    pub exclude_globs: Vec<String>,
    pub follow_reparse_points: bool,
    pub skip_cloud_placeholders: bool,
    pub path_policy: PathPolicy,
    pub verify_after_copy: bool,
    pub versioning: Versioning,
    pub delete_guard: DeleteGuard,
    pub always_skip_locked: Vec<String>,
    pub destination_id: Option<String>,
    /// 排程（ADR-0010）。`None`＝不排程。
    pub schedule: Option<crate::schedule::Schedule>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            title: None,
            format: "mirror".into(),
            exclude_globs: Vec::new(),
            follow_reparse_points: false,
            skip_cloud_placeholders: true,
            path_policy: PathPolicy::Warn,
            verify_after_copy: true,
            versioning: Versioning::default(),
            delete_guard: DeleteGuard::default(),
            always_skip_locked: Vec::new(),
            destination_id: None,
            schedule: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Versioning {
    /// `None`＝永久保留（REQ-0006）。M1 不實作自動清理。
    pub retention_days: Option<u32>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeleteGuard {
    pub min_count: u64,
    pub max_ratio: f64,
}

impl Default for DeleteGuard {
    fn default() -> Self {
        DeleteGuard {
            min_count: 50,
            max_ratio: 0.05,
        }
    }
}

// ── 驗過的模板 ──

#[derive(Debug, Clone)]
pub struct Root {
    /// 在備份碟上的資料夾名（`current\<name>`）。
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub file: PathBuf,
    pub title: String,
    pub destination: PathBuf,
    pub roots: Vec<Root>,
    pub limits: Limits,
    /// 小寫，比對時不分大小寫。
    pub exclude_dir_names: Vec<String>,
    pub exclude_globs: GlobSet,
    pub always_skip_locked: GlobSet,
    pub skip_cloud_placeholders: bool,
    pub path_policy: PathPolicy,
    pub verify_after_copy: bool,
    pub delete_guard: DeleteGuard,
    pub destination_id: Option<String>,
    pub raw: ProfileFile,
}

/// 讀並驗證一份模板。任何問題都回 `fatal` 的 `CFG-*`。
pub fn load(file: &Path) -> Result<Profile, Issue> {
    let path_text = file.display().to_string();
    let text = fs::read_to_string(file)
        .map_err(|e| Issue::new(Code::Cfg001, &[]).detail(format!("{path_text}：{e}")))?;
    parse(file, &text)
}

pub fn parse(file: &Path, text: &str) -> Result<Profile, Issue> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let raw: ProfileFile = serde_json::from_str(text).map_err(|e| {
        let line = e.line().to_string();
        Issue::new(Code::Cfg002, &[("line", &line)]).detail(e.to_string())
    })?;
    validate(file, raw)
}

fn invalid(field: &str, reason: &str) -> Issue {
    Issue::new(Code::Cfg007, &[("field", field), ("reason", reason)])
}

fn validate(file: &Path, raw: ProfileFile) -> Result<Profile, Issue> {
    if raw.schema_version != SCHEMA_VERSION {
        let found = raw.schema_version.to_string();
        return Err(Issue::new(Code::Cfg003, &[("found", &found)]));
    }
    let o = &raw.nucifraga;
    if o.format != "mirror" {
        return Err(invalid("nucifraga.format", "目前只支援 \"mirror\""));
    }
    if o.follow_reparse_points {
        return Err(invalid(
            "nucifraga.followReparsePoints",
            "固定是 false：不跟隨捷徑型資料夾",
        ));
    }
    if !(0.0..=1.0).contains(&o.delete_guard.max_ratio) {
        return Err(invalid(
            "nucifraga.deleteGuard.maxRatio",
            "要在 0 到 1 之間",
        ));
    }
    if let Some(s) = &o.schedule
        && let Err(reason) = s.validate()
    {
        return Err(invalid("nucifraga.schedule", &reason));
    }
    if raw.roots.is_empty() {
        return Err(invalid("roots", "至少要有一個來源資料夾"));
    }

    let destination = absolute(&raw.destination.root)?;
    let mut roots: Vec<Root> = Vec::new();
    for spec in &raw.roots {
        let path = absolute(&spec.path)?;
        let name = match &spec.name {
            Some(n) => n.clone(),
            None => default_name(&path),
        };
        if !valid_name(&name) {
            return Err(invalid(
                "roots[].name",
                &format!("「{name}」不能當資料夾名"),
            ));
        }
        if roots
            .iter()
            .any(|r| r.name.to_lowercase() == name.to_lowercase())
        {
            return Err(Issue::new(Code::Cfg006, &[("name", &name)]));
        }
        if contains(&path, &destination) || contains(&destination, &path) {
            return Err(Issue::new(
                Code::Cfg005,
                &[("path", &destination.display().to_string())],
            ));
        }
        roots.push(Root { name, path });
    }

    let exclude_globs = globs(&o.exclude_globs, "nucifraga.excludeGlobs")?;
    let always_skip_locked = globs(&o.always_skip_locked, "nucifraga.alwaysSkipLocked")?;
    let title = o.title.clone().unwrap_or_else(|| {
        file.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "模板".into())
    });

    Ok(Profile {
        file: file.to_path_buf(),
        title,
        destination,
        roots,
        limits: raw.limits,
        exclude_dir_names: raw
            .exclude_dir_names
            .iter()
            .map(|s| s.to_lowercase())
            .collect(),
        exclude_globs,
        always_skip_locked,
        skip_cloud_placeholders: o.skip_cloud_placeholders,
        path_policy: o.path_policy,
        verify_after_copy: o.verify_after_copy,
        delete_guard: o.delete_guard,
        destination_id: o.destination_id.clone(),
        raw,
    })
}

fn absolute(text: &str) -> Result<PathBuf, Issue> {
    let p = PathBuf::from(text);
    if !p.is_absolute() {
        return Err(Issue::new(Code::Cfg004, &[("path", text)]));
    }
    Ok(p)
}

/// 路徑的最後一段；磁碟根目錄（`D:\`）用 `D-drive`。
pub fn default_name(path: &Path) -> String {
    match path.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => {
            let s = path.to_string_lossy();
            let letter: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
            format!("{letter}-drive")
        }
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && !name.chars().any(|c| {
            matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
        })
        && !name.ends_with(['.', ' '])
        && name != ".."
}

/// `outer` 是否包含 `inner`（或相同）。比對時不分大小寫、以路徑段為單位。
pub fn contains(outer: &Path, inner: &Path) -> bool {
    let norm = |p: &Path| -> String {
        let s = p.to_string_lossy().replace('/', "\\").to_lowercase();
        let s = s.trim_end_matches('\\').to_string();
        format!("{s}\\")
    };
    norm(inner).starts_with(&norm(outer))
}

fn globs(patterns: &[String], field: &str) -> Result<GlobSet, Issue> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        let g = GlobBuilder::new(p)
            .case_insensitive(true)
            .literal_separator(true)
            .build()
            .map_err(|e| invalid(field, &e.to_string()))?;
        b.add(g);
    }
    b.build().map_err(|e| invalid(field, &e.to_string()))
}

/// 把模板檔裡一個值換掉：只動那一個值的位元組（ADR-0006）。BOM 有就保留。
/// 失敗是 `CFG-008`（警告）：備份照做，只是下次還會再問一次同樣的事。
pub fn write_value(file: &Path, path: &[&str], json: &str) -> Result<(), Issue> {
    let warn = |e: String| Issue::new(Code::Cfg008, &[("detail", &e)]);
    let bytes = fs::read(file).map_err(|e| warn(e.to_string()))?;
    let (bom, text) = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => (true, String::from_utf8_lossy(rest).into_owned()),
        None => (false, String::from_utf8_lossy(&bytes).into_owned()),
    };
    let edited = jsonedit::set_value(&text, path, json).map_err(|e| warn(format!("{e:?}")))?;
    serde_json::from_str::<serde_json::Value>(&edited)
        .map_err(|e| warn(format!("寫回後不是合法 JSON，沒有寫入：{e}")))?;
    let mut out = if bom {
        vec![0xEF, 0xBB, 0xBF]
    } else {
        Vec::new()
    };
    out.extend_from_slice(edited.as_bytes());
    fs::write(file, out).map_err(|e| warn(e.to_string()))
}

/// 模板編輯畫面的「儲存」：依序做多個最小替換，**整份驗過才寫入**。驗不過就回那個 `CFG-*`，檔案不動。
pub fn save_edits(file: &Path, edits: &[(Vec<&str>, String)]) -> Result<Profile, Issue> {
    let bytes = fs::read(file).map_err(|e| Issue::new(Code::Cfg001, &[]).detail(e.to_string()))?;
    let (bom, mut text) = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => (true, String::from_utf8_lossy(rest).into_owned()),
        None => (false, String::from_utf8_lossy(&bytes).into_owned()),
    };
    for (path, json) in edits {
        text = jsonedit::set_value(&text, path, json)
            .map_err(|e| invalid(&path.join("."), &format!("{e:?}")))?;
    }
    let profile = parse(file, &text)?;
    let mut out = if bom {
        vec![0xEF, 0xBB, 0xBF]
    } else {
        Vec::new()
    };
    out.extend_from_slice(text.as_bytes());
    fs::write(file, out).map_err(|e| Issue::new(Code::Cfg008, &[("detail", &e.to_string())]))?;
    Ok(profile)
}

/// 建立新模板檔（「建立新模板」）。已經存在就不覆蓋。
pub fn create(
    file: &Path,
    title: &str,
    destination: &str,
    roots: &[String],
) -> Result<Profile, Issue> {
    if file.exists() {
        return Err(invalid("path", "這個位置已經有檔案了，請換一個檔名"));
    }
    let doc = serde_json::json!({
        "schemaVersion": SCHEMA_VERSION,
        "destination": { "root": destination },
        "roots": roots.iter().map(|r| serde_json::json!({ "path": r })).collect::<Vec<_>>(),
        "excludeDirNames": ["node_modules", ".venv", "venv", "__pycache__", ".pytest_cache", "target"],
        "nucifraga": { "title": title },
    });
    let text = serde_json::to_string_pretty(&doc).expect("一定序列化得了") + "\n";
    let profile = parse(file, &text)?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| Issue::new(Code::Cfg008, &[("detail", &e.to_string())]))?;
    }
    fs::write(file, text).map_err(|e| Issue::new(Code::Cfg008, &[("detail", &e.to_string())]))?;
    Ok(profile)
}

pub fn write_destination_id(file: &Path, id: &str) -> Result<(), Issue> {
    write_value(file, &["nucifraga", "destinationId"], &jsonedit::quote(id))
}

/// 「以後都略過這些檔案」（REQ-0004）：把樣式加進 `nucifraga.alwaysSkipLocked`，已經有的不重複加。
pub fn append_always_skip_locked(file: &Path, patterns: &[String]) -> Result<(), Issue> {
    let current = load(file)
        .map(|p| p.raw.nucifraga.always_skip_locked)
        .unwrap_or_default();
    let mut all = current.clone();
    for p in patterns {
        if !all.contains(p) {
            all.push(p.clone());
        }
    }
    if all == current {
        return Ok(());
    }
    write_value(
        file,
        &["nucifraga", "alwaysSkipLocked"],
        &serde_json::to_string(&all).expect("字串陣列一定序列化得了"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> Result<Profile, Issue> {
        parse(Path::new(r"C:\profiles\weekly.json"), text)
    }

    const MIN: &str = r#"{"schemaVersion":1,"destination":{"root":"E:\\Backup"},"roots":[{"path":"D:\\Documents"}]}"#;

    #[test]
    fn minimal_profile_gets_defaults() {
        let pr = p(MIN).unwrap();
        assert_eq!(pr.title, "weekly");
        assert_eq!(pr.roots[0].name, "Documents");
        assert!(pr.verify_after_copy);
        assert_eq!(pr.limits.dir_path, 247);
        assert_eq!(pr.delete_guard.min_count, 50);
    }

    #[test]
    fn the_real_backup_policy_shape_is_accepted() {
        let text = include_str!("../../../examples/profiles/full.json");
        let pr = p(text).unwrap();
        assert_eq!(pr.roots.len(), 3);
        assert_eq!(pr.roots[2].name, "Documents-F");
        assert_eq!(pr.exclude_dir_names[0], "node_modules");
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let text = MIN.replace("\"roots\"", "\"somebodyElse\":{\"x\":1},\"roots\"");
        assert!(p(&text).is_ok());
    }

    fn code_of(text: &str) -> Code {
        p(text).unwrap_err().code
    }

    #[test]
    fn validation_codes() {
        assert_eq!(code_of("{"), Code::Cfg002);
        assert_eq!(
            code_of(&MIN.replace("\"schemaVersion\":1", "\"schemaVersion\":2")),
            Code::Cfg003
        );
        assert_eq!(
            code_of(&MIN.replace("E:\\\\Backup", "Backup")),
            Code::Cfg004
        );
        assert_eq!(
            code_of(&MIN.replace("E:\\\\Backup", "D:\\\\Documents\\\\bk")),
            Code::Cfg005
        );
        assert_eq!(
            code_of(&MIN.replace("E:\\\\Backup", "D:\\\\")),
            Code::Cfg005
        );
        let dup = MIN.replace(
            r#"[{"path":"D:\\Documents"}]"#,
            r#"[{"path":"D:\\Documents"},{"path":"F:\\documents"}]"#,
        );
        assert_eq!(code_of(&dup), Code::Cfg006);
        let fmt = MIN.replace("]}", r#"],"nucifraga":{"format":"restic"}}"#);
        assert_eq!(code_of(&fmt), Code::Cfg007);
    }

    #[test]
    fn overlap_check_respects_segment_boundaries() {
        assert!(contains(Path::new(r"D:\Docs"), Path::new(r"d:\docs\a")));
        assert!(!contains(Path::new(r"D:\Docs"), Path::new(r"D:\Docs2")));
    }

    #[test]
    fn drive_root_gets_a_readable_name() {
        assert_eq!(default_name(Path::new(r"D:\")), "D-drive");
    }

    #[test]
    fn invalid_edits_leave_the_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("p.json");
        fs::write(&f, MIN).unwrap();
        let err =
            save_edits(&f, &[(vec!["destination", "root"], "\"relative\"".into())]).unwrap_err();
        assert_eq!(err.code, Code::Cfg004);
        assert_eq!(fs::read_to_string(&f).unwrap(), MIN);
        let ok = save_edits(&f, &[(vec!["nucifraga", "title"], "\"新名字\"".into())]).unwrap();
        assert_eq!(ok.title, "新名字");
    }

    #[test]
    fn create_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("new.json");
        create(&f, "週備份", r"E:\Backup", &[r"D:\Documents".into()]).unwrap();
        assert_eq!(load(&f).unwrap().title, "週備份");
        assert_eq!(
            create(&f, "x", r"E:\B", &[r"D:\A".into()])
                .unwrap_err()
                .code,
            Code::Cfg007
        );
    }

    #[test]
    fn destination_id_write_back_keeps_everything_else() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("p.json");
        let original = "\u{feff}{\r\n  \"schemaVersion\": 1,\r\n  \"destination\": { \"root\": \"E:\\\\B\" },\r\n  \"roots\": [ { \"path\": \"D:\\\\A\" } ]\r\n}\r\n";
        fs::write(&f, original).unwrap();
        write_destination_id(&f, "abc").unwrap();
        let after = fs::read_to_string(&f).unwrap();
        assert!(after.starts_with('\u{feff}'), "BOM 要保留");
        assert_eq!(
            after,
            original.replace(
                "} ]\r\n}",
                "} ],\r\n  \"nucifraga\": { \"destinationId\": \"abc\" }\r\n}"
            )
        );
        assert_eq!(load(&f).unwrap().destination_id.as_deref(), Some("abc"));
    }
}
