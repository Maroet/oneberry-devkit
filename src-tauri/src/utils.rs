use std::path::PathBuf;

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
