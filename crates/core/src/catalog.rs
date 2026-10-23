//! 備份碟上的索引 `.nucifraga\catalog.sqlite`（ADR-0005）。**它是快取，不是正本**：
//! 壞了就改名保留、從 `current\` 重建，重建後的第一次執行會比較慢，但結果正確。
//!
//! 記的是**來源**的大小與 mtime。exFAT 的 mtime 只有 2 秒精度而且往上進位（2026-10-23 實測），
//! 拿備份碟上的時間跟來源比，會一直誤判成「有改」。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::Code;
use crate::longpath;
use crate::report::Issue;

const SCHEMA_VERSION: &str = "1";
/// 每累積這麼多筆寫入就 commit 一次：中斷時最多損失這麼多筆的索引更新，
/// 而損失的那幾筆在下一次會被當成「疑似變更」比對雜湊，不會重傳也不會多出舊版本。
const COMMIT_EVERY: usize = 500;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub rel: String,
    pub size: u64,
    pub mtime_ns: i64,
    pub sha256: Option<String>,
}

pub struct Catalog {
    conn: Connection,
    pending: usize,
}

pub struct Opened {
    pub catalog: Catalog,
    /// 這次是從 `current\` 重建的（`ENG-001`）。
    pub rebuilt: bool,
}

pub fn path_in(dest: &Path) -> PathBuf {
    longpath::extended(&dest.join(".nucifraga").join("catalog.sqlite"))
}

impl Catalog {
    /// 打開（或建立）索引。打不開或版本不符就改名保留並從 `current\` 重建。
    pub fn open(dest: &Path, stamp: &str) -> Result<Opened, Issue> {
        let file = path_in(dest);
        let existed = file.exists();
        if existed {
            match Self::try_open(&file) {
                Ok(c) => {
                    return Ok(Opened {
                        catalog: c,
                        rebuilt: false,
                    });
                }
                Err(_) => {
                    let broken = file.with_file_name(format!("catalog.broken-{stamp}.sqlite"));
                    fs::rename(&file, &broken).map_err(|e| fatal(e.to_string()))?;
                }
            }
        }
        let mut catalog = Self::create(&file).map_err(|e| fatal(e.to_string()))?;
        let current = longpath::extended(&dest.join("current"));
        let needs_rebuild = fs::read_dir(&current)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);
        if needs_rebuild {
            catalog
                .rebuild_from(&current)
                .map_err(|e| fatal(e.to_string()))?;
        }
        Ok(Opened {
            catalog,
            rebuilt: needs_rebuild,
        })
    }

    fn try_open(file: &Path) -> rusqlite::Result<Catalog> {
        let conn = Connection::open(file)?;
        let v: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if v.as_deref() != Some(SCHEMA_VERSION) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Self::configure(conn)
    }

    fn create(file: &Path) -> rusqlite::Result<Catalog> {
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let conn = Connection::open(file)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS entry (
               root TEXT NOT NULL, rel_path TEXT NOT NULL, rel_key TEXT NOT NULL,
               size INTEGER NOT NULL, mtime_ns INTEGER NOT NULL, sha256 TEXT,
               verified_at TEXT, last_run TEXT NOT NULL,
               PRIMARY KEY (root, rel_key));",
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
            [SCHEMA_VERSION],
        )?;
        Self::configure(conn)
    }

    fn configure(conn: Connection) -> rusqlite::Result<Catalog> {
        // 不用 WAL：外接碟可能被直接拔掉，WAL 的 -shm 在可攜式碟上沒有好處。
        conn.pragma_update(None, "journal_mode", "DELETE")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.execute_batch("BEGIN")?;
        Ok(Catalog { conn, pending: 0 })
    }

    /// 從 `current\<來源名>\` 重建：大小取實際值、mtime 記 0、雜湊記 NULL（data-model.md §4）。
    fn rebuild_from(&mut self, current: &Path) -> std::io::Result<()> {
        for root in fs::read_dir(current)? {
            let root = root?;
            if !root.file_type()?.is_dir() {
                continue;
            }
            let name = root.file_name().to_string_lossy().into_owned();
            let mut stack = vec![(root.path(), String::new())];
            while let Some((dir, rel_dir)) = stack.pop() {
                for e in fs::read_dir(&dir)? {
                    let e = e?;
                    let n = e.file_name().to_string_lossy().into_owned();
                    let rel = if rel_dir.is_empty() {
                        n
                    } else {
                        format!("{rel_dir}/{n}")
                    };
                    let meta = e.metadata()?;
                    if meta.is_dir() {
                        stack.push((e.path(), rel));
                    } else {
                        let entry = Entry {
                            rel,
                            size: meta.len(),
                            mtime_ns: 0,
                            sha256: None,
                        };
                        self.upsert(&name, &entry, "rebuild")
                            .map_err(std::io::Error::other)?;
                    }
                }
            }
        }
        self.commit().map_err(std::io::Error::other)
    }

    /// 一個來源的全部項目，key 是小寫的相對路徑。
    pub fn load_root(&self, root: &str) -> rusqlite::Result<HashMap<String, Entry>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT rel_key, rel_path, size, mtime_ns, sha256 FROM entry WHERE root = ?1",
        )?;
        let rows = stmt.query_map([root], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Entry {
                    rel: r.get(1)?,
                    size: r.get::<_, i64>(2)? as u64,
                    mtime_ns: r.get(3)?,
                    sha256: r.get(4)?,
                },
            ))
        })?;
        rows.collect()
    }

    pub fn upsert(&mut self, root: &str, e: &Entry, run: &str) -> rusqlite::Result<()> {
        self.conn
            .prepare_cached(
                "INSERT INTO entry (root, rel_path, rel_key, size, mtime_ns, sha256, verified_at, last_run)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, CASE WHEN ?6 IS NULL THEN NULL ELSE ?7 END, ?7)
                 ON CONFLICT (root, rel_key) DO UPDATE SET rel_path = excluded.rel_path, size = excluded.size,
                   mtime_ns = excluded.mtime_ns, sha256 = excluded.sha256, verified_at = excluded.verified_at,
                   last_run = excluded.last_run",
            )?
            .execute(params![root, e.rel, e.rel.to_lowercase(), e.size as i64, e.mtime_ns, e.sha256, run])?;
        self.bump()
    }

    pub fn remove(&mut self, root: &str, key: &str) -> rusqlite::Result<()> {
        self.conn
            .prepare_cached("DELETE FROM entry WHERE root = ?1 AND rel_key = ?2")?
            .execute(params![root, key])?;
        self.bump()
    }

    fn bump(&mut self) -> rusqlite::Result<()> {
        self.pending += 1;
        if self.pending >= COMMIT_EVERY {
            self.commit()
        } else {
            Ok(())
        }
    }

    pub fn commit(&mut self) -> rusqlite::Result<()> {
        self.conn.execute_batch("COMMIT; BEGIN")?;
        self.pending = 0;
        Ok(())
    }

    pub fn set_meta(&mut self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }
}

