//! Tauri entry point: IPC commands, the menu-bar tray, and window lifecycle.
//!
//! The frontend can only ever send a card *id* over IPC. Paths, sizes and
//! methods all come from the last scan held in [`ScanState`], so a compromised
//! or buggy webview cannot ask the backend to delete arbitrary paths.

mod disk;
mod exec;
mod history;
mod scan;

use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State, WindowEvent,
};

use disk::DiskUsage;
use exec::{DryRun, ExecResult};
use history::HistoryEntry;
use scan::Card;

/// Executor only ever acts on cards produced by the last scan — the frontend
/// sends an id, never paths.
#[derive(Default)]
pub struct ScanState(pub Mutex<HashMap<String, Card>>);

#[tauri::command]
fn disk_usage() -> DiskUsage {
    disk::usage()
}

#[tauri::command(async)]
fn scan(state: State<ScanState>) -> Vec<Card> {
    let cards = scan::scan_all();
    *state.0.lock().unwrap() = cards.iter().map(|c| (c.id.clone(), c.clone())).collect();
    cards
}

#[tauri::command(async)]
fn dry_run(id: String, state: State<ScanState>) -> Result<DryRun, String> {
    let card = state
        .0
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or_else(|| "unknown card — rescan first".to_string())?;
    exec::dry_run(&card)
}

#[tauri::command(async)]
fn execute(id: String, app: AppHandle, state: State<ScanState>) -> Result<ExecResult, String> {
    let card = state
        .0
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or_else(|| "unknown card — rescan first".to_string())?;
    let result = exec::execute(&card)?;
    if let Ok(dir) = app.path().app_data_dir() {
        history::append(
            &dir,
            HistoryEntry {
                timestamp: now_secs(),
                card_id: card.id.clone(),
                title: card.title.clone(),
                freed_kb: result.freed_kb,
                method: result.method.clone(),
            },
        );
    }
    update_tray(&app);
    Ok(result)
}

#[tauri::command]
fn history(app: AppHandle) -> Vec<HistoryEntry> {
    app.path()
        .app_data_dir()
        .map(|d| history::load(&d))
        .unwrap_or_default()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn tray_title(u: &DiskUsage) -> String {
    let gb = u.free_kb as f64 / (1024.0 * 1024.0);
    if gb < 15.0 {
        format!("⚠️ {gb:.1} GB")
    } else {
        format!("{gb:.0} GB")
    }
}

fn update_tray(app: &AppHandle) {
    let title = tray_title(&disk::usage());
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = handle.tray_by_id("main-tray") {
            let _ = tray.set_title(Some(title));
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ScanState::default())
        .invoke_handler(tauri::generate_handler![
            disk_usage, scan, dry_run, execute, history
        ])
        .setup(|app| {
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .icon_as_template(true)
                .title(tray_title(&disk::usage()))
                .tooltip("Storage Manager")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(60));
                update_tray(&handle);
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the menu-bar presence alive.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Storage Manager");
}
