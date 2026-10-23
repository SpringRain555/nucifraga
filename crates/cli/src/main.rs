//! `nucifraga-cli`：不開視窗執行一份模板（M1 起）。
//!
//! 離開碼的約定（REQ-0001）：0＝完成、1＝完成但有警告、2＝失敗、3＝已取消。

use clap::{Parser, Subcommand};
use nucifraga_core::error::Code;

#[derive(Parser)]
#[command(name = "nucifraga-cli", version, about = "nucifraga 的命令列介面")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 列出所有錯誤碼、層級與訊息（內容來自 docs/architecture/error-codes.md）
    Codes,
}

fn main() {
    match Cli::parse().command {
        Command::Codes => {
            for code in Code::ALL {
                let e = code.entry();
                println!("{code}\t{:?}\t{}", e.level, code.render_message(&[]));
            }
        }
    }
}
