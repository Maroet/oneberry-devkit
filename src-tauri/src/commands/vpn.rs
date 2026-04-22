use serde::{Deserialize, Serialize};
use std::process::Command;
use crate::utils::find_bin;

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
        // Bare name fallback — check if it's in PATH by trying `which`
        let which_result = Command::new("which")
            .arg("tailscale")
            .output();
        let exists = which_result.as_ref().map(|o| o.status.success()).unwrap_or(false);
        let which_path = which_result.ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
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

    // Run via /bin/sh -c to break macOS .app bundle process association.
    // When called directly from a .app, the CLI can't communicate with
    // the Tailscale daemon (XPC restriction). sh creates an independent context.
    let output = Command::new("/bin/sh")
        .args(["-c", &format!("'{}' status --json", bin_path)])
        .output();

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

            // Try to parse JSON. The standalone CLI (lowercase `tailscale`) talks
            // directly to the System Extension daemon — no GUI needed.

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

    // Use spawn() instead of output() because `tailscale up` blocks
    // waiting for the user to complete OIDC authentication in the browser.
    // Run via sh -c to break .app bundle XPC restriction.
    let bin = find_bin("tailscale");
    let _child = Command::new("/bin/sh")
        .args(["-c", &format!("'{}' up --login-server '{}' --accept-routes --reset", bin, url)])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 Tailscale 失败: {}", e))?;

    // Wait a moment for tailscale to register the login request
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Check status to see if we already got an auth URL
    let status = check_vpn().await.unwrap_or(VpnStatus {
        status: "disconnected".to_string(),
        auth_url: None,
        ip: None,
        hostname: None,
        debug_info: None,
    });

    if let Some(ref auth) = status.auth_url {
        Ok(format!("AUTH_REQUIRED:{}", auth))
    } else if status.status == "connected" {
        Ok("VPN 已连接".to_string())
    } else {
        Ok("VPN 连接请求已发送，正在等待认证...".to_string())
    }
}

#[tauri::command]
pub async fn disconnect_vpn() -> Result<String, String> {
    // Use spawn() — `tailscale down` can take 30s+ to return via .output(),
    // but the actual disconnect happens instantly. Fire-and-forget.
    // Run via sh -c to break .app bundle XPC restriction.
    let bin = find_bin("tailscale");
    let _child = Command::new("/bin/sh")
        .args(["-c", &format!("'{}' down", bin)])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("断开失败: {}", e))?;
    Ok("VPN 已断开".to_string())
}
