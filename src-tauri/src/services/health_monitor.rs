use std::process::Command;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use crate::utils::find_bin;

pub struct HealthMonitor {
    app: AppHandle,
}

impl HealthMonitor {
    pub fn new(app: AppHandle) -> Self {
        HealthMonitor { app }
    }

    pub async fn run(&self) {
        loop {
            // Check VPN
            let vpn_ok = self.check_vpn().await;
            let _ = self.app.emit("health:vpn", vpn_ok);

            // Check cluster
            let cluster_ok = self.check_cluster().await;
            let _ = self.app.emit("health:cluster", cluster_ok);

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn check_vpn(&self) -> bool {
        Command::new(find_bin("tailscale"))
            .args(["status"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn check_cluster(&self) -> bool {
        Command::new(find_bin("kubectl"))
            .args(["cluster-info"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
