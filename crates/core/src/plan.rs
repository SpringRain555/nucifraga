//! 比對：來源現在 vs 索引裡的上一次 → 這次要做什麼。
//!
//! 只比「來源現在」與「來源上一次」的大小與 mtime（ADR-0005）。內容有沒有真的變，
//! 留給 engine 用雜湊判斷 —— mtime 被碰過但內容沒變的檔案，不會多出一份舊版本。

use std::collections::HashMap;

use serde::Serialize;

use crate::catalog::Entry;
use crate::profile::DeleteGuard;
use crate::scan::RootScan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// 索引裡沒有。備份碟上若已經有同名檔，engine 會比對雜湊再決定（重建後、或上次中斷時）。
    Add,
    /// 大小或 mtime 跟上一次不同，或上一次的雜湊未知。
    Update,
    /// 來源已經沒有這個檔案 → 搬到舊版本。
    Remove,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub root: usize,
    pub rel: String,
    pub key: String,
    pub action: Action,
    /// 來源現在的大小與 mtime（`Remove` 時是索引裡的值）。
    pub size: u64,
    pub mtime_ns: i64,
    pub previous: Option<Entry>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootStats {
    pub name: String,
    pub tracked: u64,
    pub adds: u64,
    pub updates: u64,
    pub removes: u64,
    pub unchanged: u64,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub jobs: Vec<Job>,
    pub roots: Vec<RootStats>,
    pub unchanged: u64,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// 這次要寫進備份碟的位元組，每個檔案以配置單位往上取整（exFAT 的配置單位可能很大）。
    pub fn bytes_needed(&self, cluster: u64) -> u64 {
        self.jobs
            .iter()
            .filter(|j| j.action != Action::Remove)
            .map(|j| j.size.div_ceil(cluster.max(1)) * cluster.max(1))
            .sum()
    }

    pub fn bytes_to_copy(&self) -> u64 {
        self.jobs
            .iter()
            .filter(|j| j.action != Action::Remove)
            .map(|j| j.size)
            .sum()
    }
}

pub fn build(names: &[String], scans: &[RootScan], previous: &[HashMap<String, Entry>]) -> Plan {
    let mut plan = Plan::default();
    for (i, scan) in scans.iter().enumerate() {
        let prev = &previous[i];
        let mut stats = RootStats {
            name: names[i].clone(),
            tracked: prev.len() as u64,
            ..Default::default()
        };
        if scan.missing {
            // SRC-001：整個來源找不到時不做任何比對，備份碟上它的檔案原封不動。
            plan.roots.push(stats);
            continue;
        }
        let mut present: HashMap<&str, ()> = HashMap::with_capacity(scan.files.len());
        for f in &scan.files {
            present.insert(f.key.as_str(), ());
            let action = match prev.get(&f.key) {
                None => Some(Action::Add),
                Some(e) if e.size != f.size || e.mtime_ns != f.mtime_ns || e.sha256.is_none() => {
                    Some(Action::Update)
                }
                Some(_) => None,
            };
            match action {
                Some(a) => {
                    if a == Action::Add {
                        stats.adds += 1
                    } else {
                        stats.updates += 1
                    }
                    plan.jobs.push(Job {
                        root: i,
                        rel: f.rel.clone(),
                        key: f.key.clone(),
                        action: a,
                        size: f.size,
                        mtime_ns: f.mtime_ns,
                        previous: prev.get(&f.key).cloned(),
                    });
                }
                None => stats.unchanged += 1,
            }
        }
        for (key, e) in prev {
            if present.contains_key(key.as_str()) || scan.is_protected(key) {
                continue;
            }
            stats.removes += 1;
            plan.jobs.push(Job {
                root: i,
                rel: e.rel.clone(),
                key: key.clone(),
                action: Action::Remove,
                size: e.size,
                mtime_ns: e.mtime_ns,
                previous: Some(e.clone()),
            });
        }
        plan.unchanged += stats.unchanged;
        plan.roots.push(stats);
    }
    plan
}

/// 數量異常的「移到舊版本」（ENG-004）：≥ minCount 個**而且** ≥ maxRatio。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MassVersioning {
    pub root: String,
    pub count: u64,
    pub ratio: f64,
    pub sample: Vec<String>,
}

