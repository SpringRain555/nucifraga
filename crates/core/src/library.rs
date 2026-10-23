//! 模板資料夾：新模板預設存在這裡，首頁列出資料夾裡的每一份（ADR-0006 補充）。
//!
//! 只看資料夾第一層的 `*.json`，不往下找：子資料夾留給使用者自己放備份或草稿。
//! 資料夾外的模板（例如 `D:\_meta\backup-policy.json`）照舊只記路徑，不在這個模組管。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::Code;
use crate::report::Issue;

/// 資料夾裡的模板檔，依檔名排序。資料夾不在就是 `APP-004`。
pub fn list(folder: &Path) -> Result<Vec<PathBuf>, Issue> {
    let missing = || Issue::new(Code::App004, &[("path", &folder.display().to_string())]);
    let entries = fs::read_dir(folder).map_err(|_| missing())?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| is_json(p))
        .collect();
    files.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    Ok(files)
}

fn is_json(p: &Path) -> bool {
    p.extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("json"))
}

/// 兩個路徑是不是同一個資料夾。Windows 的路徑不分大小寫，結尾的分隔符號不算。
pub fn same_folder(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        p.to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .replace('/', "\\")
            .to_lowercase()
    };
    norm(a) == norm(b)
}

/// `file` 是不是直接放在 `folder` 第一層的模板檔。
pub fn contains(folder: &Path, file: &Path) -> bool {
    is_json(file)
        && file
            .parent()
            .is_some_and(|parent| same_folder(parent, folder))
}

/// Windows 保留的裝置名稱：拿來當檔名會開到裝置而不是檔案。
const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// 檔名太長會吃掉路徑預算；名稱本身照樣完整存在模板裡。
const MAX_STEM_CHARS: usize = 60;

/// 模板名稱 → 檔名主體：換掉 Windows 不允許的字元，去掉結尾的點與空白。
pub fn file_stem_for(title: &str) -> String {
    let replaced: String = title
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .take(MAX_STEM_CHARS)
        .collect();
    let mut stem = replaced.trim().trim_end_matches(['.', ' ']).to_string();
    if stem.is_empty() {
        stem = "模板".into();
    }
    if RESERVED.contains(&stem.to_lowercase().as_str()) {
        stem.push('_');
    }
    stem
}

