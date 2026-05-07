use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use crate::commands::{vpn, cluster};

/// Shared flag to pause/resume health polling (e.g. when window is hidden).
pub struct MonitorActive(pub Arc<AtomicBool>);

pub struct HealthMonitor {
    app: AppHandle,
    active: Arc<AtomicBool>,
}

impl HealthMonitor {
    pub fn new(app: AppHandle, active: Arc<AtomicBool>) -> Self {
        HealthMonitor { app, active }
    }

    pub async fn run(&self) {
        loop {
            if self.active.load(Ordering::Relaxed) {
                // Reuse the same command logic — emits full status structs
                if let Ok(status) = vpn::check_vpn().await {
                    let _ = self.app.emit("health:vpn", &status);
                }

                if let Ok(status) = cluster::check_cluster().await {
                    let _ = self.app.emit("health:cluster", &status);
                }
            }

            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
}

#[tauri::command]
pub fn set_monitor_active(active: bool, state: tauri::State<MonitorActive>) {
    state.0.store(active, Ordering::Relaxed);
}