impl Drop for Catalog {
    fn drop(&mut self) {
        let _ = self.conn.execute_batch("COMMIT");
    }
}

fn fatal(detail: String) -> Issue {
    Issue::new(Code::Eng002, &[("detail", &detail)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(rel: &str, size: u64) -> Entry {
        Entry {
            rel: rel.into(),
            size,
            mtime_ns: 7,
            sha256: Some("ab".into()),
        }
    }

    #[test]
    fn roundtrip_and_case_insensitive_key() {
        let d = tempfile::tempdir().unwrap();
        {
            let mut c = Catalog::open(d.path(), "s").unwrap().catalog;
            c.upsert("Docs", &entry("A/B.txt", 3), "r1").unwrap();
            c.upsert("Docs", &entry("a/b.TXT", 4), "r2").unwrap();
        }
        let c = Catalog::open(d.path(), "s").unwrap();
        assert!(!c.rebuilt);
        let m = c.catalog.load_root("Docs").unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m["a/b.txt"].size, 4);
        assert_eq!(m["a/b.txt"].rel, "a/b.TXT");
    }

    #[test]
    fn corrupt_catalog_is_kept_aside_and_rebuilt_from_current() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join(".nucifraga")).unwrap();
        fs::write(
            d.path().join(".nucifraga/catalog.sqlite"),
            b"not a database",
        )
        .unwrap();
        fs::create_dir_all(d.path().join("current/Docs/sub")).unwrap();
        fs::write(d.path().join("current/Docs/sub/f.txt"), b"12345").unwrap();
        let o = Catalog::open(d.path(), "20261023-1200").unwrap();
        assert!(o.rebuilt);
        assert!(
            d.path()
                .join(".nucifraga/catalog.broken-20261023-1200.sqlite")
                .exists()
        );
        let m = o.catalog.load_root("Docs").unwrap();
        assert_eq!(
            m["sub/f.txt"],
            Entry {
                rel: "sub/f.txt".into(),
                size: 5,
                mtime_ns: 0,
                sha256: None
            }
        );
    }
}
