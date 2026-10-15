//! Tauri entry point: IPC commands, the menu-bar tray, and window lifecycle.
//!
//! The frontend can only ever send a card *id* over IPC. Paths, sizes and
//! methods all come from the last scan held in [`ScanState`], so a compromised
//! or buggy webview cannot ask the backend to delete arbitrary paths.

pub mod analyze;
pub mod apfs;
pub mod breakdown;
pub mod browse;
pub mod categories;
pub mod disk;
pub mod dupes;
pub mod exec;
pub mod google;
pub mod history;
pub mod ollama;
pub mod qos;
pub mod rules;
pub mod scan;
pub mod settings;
#[cfg(target_os = "macos")]
pub mod simulators;
pub mod snapshot;
pub mod superseded;
pub mod tree;
pub mod walker;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};

use breakdown::{Payload, Session, Slot};
use disk::DiskUsage;
use exec::{DryRun, ExecResult};
use history::HistoryEntry;
use scan::Card;
use settings::AppSettings;

/// Executor only ever acts on cards produced by the last scan — the frontend
/// sends an id, never paths.
#[derive(Default)]
pub struct ScanState(pub Mutex<HashMap<String, Card>>);

pub struct SettingsState(pub Mutex<AppSettings>);

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
                auto: false,
            },
        );
    }
    update_tray(&app);
    Ok(result)
}

/// The storage detail walk of the whole disk: the running one, or a new one.
/// Payloads go out as `breakdown` events; a finished walk is cached for the
/// next launch's preview.
fn ensure_breakdown(app: &AppHandle) -> Arc<Session> {
    app.state::<Slot>().ensure(|| {
        let data = std::path::Path::new("/System/Volumes/Data");
        let data = if data.is_dir() {
            data
        } else {
            std::path::Path::new("/")
        };
        let handle = app.clone();
        Session::start(
            data.to_path_buf(),
            scan::home(),
            apfs::read_root(),
            std::time::Duration::from_millis(200),
            move |p: Payload| {
                let _ = handle.emit("breakdown", &p);
                // Only a walk that finished is worth showing next launch; a
                // cancelled one would preview a half-measured disk.
                if !p.running && p.root.state == tree::State::Done {
                    let current = handle.state::<Slot>().current();
                    if let (Some(s), Ok(dir)) = (current, handle.path().app_data_dir()) {
                        breakdown::save_cache(&dir, &s.cached());
                    }
                }
            },
        )
    })
}

/// The walk the app started at launch, or a new one if that has finished.
/// Returns a payload at once; the rest arrive as `breakdown` events.
#[tauri::command(async)]
fn breakdown_start(app: AppHandle) -> Payload {
    ensure_breakdown(&app).payload()
}

/// Moves the focus (the row whose children travel with each payload) and
/// returns a payload at once, so opening a row never waits for the next tick.
#[tauri::command]
fn breakdown_focus(id: Option<usize>, slot: State<Slot>) -> Option<Payload> {
    let s = slot.current()?;
    s.set_focus(id);
    Some(s.payload())
}

/// Where each path sits in the storage detail tree, as the chain of rows
/// holding it. Read-only: it links rows to reclaim cards, which still delete
/// only through their own id.
#[tauri::command]
fn breakdown_locate(paths: Vec<String>, slot: State<Slot>) -> Vec<Vec<usize>> {
    let paths: Vec<std::path::PathBuf> = paths.into_iter().map(Into::into).collect();
    slot.current()
        .map(|s| s.locate(&paths))
        .unwrap_or_else(|| vec![vec![]; paths.len()])
}

/// Opens System Settings at Privacy & Security → Full Disk Access. A fixed
/// URL: nothing from the webview reaches the command.
#[tauri::command]
fn open_full_disk_access() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .status()
            .map_err(|e| e.to_string())
            .map(|_| ())
    }
    #[cfg(not(target_os = "macos"))]
    Err("Full Disk Access is a macOS setting".into())
}

#[tauri::command]
fn breakdown_cancel(slot: State<Slot>) {
    if let Some(s) = slot.current() {
        s.cancel();
    }
}

/// The last finished walk, shown while a new one runs.
#[tauri::command]
fn breakdown_cached(app: AppHandle) -> Option<Payload> {
    breakdown::load_cache(&app.path().app_data_dir().ok()?)
}

#[tauri::command]
fn history(app: AppHandle) -> Vec<HistoryEntry> {
    app.path()
        .app_data_dir()
        .map(|d| history::load(&d))
        .unwrap_or_default()
}

