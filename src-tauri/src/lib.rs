mod commands;
mod services;
pub mod utils;

use commands::{vpn, cluster, session, setup};
use commands::session::SessionManager;
use services::health_monitor::HealthMonitor;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let session_manager = Arc::new(SessionManager::new());
    let cleanup_manager = session_manager.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(session_manager)
        .invoke_handler(tauri::generate_handler![
            vpn::check_vpn,
            vpn::connect_vpn,
            vpn::disconnect_vpn,
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
