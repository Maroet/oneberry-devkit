use serde::{Deserialize, Serialize};
use std::process::Command;
use crate::utils::{find_bin, run_cli, spawn_cli, check_bin_in_path};
use super::dns;

/// Open a URL in the system default browser using OS-native commands.
/// This completely bypasses the Tauri opener plugin (which has glob scope
/// issues where `*` doesn't match `/` in URL paths).
#[tauri::command]
pub async fn open_auth_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // Windows: `cmd /c start` reliably opens URLs in default browser.
        // The empty "" before the URL is needed when the URL contains special chars.
        #[cfg(target_os = "windows")]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new("cmd")
            .args(["/c", "start", "", &url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VpnStatus {
    pub status: String,  // "connected", "disconnected", "not_installed", "needs_login"
    pub auth_url: Option<String>,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub debug_info: Option<String>,
}

#[tauri::command]
pub async fn check_vpn() -> Result<VpnStatus, String> {
    use std::path::PathBuf;

    let bin_path = find_bin("tailscale");
    let mut debug = format!("bin_path={}", bin_path);

    // First: check if the binary actually exists on disk.
    let binary_exists = if bin_path == "tailscale" {
        // Bare name fallback — check if it's in PATH
        let (exists, which_path) = check_bin_in_path("tailscale");
        debug.push_str(&format!(", which_exists={}, which_path={}", exists, which_path));
        exists
    } else {
        let exists = PathBuf::from(&bin_path).exists();
        debug.push_str(&format!(", path_exists={}", exists));
        exists
    };

    if !binary_exists {
        debug.push_str(", verdict=not_installed(binary_missing)");
        return Ok(VpnStatus {
            status: "not_installed".to_string(),
            auth_url: None,
            ip: None,
            hostname: None,
            debug_info: Some(debug),
        });
    }

    // Cross-platform CLI call (macOS wraps in sh, Windows calls directly)
    let output = run_cli(&bin_path, &["status", "--json"]);

    match output {
        Err(e) => {
            debug.push_str(&format!(", cmd_err={}, verdict=not_installed(exec_fail)", e));
            Ok(VpnStatus {
                status: "not_installed".to_string(),
                auth_url: None,
                ip: None,
                hostname: None,
                debug_info: Some(debug),
            })
        },
        Ok(o) if !o.status.success() => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let exit_code = o.status.code().unwrap_or(-1);
            debug.push_str(&format!(", exit={}, stderr={}", exit_code, stderr.trim()));

            // Exit 127 = "command not found" — the binary is a broken wrapper
            // script pointing to a non-existent Tailscale.app. Treat as not installed.
            let is_broken = exit_code == 127
                || stderr.contains("command not found")
                || (stderr.contains("No such file or directory")
                    && stderr.contains("Tailscale.app"));

            if is_broken {
                debug.push_str(", verdict=not_installed(broken_wrapper)");
                Ok(VpnStatus {
                    status: "not_installed".to_string(),
                    auth_url: None,
                    ip: None,
                    hostname: None,
                    debug_info: Some(debug),
                })
            } else {
                debug.push_str(", verdict=disconnected(cmd_failed)");
                Ok(VpnStatus {
                    status: "disconnected".to_string(),
                    auth_url: None,
                    ip: None,
                    hostname: None,
                    debug_info: Some(debug),
                })
            }
        },
        Ok(o) => {
            let stdout_str = String::from_utf8_lossy(&o.stdout);

            // Try to parse JSON
            let json: serde_json::Value = match serde_json::from_str(stdout_str.trim()) {
                Ok(v) => v,
                Err(e) => {
                    let preview = if stdout_str.len() > 200 {
                        format!("{}...", &stdout_str[..200])
                    } else {
                        stdout_str.trim().to_string()
                    };
                    debug.push_str(&format!(
                        ", parse_err={}, stdout_len={}, stdout_preview=[{}], verdict=disconnected(bad_json)",
                        e, stdout_str.len(), preview
                    ));
                    return Ok(VpnStatus {
                        status: "disconnected".to_string(),
                        auth_url: None,
                        ip: None,
                        hostname: None,
                        debug_info: Some(debug),
                    });
                }
            };

            let backend_state = json["BackendState"]
                .as_str()
                .unwrap_or("Unknown");

            let status = match backend_state {
                "Running" => "connected",
                "NeedsLogin" => "needs_login",
                "NeedsMachineAuth" => "needs_auth",
                "Stopped" => "disconnected",
                _ => "disconnected",
            };

            let ip = json["Self"]["TailscaleIPs"]
                .as_array()
                .and_then(|ips| ips.first())
                .and_then(|ip| ip.as_str())
                .map(String::from);

            let hostname = json["Self"]["HostName"]
                .as_str()
                .map(String::from);

            let auth_url = json["AuthURL"]
                .as_str()
                .map(String::from);

            debug.push_str(&format!(", backend={}, verdict={}", backend_state, status));

            Ok(VpnStatus {
                status: status.to_string(),
                auth_url,
                ip: if status == "connected" { ip } else { None },
                hostname: if status == "connected" { hostname } else { None },
                debug_info: Some(debug),
            })
        }
    }
}

#[tauri::command]
pub async fn connect_vpn(headscale_url: Option<String>) -> Result<String, String> {
    let url = headscale_url.unwrap_or_else(|| "https://vpn.oneberry.cc:31443".to_string());

    let bin = find_bin("tailscale");

    // Fire-and-forget: `tailscale up` blocks waiting for OIDC auth on macOS/Linux.
    // On Windows, it tells the Tailscale Service to initiate the connection.
    let _child = spawn_cli(&bin, &["up", "--login-server", &url, "--accept-routes", "--reset"])
        .map_err(|e| format!("启动 Tailscale 失败: {}", e))?;

    // Poll status a few times — on Windows the Service may take a moment to respond.
    for i in 0..3 {
        tokio::time::sleep(std::time::Duration::from_secs(if i == 0 { 3 } else { 2 })).await;

        let status = check_vpn().await.unwrap_or(VpnStatus {
            status: "disconnected".to_string(),
            auth_url: None,
            ip: None,
            hostname: None,
            debug_info: None,
        });

        if status.status == "connected" {
            // VPN connected — auto-configure DNS for .cluster.local resolution
            if let Err(e) = dns::ensure_cluster_dns() {
                eprintln!("[DevKit] DNS 配置失败（非致命）: {}", e);
            }
            return Ok("VPN 已连接".to_string());
        }

        // Auth URL found — return it immediately
        if let Some(ref auth) = status.auth_url {
            if !auth.is_empty() {
                return Ok(format!("AUTH_REQUIRED:{}", auth));
            }
        }

        // Backend says NeedsLogin but no URL — common on Windows.
        // Signal AUTH_REQUIRED with empty URL so frontend can guide user.
        if status.status == "needs_login" || status.status == "needs_auth" {
            return Ok(format!("AUTH_REQUIRED:{}", status.auth_url.unwrap_or_default()));
        }
    }

    // Fallback — connection in progress but no clear result yet
    Ok("VPN 连接请求已发送，正在等待认证...".to_string())
}

#[tauri::command]
pub async fn disconnect_vpn() -> Result<String, String> {
    // Clean up cluster DNS before disconnecting
    // (CoreDNS will be unreachable after VPN is down)
    if let Err(e) = dns::remove_cluster_dns() {
        eprintln!("[DevKit] DNS 清理失败（非致命）: {}", e);
    }

    // Use spawn_cli() — `tailscale down` can take 30s+ via output(),
    // but the actual disconnect happens instantly. Fire-and-forget.
    let bin = find_bin("tailscale");
    let _child = spawn_cli(&bin, &["down"])
        .map_err(|e| format!("断开失败: {}", e))?;
    Ok("VPN 已断开".to_string())
}