/// 新模板要寫到的位置：`<資料夾>\<名稱>.json`，已經有同名檔就加「 (2)」「 (3)」……
pub fn new_path(folder: &Path, title: &str) -> PathBuf {
    let stem = file_stem_for(title);
    let first = folder.join(format!("{stem}.json"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| folder.join(format!("{stem} ({n}).json")))
        .find(|p| !p.exists())
        .expect("總會找到沒用過的編號")
}

/// 確定資料夾可以用：不在就建，建不了是 `APP-006`。
pub fn ensure(folder: &Path) -> Result<(), Issue> {
    fs::create_dir_all(folder).map_err(|e| {
        Issue::new(
            Code::App006,
            &[
                ("path", &folder.display().to_string()),
                ("detail", &e.to_string()),
            ],
        )
    })
}

#[derive(Debug, Default)]
pub struct Moved {
    /// `(原位置, 新位置)`
    pub moved: Vec<(PathBuf, PathBuf)>,
    /// 沒搬的每一份一則 `APP-005`；那一份留在原處、一個位元組都沒動。
    pub issues: Vec<Issue>,
}

/// 把舊資料夾第一層的模板搬到新資料夾。**同名檔絕不覆蓋**：留在原處，回報 `APP-005`。
///
/// 只搬檔案、只刪「已經確定複製成功」的那一個原檔（跨碟時）；不碰資料夾本身。
pub fn move_all(from: &Path, to: &Path) -> Result<Moved, Issue> {
    ensure(to)?;
    let mut out = Moved::default();
    for src in list(from)? {
        let name = src.file_name().expect("list 只回檔案");
        let dst = to.join(name);
        let result = if dst.exists() {
            Err("新資料夾裡已經有同名的檔案".to_string())
        } else {
            move_file(&src, &dst).map_err(|e| e.to_string())
        };
        match result {
            Ok(()) => out.moved.push((src, dst)),
            Err(reason) => out.issues.push(Issue::new(
                Code::App005,
                &[("path", &src.display().to_string()), ("reason", &reason)],
            )),
        }
    }
    Ok(out)
}

fn move_file(src: &Path, dst: &Path) -> io::Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        // 跨碟改名不行：先複製、確認大小一樣，才刪原檔。
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            let len = fs::copy(src, dst)?;
            if fs::metadata(src)?.len() != len {
                let _ = fs::remove_file(dst);
                return Err(io::Error::other("複製出來的大小不對"));
            }
            fs::remove_file(src)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stem_replaces_forbidden_characters() {
        assert_eq!(file_stem_for("D:/專案 備份?"), "D__專案 備份_");
        assert_eq!(file_stem_for("  每週.  "), "每週");
        assert_eq!(file_stem_for("..."), "模板");
        assert_eq!(file_stem_for(""), "模板");
        assert_eq!(file_stem_for("CON"), "CON_");
        assert_eq!(
            file_stem_for(&"長".repeat(200)).chars().count(),
            MAX_STEM_CHARS
        );
    }

    #[test]
    fn new_path_never_reuses_an_existing_name() {
        let dir = tempfile::tempdir().unwrap();
        let a = new_path(dir.path(), "照片");
        assert_eq!(a, dir.path().join("照片.json"));
        fs::write(&a, "{}").unwrap();
        let b = new_path(dir.path(), "照片");
        assert_eq!(b, dir.path().join("照片 (2).json"));
        fs::write(&b, "{}").unwrap();
        assert_eq!(
            new_path(dir.path(), "照片"),
            dir.path().join("照片 (3).json")
        );
    }

    #[test]
    fn list_only_takes_json_on_the_first_level() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.json"), "{}").unwrap();
        fs::write(dir.path().join("A.JSON"), "{}").unwrap();
        fs::write(dir.path().join("note.txt"), "").unwrap();
        fs::create_dir(dir.path().join("old")).unwrap();
        fs::write(dir.path().join("old").join("c.json"), "{}").unwrap();
        let names: Vec<String> = list(dir.path())
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["A.JSON", "b.json"]);
    }

    #[test]
    fn missing_folder_is_app_004() {
        let dir = tempfile::tempdir().unwrap();
        let err = list(&dir.path().join("不存在")).unwrap_err();
        assert_eq!(err.code, Code::App004);
    }

    #[test]
    fn contains_ignores_case_and_trailing_separator() {
        let folder = Path::new(r"C:\Users\me\Documents\Nucifraga\");
        assert!(contains(
            folder,
            Path::new(r"c:\users\ME\documents\nucifraga\a.json")
        ));
        assert!(!contains(
            folder,
            Path::new(r"C:\Users\me\Documents\Nucifraga\sub\a.json")
        ));
        assert!(!contains(
            folder,
            Path::new(r"C:\Users\me\Documents\Nucifraga\a.txt")
        ));
        assert!(!contains(folder, Path::new(r"D:\_meta\backup-policy.json")));
    }

    #[test]
    fn move_all_keeps_conflicts_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("old");
        let to = dir.path().join("new");
        fs::create_dir_all(&from).unwrap();
        fs::create_dir_all(&to).unwrap();
        fs::write(from.join("a.json"), "舊 a").unwrap();
        fs::write(from.join("b.json"), "舊 b").unwrap();
        fs::write(to.join("b.json"), "新 b").unwrap();

        let r = move_all(&from, &to).unwrap();

        assert_eq!(r.moved.len(), 1);
        assert_eq!(fs::read_to_string(to.join("a.json")).unwrap(), "舊 a");
        assert!(!from.join("a.json").exists());
        // 撞名的兩邊都原封不動
        assert_eq!(fs::read_to_string(from.join("b.json")).unwrap(), "舊 b");
        assert_eq!(fs::read_to_string(to.join("b.json")).unwrap(), "新 b");
        assert_eq!(r.issues.len(), 1);
        assert_eq!(r.issues[0].code, Code::App005);
    }
}
