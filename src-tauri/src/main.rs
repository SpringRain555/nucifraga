//! nucifraga 的桌機殼。生命週期（單一實例、就緒標記、關閉）見 `docs/architecture/app-lifecycle.md`。
//!
//! 前端能做的事只有這裡 `generate_handler!` 列出的 command（ADR-0007）。

// 正式版不開主控台視窗；除錯版留著，方便看 log。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;

use serde::Serialize;
use tauri::{AppHandle, Manager};

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
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![app_info, app_ready])
        .run(tauri::generate_context!())
        .expect("nucifraga 無法啟動");
}
