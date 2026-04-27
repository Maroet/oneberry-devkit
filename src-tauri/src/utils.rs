use std::path::PathBuf;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// 解析系统工具二进制路径
///
/// 优先级：
/// 1. Tauri Sidecar (内嵌在应用内的二进制) — kubectl / ktctl
/// 2. 系统常见安装路径 (macOS / Windows)
/// 3. Fallback 到 PATH
pub fn find_bin(name: &str) -> String {
    // 首先检查系统常见安装路径
    let system_paths = get_system_paths(name);
    for path in system_paths {
        if PathBuf::from(&path).exists() {
            return path;
        }
    }

    // Fallback: 假设在 PATH 中
    name.to_string()
}

/// 获取各平台下工具的常见安装路径
fn get_system_paths(name: &str) -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        match name {
            "tailscale" => vec![
                // IMPORTANT: lowercase 'tailscale' is the standalone CLI (talks to daemon socket directly)
                // uppercase 'Tailscale' is the GUI app binary (requires GUI running, fails from .app bundles)
                "/Applications/Tailscale.app/Contents/MacOS/tailscale".to_string(),
                "/opt/homebrew/bin/tailscale".to_string(),
                "/usr/local/bin/tailscale".to_string(),
            ],
            "kubectl" => vec![
                "/opt/homebrew/bin/kubectl".to_string(),
                "/usr/local/bin/kubectl".to_string(),
                "/usr/bin/kubectl".to_string(),
            ],
            "ktctl" => vec![
                "/opt/homebrew/bin/ktctl".to_string(),
                "/usr/local/bin/ktctl".to_string(),
            ],
            _ => vec![],
        }
    }

    #[cfg(target_os = "windows")]
    {
        let program_files = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());

        match name {
            "tailscale" => vec![
                format!("{}\\Tailscale\\tailscale.exe", program_files),
                format!("{}\\Tailscale IPN\\tailscale.exe", program_files),
                // User-specific install locations
                format!("{}\\Tailscale\\tailscale.exe", local_app_data),
                format!("{}\\Tailscale IPN\\tailscale.exe", local_app_data),
                // Fallback: 64-bit program files on 32-bit process
                "C:\\Program Files\\Tailscale\\tailscale.exe".to_string(),
            ],
            "kubectl" => vec![
                format!("{}\\kubectl\\kubectl.exe", program_files),
                format!("{}\\kubectl.exe", local_app_data),
                "C:\\kubectl\\kubectl.exe".to_string(),
            ],
            "ktctl" => vec![
                format!("{}\\ktctl\\ktctl.exe", program_files),
                format!("{}\\ktctl.exe", local_app_data),
            ],
            _ => vec![],
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        match name {
            "tailscale" => vec![
                "/usr/bin/tailscale".to_string(),
                "/usr/local/bin/tailscale".to_string(),
            ],
            "kubectl" => vec![
                "/usr/local/bin/kubectl".to_string(),
                "/usr/bin/kubectl".to_string(),
            ],
            "ktctl" => vec![
                "/usr/local/bin/ktctl".to_string(),
            ],
            _ => vec![],
        }
    }
}

/// 获取 Tailscale 安装包的平台特定文件名
pub fn tailscale_installer_name() -> &'static str {
    #[cfg(target_os = "macos")]
    { "tailscale-darwin.pkg" }
    #[cfg(target_os = "windows")]
    { "tailscale-windows.msi" }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    { "" }
}

/// 获取 Tailscale 的下载 URL
pub fn tailscale_download_url() -> &'static str {
    #[cfg(target_os = "macos")]
    { "https://pkgs.tailscale.com/stable/#macos" }
    #[cfg(target_os = "windows")]
    { "https://pkgs.tailscale.com/stable/#windows" }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    { "https://tailscale.com/download" }
}

// ─── Cross-platform CLI helpers ───────────────────────────────────────

use std::process::{Command, Output, Stdio};

/// Run a CLI command (blocking) with platform-specific wrapping.
///
/// macOS: wraps in `/bin/sh -c '...'` to bypass .app bundle XPC restrictions.
/// Windows/Linux: runs the binary directly — no wrapping needed.
pub fn run_cli(bin: &str, args: &[&str]) -> std::io::Result<Output> {
    #[cfg(target_os = "macos")]
    {
        let full_cmd = std::iter::once(format!("'{}'", bin))
            .chain(args.iter().map(|a| format!("'{}'", a)))
            .collect::<Vec<_>>()
            .join(" ");
        Command::new("/bin/sh")
            .args(["-c", &full_cmd])
            .output()
    }

    #[cfg(target_os = "windows")]
    {
        // CREATE_NO_WINDOW (0x08000000) prevents terminal window flashing
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(bin)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Command::new(bin)
            .args(args)
            .output()
    }
}

/// Spawn a CLI command (non-blocking, fire-and-forget) with platform-specific wrapping.
///
/// Same platform logic as `run_cli`, but uses `spawn()` instead of `output()`.
pub fn spawn_cli(bin: &str, args: &[&str]) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "macos")]
    {
        let full_cmd = std::iter::once(format!("'{}'", bin))
            .chain(args.iter().map(|a| format!("'{}'", a)))
            .collect::<Vec<_>>()
            .join(" ");
        Command::new("/bin/sh")
            .args(["-c", &full_cmd])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(bin)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Command::new(bin)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }
}

/// Check if a bare binary name exists in PATH.
///
/// macOS/Linux: uses `which`. Windows: uses `where`.
pub fn check_bin_in_path(name: &str) -> (bool, String) {
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let result = Command::new("where")
            .arg(name)
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        let exists = result.as_ref().map(|o| o.status.success()).unwrap_or(false);
        let path = result.ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        (exists, path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let result = Command::new("which").arg(name).output();
        let exists = result.as_ref().map(|o| o.status.success()).unwrap_or(false);
        let path = result.ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        (exists, path)
    }
}

/// Check if a process is alive by PID (cross-platform).
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout);
                o.status.success() && stdout.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("ps")
            .args(["-p", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
