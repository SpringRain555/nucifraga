//! nucifraga 的桌機殼。生命週期（單一實例、就緒標記、關閉）見 `docs/architecture/app-lifecycle.md`。
//!
//! 前端能做的事只有這裡 `generate_handler!` 列出的 command（ADR-0007）。

// 正式版不開主控台視窗；除錯版留著，方便看 log。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// 命令回傳的錯誤是 `Issue`（帶著渲染好的中文訊息），理由同 nucifraga-core。
#![allow(clippy::result_large_err)]

mod commands;
mod runctl;
mod settings;

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use commands::AppState;

/// 啟動器等的就緒標記，位置：`<app_local_data_dir>\ui-ready.txt`。
const READY_MARKER: &str = "ui-ready.txt";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    identifier: String,
}

#[tauri::command]
fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: nucifraga_core::VERSION,
        identifier: app.config().identifier.clone(),
    }
}

#[derive(Serialize, Clone)]
struct CloseRequested {
    /// true：問「取消備份並關閉」；false：問「有未儲存的修改」。
    running: bool,
}

/// 前端畫出第一個畫面之後呼叫。寫就緒標記，讓 `tools\Launch.ps1` 知道可以退場了。
#[tauri::command]
fn app_ready(app: AppHandle) -> Result<(), String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(READY_MARKER), std::process::id().to_string()).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        // 必須最先註冊：第二個行程在這裡就把參數交給第一個並結束，不會建任何視窗（§16 第 1、2 題）。
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            // 排程時間到、app 已經開著：把「開始備份這份模板」交給前端（ADR-0010）。
            if let Some(path) = nucifraga_core::schedule::run_request(&args) {
                let _ = app.emit("launch-request", path);
            }
        }))
        // 只用來讓使用者挑資料夾與檔案：它回傳路徑，不給前端讀寫權（ADR-0007）。
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config = app.path().app_config_dir()?;
            let local = app.path().app_local_data_dir()?;
            let settings_file = config.join("settings.json");
            let loaded = settings::load(&settings_file);
            app.manage(AppState {
                settings_file,
                settings: Mutex::new(loaded.settings),
                settings_was_broken: loaded.was_broken,
                suggested_folder: app.path().document_dir()?.join("Nucifraga"),
                fallback_reports: local.join("reports"),
                run: Arc::new(Mutex::new(None)),
                unsaved: AtomicBool::new(false),
                launch_request: Mutex::new(nucifraga_core::schedule::run_request(std::env::args())),
                schedule_issues: Mutex::new(Vec::new()),
            });
            // 工作排程器跟模板同步。要跑 schtasks 好幾次，放到背景，不擋視窗出來。
            let handle = app.handle().clone();
            std::thread::spawn(move || commands::reconcile_schedules(&handle.state::<AppState>()));
            Ok(())
        })
        // 備份進行中、或模板有未儲存的修改時按關閉：先攔下來問（app-lifecycle.md 第 5 題）。
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                let running = state.run.lock().map(|g| g.is_some()).unwrap_or(false);
                let unsaved = state.unsaved.load(Ordering::Relaxed);
                if running || unsaved {
                    api.prevent_close();
                    let _ = window.emit("close-requested", CloseRequested { running });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            app_ready,
            commands::home,
            commands::add_profile,
            commands::create_profile,
            commands::forget_profile,
            commands::set_profiles_folder,
            commands::open_profiles_folder,
            commands::diagnostics,
            commands::open_app_folder,
            commands::set_unsaved,
            commands::quit_app,
            commands::set_background_mode,
            commands::take_launch_request,
            commands::get_profile,
            commands::save_profile,
            commands::reset_destination,
            commands::start_run,
            commands::answer_confirmation,
            commands::answer_locks,
            commands::answer_destination,
            commands::cancel_run,
            commands::history,
            commands::versions,
            commands::delete_version,
            commands::open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("nucifraga 無法啟動");
}
