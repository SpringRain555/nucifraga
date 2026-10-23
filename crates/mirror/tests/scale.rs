//! 規模基準（REQ-0007）：大量小檔案，跑兩次，量時間。預設不跑（#[ignore]）——
//! 用 `tools\Bench-Scale.ps1` 跑，它會一併量峰值記憶體，結果寫進 docs/operations/benchmarks.md。
//!
//! 檔案數由環境變數 `NUCIFRAGA_SCALE_FILES` 決定（預設 500000）。資料夾在 %TEMP% 的亂數目錄裡，跑完自動清掉。

use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use nucifraga_core::locks::LockGroup;
use nucifraga_core::report::Report;
use nucifraga_core::run::{Confirmation, Decider, LockDecision, RunOptions, Runner};
use nucifraga_mirror::Mirror;

struct Yes;
impl Decider for Yes {
    fn confirm(&self, _: &Confirmation) -> bool {
        true
    }
    fn locked(&self, _: &[LockGroup]) -> LockDecision {
        LockDecision::Skip { remember: false }
    }
}

fn run(profile: &std::path::Path) -> (Report, f64) {
    let sink = |_e: nucifraga_core::run::Event| {};
    let t = Instant::now();
    let r = Runner::new(
        &Mirror,
        &Yes,
        &sink,
        Arc::new(AtomicBool::new(false)),
        RunOptions {
            fallback_report_dir: None,
        },
    )
    .run(profile);
    (r, t.elapsed().as_secs_f64())
}

#[test]
#[ignore = "規模基準，用 tools\\Bench-Scale.ps1 跑"]
fn scale() {
    let n: usize = std::env::var("NUCIFRAGA_SCALE_FILES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500_000);
    let dir = tempfile::Builder::new()
        .prefix("nucifraga-scale-")
        .tempdir()
        .unwrap();
    let src = dir.path().join("src");
    let per_dir = 1000;
    let t = Instant::now();
    for d in 0..n.div_ceil(per_dir) {
        let sub = src.join(format!("d{d:04}"));
        fs::create_dir_all(&sub).unwrap();
        for f in 0..per_dir.min(n - d * per_dir) {
            fs::write(sub.join(format!("f{f:04}.txt")), format!("file {d}/{f}\n")).unwrap();
        }
    }
    println!(
        "BENCH generate files={n} seconds={:.1}",
        t.elapsed().as_secs_f64()
    );

    // NUCIFRAGA_SCALE_VERIFY=0：關掉讀回驗證，用來拆出它佔多少時間。
    let verify = std::env::var("NUCIFRAGA_SCALE_VERIFY").map_or(true, |v| v != "0");
    let profile = dir.path().join("p.json");
    let js = |p: &std::path::Path| serde_json::to_string(&p.display().to_string()).unwrap();
    fs::write(
        &profile,
        format!(
            r#"{{"schemaVersion":1,"destination":{{"root":{}}},"roots":[{{"path":{},"name":"S"}}],"nucifraga":{{"verifyAfterCopy":{verify}}}}}"#,
            js(&dir.path().join("dest")),
            js(&src)
        ),
    )
    .unwrap();
    println!("BENCH verifyAfterCopy={verify}");

    let (r1, s1) = run(&profile);
    println!(
        "BENCH first  outcome={:?} copied={} seconds={s1:.1}",
        r1.outcome, r1.counts.copied
    );
    let (r2, s2) = run(&profile);
    println!(
        "BENCH second outcome={:?} unchanged={} seconds={s2:.1}",
        r2.outcome, r2.counts.unchanged
    );
    assert_eq!(r2.counts.unchanged as usize, n);
}
