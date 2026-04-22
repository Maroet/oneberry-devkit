use std::time::Duration;
use tauri::{AppHandle, Emitter};
use crate::utils::{find_bin, run_cli};

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
        let bin = find_bin("tailscale");
        run_cli(&bin, &["status"])
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn check_cluster(&self) -> bool {
        let bin = find_bin("kubectl");
        run_cli(&bin, &["cluster-info"])
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
