mod commands;
mod services;
pub mod utils;

use commands::{vpn, cluster, session, setup};
use commands::session::SessionManager;
use services::health_monitor::HealthMonitor;
use std::sync::Arc;

/// Auto-deploy bundled kubeconfig to ~/.kube/config if not present.
/// This ensures kubectl works out-of-the-box for new installations.
fn deploy_kubeconfig(app: &tauri::AppHandle) {
    use tauri::Manager;

    let home = dirs_next::home_dir().unwrap_or_default();
    let kube_dir = home.join(".kube");
    let kube_config = kube_dir.join("config");

    // Don't overwrite existing config
    if kube_config.exists() {
        return;
    }

    // Try to find bundled kubeconfig
    let mut candidates = Vec::new();

    // 1. Production: resource_dir
    if let Ok(rd) = app.path().resource_dir() {
        candidates.push(rd.join("resources").join("kubeconfig"));
    }

    // 2. Dev mode: CARGO_MANIFEST_DIR
    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("kubeconfig");
    candidates.push(dev_path);

    if let Some(src) = candidates.iter().find(|p| p.exists()) {
        // Create ~/.kube/ directory
        let _ = std::fs::create_dir_all(&kube_dir);
        match std::fs::copy(src, &kube_config) {
            Ok(_) => eprintln!("[DevKit] Deployed kubeconfig to {}", kube_config.display()),
            Err(e) => eprintln!("[DevKit] Failed to deploy kubeconfig: {}", e),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let session_manager = Arc::new(SessionManager::new());
    let cleanup_manager = session_manager.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(session_manager)
        .invoke_handler(tauri::generate_handler![
            vpn::check_vpn,
            vpn::connect_vpn,
            vpn::disconnect_vpn,
            vpn::open_auth_url,
            cluster::check_cluster,
            cluster::list_services,
            session::start_exchange,
            session::start_mesh,
            session::stop_session,
            session::list_sessions,
            session::recover_service,
            setup::check_setup,
            setup::install_tailscale,
            setup::get_config,
            setup::save_config,
        ])
        .setup(|app| {
            // Auto-deploy kubeconfig if not present
            deploy_kubeconfig(app.handle());

            // Build tray icon
            let _tray = tauri::tray::TrayIconBuilder::new()
                .tooltip("OneBerry DevKit")
                .menu(&tauri::menu::Menu::new(app.handle())?)
                .on_menu_event(|_app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            // TODO: show main window
                        }
                        "quit" => {
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // Start health monitor in background
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let monitor = HealthMonitor::new(handle);
                monitor.run().await;
            });

            Ok(())
        })
        .on_window_event(move |_window, event| {
            // Clean up all sessions when the app is about to close
            if let tauri::WindowEvent::Destroyed = event {
                cleanup_manager.kill_all();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
