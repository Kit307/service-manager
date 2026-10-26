use crate::{
    logs::LogPage,
    manager::Manager,
    mcp::AiBridge,
    model::*,
    project::{Project, ServiceRef, WorkspaceGroup},
    vpn::{VpnImport, VpnProfileView},
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{Emitter, Manager as _, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_liquid_glass::{GlassMaterialVariant, LiquidGlassConfig, LiquidGlassExt};

pub fn apply_window_glass(window: &tauri::WebviewWindow) {
    if let Err(error) = window.liquid_glass().set_effect(
        window,
        LiquidGlassConfig {
            corner_radius: 16.0,
            variant: GlassMaterialVariant::Clear,
            ..Default::default()
        },
    ) {
        eprintln!("Liquid Glass unavailable: {error}");
        if let Err(error) = window.set_effects(
            tauri::window::EffectsBuilder::new()
                .effect(tauri::window::Effect::UnderWindowBackground)
                .radius(16.0)
                .build(),
        ) {
            eprintln!("Window blur fallback unavailable: {error}");
        }
    }
}

pub struct DesktopState {
    pub manager: Manager,
    pub ai: AiBridge,
    vpn_profiles: tokio::sync::Mutex<Vec<VpnProfileView>>,
    exiting: Arc<AtomicBool>,
    prompting: Arc<AtomicBool>,
}
#[tauri::command]
async fn workspace(state: State<'_, DesktopState>) -> Result<Value> {
    let projects = state.manager.projects().await;
    let vpns = state.vpn_profiles.lock().await.clone();
    let store = state.manager.store.lock().await;
    let groups = store.private.groups.clone();
    let unclean = store.unclean;
    drop(store);
    Ok(
        json!({"projects":projects,"vpns":vpns,"groups":groups,"ai":state.ai.status().await,"uncleanShutdown":unclean}),
    )
}
async fn refresh_vpn_profiles(state: &DesktopState) -> Result<Vec<VpnProfileView>> {
    let profiles = match tokio::time::timeout(std::time::Duration::from_secs(10), state.manager.vpns.list()).await {
        Ok(result) => result?,
        Err(_) => {
            let mut cached = state.vpn_profiles.lock().await;
            for profile in cached.iter_mut() {
                profile.state = "unavailable".into();
                profile.error = Some("VPN status timed out. Check the local Docker engine and refresh.".into());
            }
            return Err(AppError::new("VPN_UNAVAILABLE", "VPN status timed out. Check the local Docker engine and refresh."));
        }
    };
    *state.vpn_profiles.lock().await = profiles.clone();
    Ok(profiles)
}
fn allow_vpn_change(state: &DesktopState) -> Result<()> {
    if state.exiting.load(Ordering::SeqCst) {
        return Err(AppError::new("OPERATION_IN_PROGRESS", "The application is shutting down."));
    }
    Ok(())
}
#[tauri::command]
async fn list_vpn_profiles(state: State<'_, DesktopState>) -> Result<Vec<VpnProfileView>> {
    refresh_vpn_profiles(&state).await
}
#[tauri::command]
async fn import_vpn_profile(state: State<'_, DesktopState>, input: VpnImport) -> Result<VpnProfileView> {
    allow_vpn_change(&state)?;
    let profile = state.manager.vpns.import(input).await?;
    let mut cached = state.vpn_profiles.lock().await;
    cached.retain(|entry| entry.id != profile.id);
    cached.push(profile.clone());
    Ok(profile)
}
#[tauri::command]
async fn connect_vpn(state: State<'_, DesktopState>, id: String) -> Result<VpnProfileView> {
    allow_vpn_change(&state)?;
    let profile = state.manager.vpns.connect(&id).await?;
    let mut cached = state.vpn_profiles.lock().await;
    cached.retain(|entry| entry.id != id);
    cached.push(profile.clone());
    Ok(profile)
}
#[tauri::command]
async fn disconnect_vpn(state: State<'_, DesktopState>, id: String) -> Result<()> {
    allow_vpn_change(&state)?;
    state.manager.vpns.disconnect(&id).await?;
    if let Some(profile) = state.vpn_profiles.lock().await.iter_mut().find(|profile| profile.id == id) {
        profile.state = "disconnected".into();
        profile.error = None;
        profile.users.clear();
    }
    Ok(())
}
#[tauri::command]
async fn remove_vpn_profile(state: State<'_, DesktopState>, id: String) -> Result<()> {
    allow_vpn_change(&state)?;
    state.manager.vpns.remove(&id).await?;
    state.vpn_profiles.lock().await.retain(|profile| profile.id != id);
    Ok(())
}
#[tauri::command]
async fn pull_container_image(state: State<'_, DesktopState>, image: String) -> Result<()> {
    allow_vpn_change(&state)?;
    crate::docker::pull_image(&image).await
}
#[tauri::command]
async fn save_group(
    state: State<'_, DesktopState>,
    id: Option<String>,
    name: String,
    services: Vec<ServiceRef>,
) -> Result<WorkspaceGroup> {
    state
        .manager
        .store
        .lock()
        .await
        .save_group(id.as_deref(), &name, services)
}
#[tauri::command]
async fn move_service(
    state: State<'_, DesktopState>,
    service: ServiceRef,
    group_id: Option<String>,
) -> Result<()> {
    state
        .manager
        .store
        .lock()
        .await
        .move_service(service, group_id.as_deref())
}
#[tauri::command]
async fn remove_group(state: State<'_, DesktopState>, id: String) -> Result<()> {
    state.manager.store.lock().await.remove_group(&id)
}
#[tauri::command]
async fn open_project(state: State<'_, DesktopState>, path: String) -> Result<Project> {
    state.manager.open(&path).await
}
#[tauri::command]
async fn remove_project(state: State<'_, DesktopState>, project_id: String) -> Result<()> {
    state.manager.remove_project(&project_id).await
}
#[tauri::command]
async fn save_service(
    state: State<'_, DesktopState>,
    project_id: String,
    definition: ServiceDefinition,
    expected_revision: String,
) -> Result<Project> {
    state
        .manager
        .save(&project_id, definition, &expected_revision, false)
        .await
}
#[tauri::command]
async fn trust_service(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
    expected_revision: String,
) -> Result<()> {
    state
        .manager
        .store
        .lock()
        .await
        .trust(&project_id, &service_id, &expected_revision)
}
#[tauri::command]
async fn control_service(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
    action: String,
) -> Result<()> {
    if state.exiting.load(Ordering::SeqCst) {
        return Err(AppError::new(
            "OPERATION_IN_PROGRESS",
            "The application is shutting down.",
        ));
    }
    match action.as_str() {
        "start" => {
            state
                .manager
                .start(&project_id, &service_id, "Desktop")
                .await?;
        }
        "stop" => {
            state.manager.stop(&project_id, &service_id).await?;
        }
        "restart" => {
            state
                .manager
                .restart(&project_id, &service_id, "Desktop")
                .await?;
        }
        _ => return Err(AppError::new("CONFIG_INVALID", "Unknown lifecycle action.")),
    }
    Ok(())
}
#[tauri::command]
async fn read_logs(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
    cursor: Option<String>,
) -> Result<LogPage> {
    state
        .manager
        .read_logs(&project_id, &service_id, cursor.as_deref())
        .await
}
#[tauri::command]
async fn clear_logs(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
) -> Result<()> {
    state.manager.status(&project_id, &service_id).await?;
    state
        .manager
        .logs
        .lock()
        .await
        .clear(&format!("{project_id}/{service_id}"));
    Ok(())
}
#[tauri::command]
async fn inspect_ports(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
) -> Result<Vec<crate::ports::PortListener>> {
    state.manager.inspect_ports(&project_id, &service_id).await
}
#[tauri::command]
async fn kill_port(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
    port: u16,
    pid: u32,
    identity: String,
) -> Result<()> {
    state
        .manager
        .kill_port(&project_id, &service_id, port, pid, &identity)
        .await
}
#[tauri::command]
async fn run_history(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
) -> Result<Vec<crate::history::RunSummary>> {
    state.manager.run_history(&project_id, &service_id).await
}
#[tauri::command]
async fn run_details(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
    run_id: String,
) -> Result<crate::history::RunRecord> {
    state
        .manager
        .run_details(&project_id, &service_id, &run_id)
        .await
}
#[tauri::command]
async fn diagnose_service(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
) -> Result<Value> {
    state.manager.diagnose(&project_id, &service_id).await
}
#[tauri::command]
async fn open_service_url(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
) -> Result<()> {
    let url = state.manager.checked_url(&project_id, &service_id).await?;
    let status = tokio::process::Command::new("/usr/bin/open")
        .arg(url)
        .status()
        .await?;
    if !status.success() {
        return Err(AppError::new(
            "OPEN_URL_FAILED",
            "The browser could not open the service URL.",
        ));
    }
    Ok(())
}
#[tauri::command]
async fn control_sequence(
    state: State<'_, DesktopState>,
    targets: Vec<ServiceRef>,
    action: String,
) -> Result<()> {
    if state.exiting.load(Ordering::SeqCst) {
        return Err(AppError::new(
            "OPERATION_IN_PROGRESS",
            "The application is shutting down.",
        ));
    }
    state
        .manager
        .control_sequence(targets, &action, "Desktop group")
        .await
}
#[tauri::command]
async fn cancel_sequence(state: State<'_, DesktopState>) -> Result<()> {
    state.manager.cancel_sequence();
    Ok(())
}
#[tauri::command]
async fn remove_service(
    state: State<'_, DesktopState>,
    project_id: String,
    service_id: String,
    expected_revision: String,
) -> Result<()> {
    state
        .manager
        .remove(&project_id, &service_id, &expected_revision)
        .await
}
#[tauri::command]
async fn set_ai(state: State<'_, DesktopState>, enabled: bool) -> Result<Value> {
    state.ai.set_enabled(enabled).await
}
#[tauri::command]
async fn create_client(
    state: State<'_, DesktopState>,
    name: String,
    projects: Vec<String>,
    control: bool,
    edit: bool,
    project_roots: Vec<String>,
) -> Result<crate::mcp::CreatedClient> {
    state
        .ai
        .create(
            name,
            projects,
            control,
            edit,
            project_roots,
            std::env::current_exe()?,
        )
        .await
}
#[tauri::command]
async fn revoke_client(state: State<'_, DesktopState>, id: String) -> Result<()> {
    state.ai.revoke(&id).await
}

pub fn run() {
    let app = tauri::Builder::default()
        .menu(|app| {
            let menu = tauri::menu::Menu::default(app)?;
            if let Some(tauri::menu::MenuItemKind::Submenu(application)) = menu.items()?.first() {
                // Native predefined Quit calls NSApp terminate directly, bypassing controlled cleanup.
                application.remove_at(application.items()?.len() - 1)?;
                application.append(&tauri::menu::MenuItem::with_id(app, "managed-quit", "Quit Service Manager", true, Some("CmdOrCtrl+Q"))?)?;
            }
            for item in menu.items()? {
                if let tauri::menu::MenuItemKind::Submenu(window) = item {
                    if window.id().as_ref() == tauri::menu::WINDOW_SUBMENU_ID {
                        // The native Minimize menu action can remain enabled after the titlebar button is removed.
                        window.remove_at(0)?;
                        window.insert(&tauri::menu::MenuItem::with_id(app, "disabled-minimize", "Minimize", false, Some("CmdOrCtrl+M"))?, 0)?;
                    }
                }
            }
            Ok(menu)
        })
        .on_menu_event(|app, event| { if event.id.as_ref() == "managed-quit" { app.exit(0); } })
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_liquid_glass::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                apply_window_glass(&window);
            }
            let dir = std::env::var_os("SERVICE_MANAGER_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?);
            let manager = Manager::new(dir.clone())?;
            let ai = AiBridge::new(manager.clone(), dir, 47831)?;
            app.manage(DesktopState {
                manager,
                ai,
                vpn_profiles: tokio::sync::Mutex::new(Vec::new()),
                exiting: Arc::new(AtomicBool::new(false)),
                prompting: Arc::new(AtomicBool::new(false)),
            });
            let vpn_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let state = vpn_handle.state::<DesktopState>();
                    if state.exiting.load(Ordering::SeqCst) { break; }
                    // Docker inspection must never delay workspace or log publication.
                    let _ = refresh_vpn_profiles(&state).await;
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut previous = String::new();
                let mut log_revision = 0;
                let mut ticks = 0u32;
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    let state = handle.state::<DesktopState>();
                    if ticks.is_multiple_of(20) {
                        state.manager.projects().await;
                    }
                    ticks = ticks.wrapping_add(1);
                    let store = state.manager.store.lock().await;
                    let mut projects: Vec<_> = store.projects.values().cloned().collect();
                    projects.sort_by(|a, b| a.name.cmp(&b.name));
                    let unclean = store.unclean;
                    let groups = store.private.groups.clone();
                    drop(store);
                    let vpns = state.vpn_profiles.lock().await.clone();
                    let data = json!({"vpns": vpns, "projects": projects, "groups": groups, "ai": state.ai.status().await, "uncleanShutdown": unclean});
                    let revision = crate::project::hash(&serde_json::to_vec(&data).unwrap_or_default());
                    if revision != previous { let _ = handle.emit("workspace-changed", data); previous = revision; }
                    let seq = state.manager.logs.lock().await.revision();
                    if seq != log_revision { let _ = handle.emit("logs-changed", seq); log_revision = seq; }
                }
            });
            let open = tauri::menu::MenuItem::with_id(
                app,
                "show",
                "Open Service Manager",
                true,
                None::<&str>,
            )?;
            let quit = tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&open, &quit])?;
            let mut tray = tauri::tray::TrayIconBuilder::new()
                .tooltip("Service Manager")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "show" {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            workspace,
            list_vpn_profiles,
            import_vpn_profile,
            connect_vpn,
            disconnect_vpn,
            remove_vpn_profile,
            pull_container_image,
            save_group,
            move_service,
            remove_group,
            open_project,
            remove_project,
            save_service,
            trust_service,
            control_service,
            read_logs,
            clear_logs,
            remove_service,
            set_ai,
            create_client,
            revoke_client
            ,inspect_ports,kill_port,run_history,run_details,diagnose_service,open_service_url,control_sequence,cancel_sequence
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("Unable to initialize Service Manager");
    app.run(|handle, event| {
        match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                let state = handle.state::<DesktopState>();
                if state.exiting.load(Ordering::SeqCst) { return; }
                api.prevent_exit();
                if state.prompting.swap(true, Ordering::SeqCst) { return; }
                let manager = state.manager.clone(); let bridge = state.ai.clone(); let exiting = state.exiting.clone(); let prompting = state.prompting.clone(); let handle = handle.clone();
                let mut dialog = handle.dialog().message("Stop active services and quit? Closing the window only hides it; services keep running until you stop them or quit.").title("Quit Service Manager").buttons(MessageDialogButtons::OkCancelCustom("Stop services & quit".into(), "Cancel".into()));
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.show(); let _ = window.set_focus();
                    dialog = dialog.parent(&window);
                }
                dialog.show(move |confirmed| {
                    if !confirmed { prompting.store(false, Ordering::SeqCst); return; }
                    tauri::async_runtime::spawn(async move {
                        let _ = bridge.set_enabled(false).await;
                        match manager.shutdown().await {
                            Ok(()) => { exiting.store(true, Ordering::SeqCst); handle.exit(0); },
                            Err(error) => { prompting.store(false, Ordering::SeqCst); handle.dialog().message(error.message).title("Could not stop all services").show(|_| {}); }
                        }
                    });
                });
            },
            tauri::RunEvent::Reopen { .. } => { if let Some(w) = handle.get_webview_window("main") { let _ = w.show(); let _ = w.set_focus(); } },
            _ => {}
        }
    });
}
