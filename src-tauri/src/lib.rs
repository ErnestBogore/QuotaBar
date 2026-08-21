mod app_server;
mod hooks;
mod logs;
mod store;

use chrono::{DateTime, Utc};
use hooks::HookManager;
use logs::{classify_transcript, SessionLogWatcher};
use quota_core::{
    should_block_client, ClientKind, GateSnapshotV1, MeterEngine, OverrideStatus, Settings,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use store::Store;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_positioner::{Position, WindowExt as PositionerWindowExt};
use tauri_plugin_updater::UpdaterExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration as TokioDuration};

#[derive(Debug)]
struct RuntimeState {
    engine: MeterEngine,
    sessions: HashMap<String, ClientKind>,
    classification_healthy: bool,
    last_notification_threshold: u8,
}

#[derive(Clone)]
struct AppRuntime {
    inner: Arc<RwLock<RuntimeState>>,
    store: Store,
    hook_manager: HookManager,
    sessions_root: PathBuf,
    socket_path: PathBuf,
}

impl AppRuntime {
    async fn snapshot(&self) -> GateSnapshotV1 {
        let hook_installed = self.hook_manager.is_installed();
        let mut inner = self.inner.write().await;
        let healthy = inner.classification_healthy;
        inner.engine.snapshot(Utc::now(), hook_installed, healthy)
    }

    async fn persist(&self) {
        let inner = self.inner.read().await;
        let _ = self.store.save_engine(&inner.engine);
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GateRequest {
    version: u8,
    session_id: String,
    transcript_path: Option<String>,
}

#[derive(Debug, Serialize)]
struct GateResponse {
    decision: &'static str,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverrideResponse {
    status: OverrideStatus,
    snapshot: GateSnapshotV1,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCheckResponse {
    current_version: String,
    available_version: Option<String>,
    notes: Option<String>,
}

#[tauri::command]
async fn get_state(runtime: State<'_, AppRuntime>) -> Result<GateSnapshotV1, String> {
    Ok(runtime.snapshot().await)
}

#[tauri::command]
async fn update_settings(
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
    settings: Settings,
) -> Result<GateSnapshotV1, String> {
    let settings = settings.normalized();
    {
        let mut inner = runtime.inner.write().await;
        inner.engine.set_settings(settings.clone());
    }
    if settings.launch_at_login {
        app.autolaunch()
            .enable()
            .map_err(|error| error.to_string())?;
    } else {
        app.autolaunch()
            .disable()
            .map_err(|error| error.to_string())?;
    }
    runtime.persist().await;
    Ok(runtime.snapshot().await)
}

#[tauri::command]
async fn request_override(
    runtime: State<'_, AppRuntime>,
    phrase: String,
) -> Result<OverrideResponse, String> {
    let status = {
        let mut inner = runtime.inner.write().await;
        inner
            .engine
            .request_override(&phrase, Utc::now())
            .map_err(|error| error.to_string())?
    };
    runtime.persist().await;
    Ok(OverrideResponse {
        status,
        snapshot: runtime.snapshot().await,
    })
}

#[tauri::command]
async fn install_desktop_hook(runtime: State<'_, AppRuntime>) -> Result<GateSnapshotV1, String> {
    runtime.hook_manager.install()?;
    Ok(runtime.snapshot().await)
}

#[tauri::command]
async fn repair_desktop_hook(runtime: State<'_, AppRuntime>) -> Result<GateSnapshotV1, String> {
    runtime.hook_manager.install()?;
    Ok(runtime.snapshot().await)
}

#[tauri::command]
async fn remove_desktop_hook(runtime: State<'_, AppRuntime>) -> Result<GateSnapshotV1, String> {
    runtime.hook_manager.remove()?;
    Ok(runtime.snapshot().await)
}

#[tauri::command]
async fn delete_history(runtime: State<'_, AppRuntime>) -> Result<GateSnapshotV1, String> {
    runtime.store.delete_history()?;
    {
        let mut inner = runtime.inner.write().await;
        inner.engine.reset_history();
    }
    runtime.persist().await;
    Ok(runtime.snapshot().await)
}

#[tauri::command]
fn hide_main_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[tauri::command]
async fn check_for_updates(app: AppHandle) -> Result<UpdateCheckResponse, String> {
    let current_version = app.package_info().version.to_string();
    let update = app
        .updater()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;
    Ok(UpdateCheckResponse {
        current_version,
        available_version: update.as_ref().map(|value| value.version.clone()),
        notes: update.and_then(|value| value.body),
    })
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app
        .updater()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "QuotaBar is already up to date".to_string())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| error.to_string())?;
    app.restart()
}

fn app_data_paths() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let data = dirs::data_dir()
        .ok_or("macOS Application Support directory is unavailable")?
        .join("QuotaBar");
    let sessions = dirs::home_dir()
        .ok_or("Home directory is unavailable")?
        .join(".codex/sessions");
    Ok((
        data.join("quotabar.sqlite3"),
        data.join("gate.sock"),
        sessions,
    ))
}