pub fn guard(plan: &Plan, g: &DeleteGuard) -> Vec<MassVersioning> {
    let mut out = Vec::new();
    // 舊版本來自「刪除」與「更新」兩種：更新會把舊檔搬過去，大量改寫同樣值得停下來看。
    // 但上一次雜湊未知的更新（索引剛重建）只是比對，不一定產生舊版本，不算進來 ——
    // 否則索引一重建，每一次都會被這道門擋下。
    let may_version = |j: &Job| match j.action {
        Action::Remove => true,
        Action::Update => j.previous.as_ref().is_some_and(|p| p.sha256.is_some()),
        Action::Add => false,
    };
    for (i, s) in plan.roots.iter().enumerate() {
        let count = plan
            .jobs
            .iter()
            .filter(|j| j.root == i && may_version(j))
            .count() as u64;
        if s.tracked == 0 || count < g.min_count {
            continue;
        }
        let ratio = count as f64 / s.tracked as f64;
        if ratio >= g.max_ratio {
            let sample = plan
                .jobs
                .iter()
                .filter(|j| j.root == i && may_version(j))
                .take(20)
                .map(|j| j.rel.clone())
                .collect();
            out.push(MassVersioning {
                root: s.name.clone(),
                count,
                ratio,
                sample,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::SourceFile;

    fn file(rel: &str, size: u64, mtime: i64) -> SourceFile {
        SourceFile {
            rel: rel.into(),
            key: rel.to_lowercase(),
            size,
            mtime_ns: mtime,
        }
    }

    fn prev(rel: &str, size: u64, mtime: i64) -> (String, Entry) {
        (
            rel.to_lowercase(),
            Entry {
                rel: rel.into(),
                size,
                mtime_ns: mtime,
                sha256: Some("h".into()),
            },
        )
    }

    fn one(scan: RootScan, p: Vec<(String, Entry)>) -> Plan {
        build(&["R".into()], &[scan], &[p.into_iter().collect()])
    }

    #[test]
    fn classifies_add_update_unchanged_remove() {
        let scan = RootScan {
            files: vec![file("new", 1, 1), file("same", 2, 2), file("touched", 3, 9)],
            ..Default::default()
        };
        let plan = one(
            scan,
            vec![
                prev("same", 2, 2),
                prev("touched", 3, 3),
                prev("gone", 4, 4),
            ],
        );
        let got: Vec<_> = plan
            .jobs
            .iter()
            .map(|j| (j.rel.as_str(), j.action))
            .collect();
        assert!(got.contains(&("new", Action::Add)));
        assert!(got.contains(&("touched", Action::Update)));
        assert!(got.contains(&("gone", Action::Remove)));
        assert_eq!(plan.unchanged, 1);
    }

    #[test]
    fn unknown_hash_forces_a_comparison() {
        let scan = RootScan {
            files: vec![file("a", 1, 1)],
            ..Default::default()
        };
        let mut p = prev("a", 1, 1);
        p.1.sha256 = None;
        assert_eq!(one(scan, vec![p]).jobs[0].action, Action::Update);
    }

    #[test]
    fn missing_root_never_removes_anything() {
        let scan = RootScan {
            missing: true,
            ..Default::default()
        };
        let plan = one(scan, vec![prev("a", 1, 1), prev("b", 1, 1)]);
        assert!(plan.jobs.is_empty());
    }

    #[test]
    fn protected_files_are_not_treated_as_deleted() {
        let scan = RootScan {
            protected: vec!["locked-dir/".into(), "cloud.docx".into()],
            ..Default::default()
        };
        let plan = one(
            scan,
            vec![
                prev("locked-dir/x", 1, 1),
                prev("cloud.docx", 1, 1),
                prev("really-gone", 1, 1),
            ],
        );
        let removed: Vec<_> = plan.jobs.iter().map(|j| j.rel.as_str()).collect();
        assert_eq!(removed, vec!["really-gone"]);
    }

    #[test]
    fn guard_trips_only_when_both_count_and_ratio_are_exceeded() {
        let g = DeleteGuard {
            min_count: 50,
            max_ratio: 0.05,
        };
        let prevs: Vec<_> = (0..1000).map(|i| prev(&format!("f{i}"), 1, 1)).collect();
        // 刪 60 個（6%）→ 停
        let keep: Vec<_> = (60..1000).map(|i| file(&format!("f{i}"), 1, 1)).collect();
        let plan = one(
            RootScan {
                files: keep,
                ..Default::default()
            },
            prevs.clone(),
        );
        assert_eq!(guard(&plan, &g)[0].count, 60);
        // 刪 40 個（4%）→ 不停
        let keep: Vec<_> = (40..1000).map(|i| file(&format!("f{i}"), 1, 1)).collect();
        assert!(
            guard(
                &one(
                    RootScan {
                        files: keep,
                        ..Default::default()
                    },
                    prevs
                ),
                &g
            )
            .is_empty()
        );
    }

    #[test]
    fn rebuilt_catalog_does_not_trip_the_guard() {
        let g = DeleteGuard {
            min_count: 50,
            max_ratio: 0.05,
        };
        let prevs: Vec<_> = (0..1000)
            .map(|i| {
                let (k, mut e) = prev(&format!("f{i}"), 1, 0);
                e.sha256 = None;
                (k, e)
            })
            .collect();
        let files: Vec<_> = (0..1000).map(|i| file(&format!("f{i}"), 1, 5)).collect();
        let plan = one(
            RootScan {
                files,
                ..Default::default()
            },
            prevs,
        );
        assert_eq!(plan.jobs.len(), 1000);
        assert!(guard(&plan, &g).is_empty());
    }

    #[test]
    fn bytes_needed_rounds_up_to_clusters() {
        let scan = RootScan {
            files: vec![file("a", 1, 1), file("b", 262_145, 1)],
            ..Default::default()
        };
        let plan = one(scan, vec![]);
        assert_eq!(plan.bytes_needed(262_144), 262_144 * 3);
    }
}
