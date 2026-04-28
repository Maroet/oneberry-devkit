use std::process::Command;

/// K8s CoreDNS ClusterIP — used to resolve *.svc.cluster.local domains
const KUBE_DNS_IP: &str = "10.96.0.10";
const DNS_DOMAIN: &str = ".cluster.local";

/// Ensure the system can resolve K8s cluster DNS names (*.svc.cluster.local).
///
/// - **Windows**: Adds an NRPT (Name Resolution Policy Table) rule that forwards
///   `.cluster.local` queries to the K8s CoreDNS.
/// - **macOS**: Creates `/etc/resolver/cluster.local` pointing to CoreDNS.
/// - **Linux**: no-op (users manage DNS manually or via systemd-resolved).
///
/// This is idempotent — safe to call multiple times.
pub fn ensure_cluster_dns() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        ensure_dns_windows()
    }

    #[cfg(target_os = "macos")]
    {
        ensure_dns_macos()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Linux: no-op, users handle DNS via systemd-resolved or /etc/resolv.conf
        Ok(())
    }
}

/// Remove the cluster DNS resolver configuration.
///
/// Called when VPN is disconnected to avoid DNS resolution failures
/// when the CoreDNS is unreachable.
pub fn remove_cluster_dns() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        remove_dns_windows()
    }

    #[cfg(target_os = "macos")]
    {
        remove_dns_macos()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(())
    }
}

/// Tauri command: manually trigger DNS setup (for UI "fix DNS" button if needed)
#[tauri::command]
pub async fn setup_cluster_dns() -> Result<String, String> {
    ensure_cluster_dns()?;
    Ok("集群 DNS 已配置".to_string())
}

/// Tauri command: check if cluster DNS is configured
#[tauri::command]
pub async fn check_cluster_dns() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        check_dns_windows()
    }

    #[cfg(target_os = "macos")]
    {
        check_dns_macos()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(false)
    }
}

// ─── Windows implementation ──────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn ensure_dns_windows() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // Check if rule already exists
    if check_dns_windows().unwrap_or(false) {
        return Ok(());
    }

    // Add NRPT rule: forward .cluster.local queries to K8s CoreDNS
    let ps_script = format!(
        "Add-DnsClientNrptRule -Namespace '{}' -NameServers '{}'",
        DNS_DOMAIN, KUBE_DNS_IP
    );

    // Try direct execution first (works if already running as admin)
    let direct = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(ref o) = direct {
        if o.status.success() {
            eprintln!("[DevKit] Windows NRPT rule added for {}", DNS_DOMAIN);
            return Ok(());
        }
    }

    // Fallback: elevate with UAC (will prompt user)
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-Command",
            &format!(
                "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass -Command \"{}\"' -Wait",
                ps_script
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行 PowerShell 失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("canceled") || stderr.contains("拒绝") {
            return Err("用户取消了管理员权限请求".to_string());
        }
        return Err(format!("配置 DNS 失败: {}", stderr));
    }

    eprintln!("[DevKit] Windows NRPT rule added for {} (elevated)", DNS_DOMAIN);
    Ok(())
}

#[cfg(target_os = "windows")]
fn remove_dns_windows() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // Don't bother if no rule exists
    if !check_dns_windows().unwrap_or(true) {
        return Ok(());
    }

    let ps_script = format!(
        "Get-DnsClientNrptRule | Where-Object {{ $_.Namespace -eq '{}' }} | Remove-DnsClientNrptRule -Force",
        DNS_DOMAIN
    );

    // Try direct first
    let direct = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(ref o) = direct {
        if o.status.success() {
            eprintln!("[DevKit] Windows NRPT rule removed for {}", DNS_DOMAIN);
            return Ok(());
        }
    }

    // Fallback: elevate (best effort, don't fail disconnect)
    let _output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-Command",
            &format!(
                "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass -Command \"{}\"' -Wait",
                ps_script
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok();

    eprintln!("[DevKit] Windows NRPT rule removed for {}", DNS_DOMAIN);
    Ok(())
}

#[cfg(target_os = "windows")]
fn check_dns_windows() -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let ps_script = format!(
        "(Get-DnsClientNrptRule | Where-Object {{ $_.Namespace -eq '{}' }}).Count",
        DNS_DOMAIN
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("检查 DNS 配置失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let count: i32 = stdout.trim().parse().unwrap_or(0);
    Ok(count > 0)
}

// ─── macOS implementation ────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn ensure_dns_macos() -> Result<(), String> {
    use std::path::Path;

    let resolver_dir = Path::new("/etc/resolver");
    let resolver_file = resolver_dir.join("cluster.local");

    // Check if already configured
    if resolver_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&resolver_file) {
            if content.contains(KUBE_DNS_IP) {
                return Ok(()); // Already correct
            }
        }
    }

    let content = format!("nameserver {}\n", KUBE_DNS_IP);

    // Create /etc/resolver/ and write the file (requires sudo)
    let script = format!(
        "do shell script \"mkdir -p /etc/resolver && echo '{}' > /etc/resolver/cluster.local\" with administrator privileges",
        content.trim()
    );

    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| format!("执行 osascript 失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("-128") {
            return Err("用户取消了管理员权限请求".to_string());
        }
        return Err(format!("配置 DNS 失败: {}", stderr));
    }

    eprintln!("[DevKit] macOS resolver created for cluster.local → {}", KUBE_DNS_IP);
    Ok(())
}

#[cfg(target_os = "macos")]
fn remove_dns_macos() -> Result<(), String> {
    use std::path::Path;

    let resolver_file = Path::new("/etc/resolver/cluster.local");
    if !resolver_file.exists() {
        return Ok(());
    }

    let script = "do shell script \"rm -f /etc/resolver/cluster.local\" with administrator privileges";
    let _output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok(); // Best effort

    eprintln!("[DevKit] macOS resolver removed for cluster.local");
    Ok(())
}

#[cfg(target_os = "macos")]
fn check_dns_macos() -> Result<bool, String> {
    use std::path::Path;

    let resolver_file = Path::new("/etc/resolver/cluster.local");
    if !resolver_file.exists() {
        return Ok(false);
    }
    let content = std::fs::read_to_string(resolver_file)
        .map_err(|e| e.to_string())?;
    Ok(content.contains(KUBE_DNS_IP))
}