async fn handle_gate_request(runtime: &AppRuntime, request: GateRequest) -> GateResponse {
    if request.version != 1 {
        return GateResponse {
            decision: "allow",
            reason: None,
        };
    }
    let mut client = {
        let inner = runtime.inner.read().await;
        inner
            .sessions
            .get(&request.session_id)
            .cloned()
            .unwrap_or(ClientKind::Unknown)
    };
    if client == ClientKind::Unknown {
        if let Some(path) = request.transcript_path.as_deref() {
            client = classify_transcript(path, &runtime.sessions_root);
        }
    }
    if client == ClientKind::Desktop {
        let mut inner = runtime.inner.write().await;
        inner.classification_healthy = true;
        inner
            .sessions
            .insert(request.session_id.clone(), ClientKind::Desktop);
    }
    // Positive desktop evidence is mandatory. Every other client fails open.
    if client != ClientKind::Desktop {
        return GateResponse {
            decision: "allow",
            reason: None,
        };
    }
    let snapshot = runtime.snapshot().await;
    if should_block_client(
        &client,
        &snapshot.state,
        snapshot.desktop_classification_healthy,
    ) {
        let reset = snapshot
            .window_ends_at
            .map(format_reset)
            .unwrap_or_else(|| "the next reset".to_string());
        GateResponse {
            decision: "block",
            reason: Some(format!("QuotaBar's reconstructed five-hour allowance is exhausted. New Mac app prompts are paused until {reset}. CLI, IDE, browser, and cloud Codex remain available.")),
        }
    } else {
        GateResponse {
            decision: "allow",
            reason: None,
        }
    }
}

fn format_reset(value: DateTime<Utc>) -> String {
    value
        .with_timezone(&chrono::Local)
        .format("%H:%M")
        .to_string()
}

async fn serve_gate_socket(runtime: AppRuntime) {
    if let Some(parent) = runtime.socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::remove_file(&runtime.socket_path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => return,
    }
    let Ok(listener) = UnixListener::bind(&runtime.socket_path) else {
        return;
    };
    let _ = std::fs::set_permissions(&runtime.socket_path, std::fs::Permissions::from_mode(0o600));
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let runtime = runtime.clone();
        tauri::async_runtime::spawn(async move {
            let _ = process_gate_connection(runtime, stream).await;
        });
    }
}

async fn process_gate_connection(runtime: AppRuntime, stream: UnixStream) -> Result<(), String> {
    let (read, mut write) = stream.into_split();
    let mut line = String::new();
    BufReader::new(read)
        .read_line(&mut line)
        .await
        .map_err(|error| error.to_string())?;
    let request = serde_json::from_str::<GateRequest>(&line).map_err(|error| error.to_string())?;
    let response = handle_gate_request(&runtime, request).await;
    let encoded = serde_json::to_string(&response).map_err(|error| error.to_string())?;
    write
        .write_all(format!("{encoded}\n").as_bytes())
        .await
        .map_err(|error| error.to_string())
}

async fn watch_session_logs(runtime: AppRuntime) {
    let mut watcher = SessionLogWatcher::new(runtime.sessions_root.clone());
    loop {
        let connected = watcher.root_exists();
        let batch = watcher.scan();
        {
            let mut inner = runtime.inner.write().await;
            inner.engine.session_logs_connected = connected;
            for (session_id, client) in batch.sessions {
                if client == ClientKind::Desktop {
                    inner.classification_healthy = true;
                }
                inner.sessions.insert(session_id, client);
            }
            for rate in &batch.rates {
                if rate.quota_id == "codex" {
                    inner.engine.observe_rate_limit(rate);
                }
                let _ = runtime.store.record_rate(rate);
            }
            for usage in &batch.usage {
                inner.engine.observe_usage(usage);
                let _ = runtime.store.record_usage(usage);
            }
        }
        if !batch.rates.is_empty() || !batch.usage.is_empty() {
            runtime.persist().await;
        }
        sleep(TokioDuration::from_secs(2)).await;
    }
}

async fn poll_app_server(runtime: AppRuntime) {
    loop {
        let _ = runtime.store.prune_details(Utc::now());
        let Some(path) = app_server::locate_codex() else {
            runtime.inner.write().await.engine.app_server_connected = false;
            sleep(TokioDuration::from_secs(30)).await;
            continue;
        };
        let (sender, mut receiver) = mpsc::channel(4);
        let stream = app_server::stream_rate_limits(&path, sender);
        tokio::pin!(stream);
        loop {
            tokio::select! {
              result = &mut stream => {
                let _ = result;
                runtime.inner.write().await.engine.app_server_connected = false;
                runtime.persist().await;
                break;
              }
              snapshots = receiver.recv() => {
                let Some(snapshots) = snapshots else { break };
                let mut inner = runtime.inner.write().await;
                inner.engine.app_server_connected = true;
                inner.engine.set_official_buckets(&snapshots);
                for snapshot in &snapshots {
                    let _ = runtime.store.record_rate(snapshot);
                }
                drop(inner);
                runtime.persist().await;
              }
            }
        }
        sleep(TokioDuration::from_secs(5)).await;
    }
}

