//! `nucifraga-cli`：不開視窗執行一份模板。給工作排程器與端到端測試用。
//!
//! 離開碼（REQ-0001）：0＝完成、1＝完成但有警告、2＝失敗、3＝已取消。
//! 沒有人可以問的兩件事用固定的保守答案（state-machines.md「沒有人可以問的時候」）。

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use clap::{Parser, Subcommand};
use nucifraga_core::error::Code;
use nucifraga_core::locks::LockGroup;
use nucifraga_core::run::{
    Confirmation, Decider, Event, LockDecision, RunOptions, RunState, Runner,
};
use nucifraga_mirror::Mirror;

#[derive(Parser)]
#[command(name = "nucifraga-cli", version, about = "nucifraga 的命令列介面")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 執行一份模板
    Run {
        /// 模板檔（.json）
        profile: PathBuf,
        /// 「移到舊版本」數量異常時仍然繼續（預設：停下來，結果是已取消）
        #[arg(long)]
        allow_mass_versioning: bool,
        /// 讓這份模板使用一顆已經屬於別的模板的備份碟（預設：不使用，結果是已取消）
        #[arg(long)]
        adopt_destination: bool,
        /// 有檔案被佔用時取消整次備份（預設：略過那些檔案並記在報告裡）
        #[arg(long)]
        cancel_on_locked: bool,
        /// 結束時把完整報告以 JSON 印到標準輸出
        #[arg(long)]
        json: bool,
        /// Windows 背景模式：CPU 與磁碟讓給前景的程式（比較慢，但不打擾正在用電腦的人）
        #[arg(long)]
        background: bool,
    },
    /// 列出所有錯誤碼、層級與訊息（內容來自 docs/architecture/error-codes.md）
    Codes,
}

struct Flags {
    allow_mass_versioning: bool,
    adopt_destination: bool,
    cancel_on_locked: bool,
}

impl Decider for Flags {
    fn confirm(&self, c: &Confirmation) -> bool {
        match c {
            Confirmation::MassVersioning { .. } => self.allow_mass_versioning,
            Confirmation::AdoptDestination { .. } => self.adopt_destination,
        }
    }
    fn locked(&self, _: &[LockGroup]) -> LockDecision {
        if self.cancel_on_locked {
            LockDecision::Cancel
        } else {
            LockDecision::Skip { remember: false }
        }
    }
}

fn state_label(s: RunState) -> &'static str {
    match s {
        RunState::Idle => "待命",
        RunState::Preflight => "預檢",
        RunState::Scanning => "掃描",
        RunState::Planning => "比對",
        RunState::AwaitingConfirmation => "需確認",
        RunState::CheckingLocks => "檢查被佔用的檔案",
        RunState::AwaitingLockDecision => "需處理鎖定檔",
        RunState::Transferring => "傳輸",
        RunState::Finalizing => "收尾",
        RunState::Completed => "完成",
        RunState::CompletedWithWarnings => "完成但有警告",
        RunState::Failed => "失敗",
        RunState::Cancelled => "已取消",
    }
}

fn fallback_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|d| PathBuf::from(d).join("io.github.nucifraga").join("reports"))
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Codes => {
            for code in Code::ALL {
                let e = code.entry();
                println!("{code}\t{:?}\t{}", e.level, code.render_message(&[]));
            }
            ExitCode::SUCCESS
        }
        Command::Run {
            profile,
            allow_mass_versioning,
            adopt_destination,
            cancel_on_locked,
            json,
            background,
        } => {
            let flags = Flags {
                allow_mass_versioning,
                adopt_destination,
                cancel_on_locked,
            };
            let sink = |e: Event| match e {
                Event::State(s) => eprintln!("── {}", state_label(s)),
                Event::Issue(i) => eprintln!("   [{}] {}", i.code, i.message),
                _ => {}
            };
            let runner = Runner::new(
                &Mirror,
                &flags,
                &sink,
                Arc::new(AtomicBool::new(false)),
                RunOptions {
                    fallback_report_dir: fallback_dir(),
                    background,
                },
            );
            let report = runner.run(&profile);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("報告一定序列化得了")
                );
            } else {
                eprintln!();
                eprint!("{}", report.summary_text().replace("\r\n", "\n"));
            }
            if let Some(dir) = &report.written_to {
                eprintln!("\n報告：{}", nucifraga_core::longpath::display(dir));
            }
            ExitCode::from(report.outcome.exit_code() as u8)
        }
    }
}
