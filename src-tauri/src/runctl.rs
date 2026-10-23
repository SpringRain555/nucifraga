//! 在背景執行緒跑一次備份，事件轉給前端；兩個「等使用者」的狀態在這裡等前端的回答。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use nucifraga_core::locks::LockGroup;
use nucifraga_core::run::{Confirmation, Decider, Event, LockDecision, RunOptions, Runner};
use nucifraga_mirror::Mirror;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub enum Answer {
    Confirm(bool),
    Lock(LockDecision),
    /// 「已接上，繼續」（true）或取消（false）。
    Destination(bool),
}

/// 正在跑的那一次。同一時間只有一次（備份碟上還有作業系統層的執行鎖）。
pub struct Active {
    pub cancel: Arc<AtomicBool>,
    pub answers: Sender<Answer>,
    /// 使用者在備份中關視窗、選了「取消並關閉」：跑完就結束程式（app-lifecycle.md 第 5 題）。
    pub quit_when_done: Arc<AtomicBool>,
}

struct UiDecider {
    answers: Mutex<Receiver<Answer>>,
    cancel: Arc<AtomicBool>,
}

impl UiDecider {
    fn wait(&self) -> Option<Answer> {
        self.answers.lock().ok()?.recv().ok()
    }
}

impl Decider for UiDecider {
    fn can_wait_for_destination(&self) -> bool {
        true
    }

    fn destination_missing(&self, _: &str) -> bool {
        loop {
            match self.wait() {
                Some(Answer::Destination(retry)) => {
                    return retry && !self.cancel.load(Ordering::Relaxed);
                }
                Some(_) => continue,
                None => return false,
            }
        }
    }

    fn confirm(&self, _: &Confirmation) -> bool {
        // 事件已經由 Runner 送給前端；這裡等回答。視窗關掉（通道斷了）或取消就當成「不要」。
        loop {
            match self.wait() {
                Some(Answer::Confirm(yes)) => return yes && !self.cancel.load(Ordering::Relaxed),
                Some(_) => continue,
                None => return false,
            }
        }
    }

    fn locked(&self, _: &[LockGroup]) -> LockDecision {
        loop {
            match self.wait() {
                Some(Answer::Lock(d)) => return d,
                Some(_) => continue,
                None => return LockDecision::Cancel,
            }
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finished {
    pub report: nucifraga_core::report::Report,
    pub report_dir: Option<String>,
}

pub fn start(
    app: AppHandle,
    profile: PathBuf,
    fallback: PathBuf,
    background: bool,
    slot: Arc<Mutex<Option<Active>>>,
) -> Result<(), String> {
    let mut guard = slot.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Err("已經有一次備份在進行中。".into());
    }
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));
    *guard = Some(Active {
        cancel: cancel.clone(),
        answers: tx,
        quit_when_done: quit.clone(),
    });
    drop(guard);

    std::thread::spawn(move || {
        let decider = UiDecider {
            answers: Mutex::new(rx),
            cancel: cancel.clone(),
        };
        let emitter = app.clone();
        let sink = move |e: Event| {
            let _ = emitter.emit("run-event", &e);
        };
        let report = Runner::new(
            &Mirror,
            &decider,
            &sink,
            cancel,
            RunOptions {
                fallback_report_dir: Some(fallback),
                background,
            },
        )
        .run(&profile);
        let report_dir = report
            .written_to
            .as_ref()
            .map(|p| nucifraga_core::longpath::display(p));
        if let Ok(mut g) = slot.lock() {
            *g = None;
        }
        let _ = app.emit("run-finished", Finished { report, report_dir });
        if quit.load(Ordering::Relaxed) {
            app.exit(0);
        }
    });
    Ok(())
}