async fn refresh_ui(app: AppHandle, runtime: AppRuntime, quit_item: MenuItem<tauri::Wry>) {
    loop {
        let snapshot = runtime.snapshot().await;
        if let Some(tray) = app.tray_by_id("main") {
            let weekly = snapshot
                .weekly_remaining_percent
                .map(|value| format!("{}%", value.round()))
                .unwrap_or_else(|| "—".to_string());
            let _ = tray.set_title(Some(format!(
                "5h {}% · Week {weekly}",
                snapshot.five_hour_remaining_percent.round()
            )));
        }
        let locked = snapshot.state == quota_core::GateState::Exhausted;
        let _ = quit_item.set_enabled(!locked);
        let _ = app.emit("gate-state", &snapshot);
        maybe_notify(&app, &runtime, &snapshot).await;
        sleep(TokioDuration::from_secs(1)).await;
    }
}

async fn maybe_notify(app: &AppHandle, runtime: &AppRuntime, snapshot: &GateSnapshotV1) {
    let threshold = match snapshot.five_hour_used_percent {
        value if value >= 100.0 => 100,
        value if value >= 90.0 => 90,
        value if value >= 75.0 => 75,
        value if value >= 50.0 => 50,
        _ => 0,
    };
    let should_notify = {
        let mut inner = runtime.inner.write().await;
        if threshold > inner.last_notification_threshold
            && inner.engine.settings.notifications_enabled
        {
            inner.last_notification_threshold = threshold;
            true
        } else {
            if threshold == 0 {
                inner.last_notification_threshold = 0;
            }
            false
        }
    };
    if should_notify {
        let body = if threshold == 100 {
            "New prompts in the Codex Mac app are paused until the five-hour reset.".to_string()
        } else {
            format!("You have used {threshold}% of the current five-hour allowance.")
        };
        let _ = app
            .notification()
            .builder()
            .title("QuotaBar")
            .body(body)
            .show();
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<MenuItem<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", "Open QuotaBar", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit QuotaBar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    TrayIconBuilder::with_id("main")
        .title("5h — · Week —")
        .tooltip("QuotaBar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => {
                let locked = app
                    .try_state::<AppRuntime>()
                    .and_then(|runtime| {
                        runtime.inner.try_read().ok().map(|inner| {
                            inner
                                .engine
                                .window
                                .as_ref()
                                .map(|window| {
                                    window.displayed_used_percent >= 100.0
                                        && window.override_ends_at.is_none()
                                })
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);
                if !locked {
                    app.exit(0);
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(quit)
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.move_window_constrained(Position::TrayCenter);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let (database_path, socket_path, sessions_root) =
                app_data_paths().map_err(std::io::Error::other)?;
            let store = Store::new(database_path).map_err(std::io::Error::other)?;
            let engine = store.load_engine().unwrap_or_default();
            let codex_home = dirs::home_dir()
                .ok_or_else(|| std::io::Error::other("Home directory unavailable"))?
                .join(".codex");
            let runtime = AppRuntime {
                inner: Arc::new(RwLock::new(RuntimeState {
                    engine,
                    sessions: HashMap::new(),
                    classification_healthy: false,
                    last_notification_threshold: 0,
                })),
                store,
                hook_manager: HookManager::new(codex_home),
                sessions_root,
                socket_path,
            };
            app.manage(runtime.clone());
            let quit_item = build_tray(app)?;
            if runtime
                .inner
                .blocking_read()
                .engine
                .settings
                .launch_at_login
            {
                let _ = app.handle().autolaunch().enable();
            }
            if let Some(window) = app.get_webview_window("main") {
                let close_window = window.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = close_window.hide();
                    }
                    WindowEvent::Focused(false) => {
                        let _ = close_window.hide();
                    }
                    _ => {}
                });
            }
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(serve_gate_socket(runtime.clone()));
            tauri::async_runtime::spawn(watch_session_logs(runtime.clone()));
            tauri::async_runtime::spawn(poll_app_server(runtime.clone()));
            tauri::async_runtime::spawn(refresh_ui(handle, runtime, quit_item));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            update_settings,
            request_override,
            install_desktop_hook,
            repair_desktop_hook,
            remove_desktop_hook,
            delete_history,
            hide_main_window,
            check_for_updates,
            install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running QuotaBar");
}