#[tauri::command]
fn get_settings(state: State<SettingsState>) -> AppSettings {
    state.0.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(
    new_settings: AppSettings,
    app: AppHandle,
    state: State<SettingsState>,
) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    settings::save(&dir, &new_settings);
    let was_menu_bar_only = state.0.lock().unwrap().menu_bar_only;
    if new_settings.menu_bar_only != was_menu_bar_only {
        apply_activation_policy(&app, new_settings.menu_bar_only);
        // Entering menu-bar mode tucks the window into the tray; leaving it
        // brings the window straight back.
        if let Some(w) = app.get_webview_window("main") {
            if new_settings.menu_bar_only {
                let _ = w.hide();
            } else {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
    }
    *state.0.lock().unwrap() = new_settings;
    update_tray(&app); // threshold may have changed the ⚠️ state
    Ok(())
}

/// Menu-bar-only mode: Accessory hides the Dock icon; the window and tray
/// keep working. No-op off macOS.
fn apply_activation_policy(app: &AppHandle, menu_bar_only: bool) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(if menu_bar_only {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, menu_bar_only);
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn tray_title(u: &DiskUsage, warn_below_gb: f64) -> String {
    let gb = u.free_kb as f64 / (1024.0 * 1024.0);
    if gb < warn_below_gb {
        format!("⚠️ {gb:.1} GB")
    } else {
        format!("{gb:.0} GB")
    }
}

fn warn_threshold(app: &AppHandle) -> f64 {
    app.try_state::<SettingsState>()
        .map(|s| s.0.lock().unwrap().notify_below_gb)
        .unwrap_or(15.0)
}

fn update_tray(app: &AppHandle) {
    let title = tray_title(&disk::usage(), warn_threshold(app));
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = handle.tray_by_id("main-tray") {
            let _ = tray.set_title(Some(title));
        }
    });
}

/// User-space notification without a plugin dependency.
fn notify(body: &str) {
    let script = format!(
        "display notification \"{}\" with title \"Alpheus\"",
        body.replace('"', "'")
    );
    let _ = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .output();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(ScanState::default())
        .manage(Slot::default())
        .manage(google::GoogleState::default())
        .invoke_handler(tauri::generate_handler![
            disk_usage,
            scan,
            dry_run,
            execute,
            history,
            get_settings,
            set_settings,
            google::google_connect,
            google::google_status,
            google::google_token,
            google::google_disconnect,
            breakdown_start,
            breakdown_focus,
            breakdown_cancel,
            breakdown_locate,
            open_full_disk_access,
            breakdown_cached
        ])
        .setup(|app| {
            let initial = app
                .path()
                .app_data_dir()
                .map(|d| settings::load(&d))
                .unwrap_or_default();
            let initial_threshold = initial.notify_below_gb;
            apply_activation_policy(app.handle(), initial.menu_bar_only);
            // Menu-bar-only launches (e.g. at login) start in the tray.
            if initial.menu_bar_only {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            app.manage(SettingsState(Mutex::new(initial)));

            // Start measuring at launch, before the window has even loaded,
            // so it opens onto a walk already under way. Low priority: the
            // reclaim scan keeps its speed (PLAN.md, Gate A).
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                ensure_breakdown(&handle);
            });

            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .icon_as_template(true)
                .title(tray_title(&disk::usage(), initial_threshold))
                .tooltip("Alpheus")
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

            // One background thread: tray refresh, low-space notification with
            // hysteresis, and the optional automatic rescan.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut warned_low = false;
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(60));
                    update_tray(&handle);

                    let (interval_secs, threshold_gb, auto_clean, auto_ids, last_scan_ts) = {
                        let s = handle.state::<SettingsState>();
                        let s = s.0.lock().unwrap();
                        (
                            s.auto_scan_secs,
                            s.notify_below_gb,
                            s.auto_clean,
                            s.auto_clean_ids.clone(),
                            s.last_auto_scan_ts,
                        )
                    };

                    let free_gb = disk::usage().free_kb as f64 / (1024.0 * 1024.0);
                    if free_gb < threshold_gb {
                        if !warned_low {
                            warned_low = true;
                            notify(&format!(
                                "Free space is down to {free_gb:.1} GB — open Alpheus to reclaim."
                            ));
                        }
                    } else {
                        warned_low = false;
                    }

                    if interval_secs > 0 && now_secs().saturating_sub(last_scan_ts) >= interval_secs
                    {
                        // Persist the timestamp so weekly/monthly cadences
                        // survive restarts instead of resetting to "now".
                        {
                            let s = handle.state::<SettingsState>();
                            let mut s = s.0.lock().unwrap();
                            s.last_auto_scan_ts = now_secs();
                            if let Ok(dir) = handle.path().app_data_dir() {
                                settings::save(&dir, &s);
                            }
                        }
                        let mut cards = scan::scan_all();

                        // Automatic cleanup: only safe-tier, delete-action
                        // cards the user marked — enforced here regardless of
                        // what the settings file claims. Same executor, same
                        // denylist and trash rules as a manual click.
                        if auto_clean {
                            let mut freed_kb: u64 = 0;
                            let mut cleaned: u32 = 0;
                            cards.retain(|card| {
                                let eligible = card.tier == scan::Tier::Safe
                                    && card.action == scan::ActionKind::Delete
                                    && auto_ids.contains(&card.id);
                                if !eligible {
                                    return true;
                                }
                                match exec::execute(card) {
                                    Ok(res) => {
                                        freed_kb += res.freed_kb;
                                        cleaned += 1;
                                        if let Ok(dir) = handle.path().app_data_dir() {
                                            history::append(
                                                &dir,
                                                HistoryEntry {
                                                    timestamp: now_secs(),
                                                    card_id: card.id.clone(),
                                                    title: card.title.clone(),
                                                    freed_kb: res.freed_kb,
                                                    method: res.method.clone(),
                                                    auto: true,
                                                },
                                            );
                                        }
                                        false // cleaned — drop from the list
                                    }
                                    Err(_) => true,
                                }
                            });
                            if cleaned > 0 {
                                notify(&format!(
                                    "Auto-cleaned {:.1} GB across {cleaned} categor{}",
                                    freed_kb as f64 / (1024.0 * 1024.0),
                                    if cleaned == 1 { "y" } else { "ies" }
                                ));
                                let _ = handle.emit(
                                    "auto-clean",
                                    serde_json::json!({ "freed_kb": freed_kb, "count": cleaned }),
                                );
                            }
                        }

                        let state = handle.state::<ScanState>();
                        *state.0.lock().unwrap() =
                            cards.iter().map(|c| (c.id.clone(), c.clone())).collect();
                        let _ = handle.emit("auto-scan", &cards);
                        update_tray(&handle);
                    }
                }
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
        .expect("error while running Alpheus");
}
