use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::Manager;
use crate::utils::{find_bin, run_cli};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetupStatus {
    pub tailscale_installed: bool,
    pub kubectl_available: bool,
    pub ktctl_available: bool,
    pub daemon_running: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub headscale_url: String,
    pub namespace: String,
    pub shadow_node: String,
    pub shadow_image: String,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            headscale_url: "https://vpn.oneberry.cc:31443".to_string(),
            namespace: "oneberry-dev".to_string(),
            shadow_node: "hmdev-node01".to_string(),
            shadow_image: "image.hm.metavarse.tech:9443/hongmei-dev/kt-connect-shadow:v0.3.7".to_string(),
            theme: "system".to_string(),
        }
    }
}

#[tauri::command]
pub async fn check_setup() -> Result<SetupStatus, String> {
    // Tailscale — cross-platform CLI check
    let ts_bin = find_bin("tailscale");
    let tailscale = run_cli(&ts_bin, &["version"])
        .map(|o| o.status.success())
        .unwrap_or(false);

    // kubectl — 优先检测 sidecar，fallback 到系统路径
    let kubectl = Command::new(find_bin("kubectl"))
        .args(["version", "--client", "--short"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // ktctl — 优先检测 sidecar，fallback 到系统路径
    let ktctl = Command::new(find_bin("ktctl"))
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    Ok(SetupStatus {
        tailscale_installed: tailscale,
        kubectl_available: kubectl,
        ktctl_available: ktctl,
        daemon_running: false, // TODO: check daemon IPC
    })
}

#[tauri::command]
pub async fn install_tailscale(app: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        use std::path::PathBuf;

        // Try multiple candidate paths for the embedded pkg
        let mut candidates: Vec<PathBuf> = Vec::new();

        // 1. Tauri resource_dir (works in production bundle)
        if let Ok(rd) = app.path().resource_dir() {
            candidates.push(rd.join("resources").join("tailscale-darwin.pkg"));
        }

        // 2. Dev mode: relative to the workspace src-tauri dir
        let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("tailscale-darwin.pkg");
        candidates.push(dev_path);

        // Find the first candidate that exists
        let pkg_path = candidates.iter().find(|p| p.exists());

        if let Some(path) = pkg_path {
            // Silent install: use osascript to get admin privileges and run
            // the installer CLI. This shows only a single macOS password prompt
            // instead of the full Installer.app GUI.
            let script = format!(
                "do shell script \"installer -pkg '{}' -target /\" with administrator privileges",
                path.to_str().unwrap()
            );
            let output = Command::new("osascript")
                .args(["-e", &script])
                .output()
                .map_err(|e| format!("启动安装器失败: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // User cancelled the password prompt
                if stderr.contains("User canceled") || stderr.contains("-128") {
                    return Err("用户取消了安装".to_string());
                }
                return Err(format!("安装失败: {}", stderr));
            }

            // Post-install: wait for the Tailscale daemon (Network Extension) to initialize.
            // We do NOT quit or kill the Tailscale app — on macOS the Network Extension
            // and CLI depend on the app process, and killing it makes the daemon unreachable.
            // We also avoid touching System Events (Login Items) which triggers a confusing
            // macOS permissions dialog for the user.
            let mut daemon_ready = false;
            for i in 0..15 {
                let ts_bin = find_bin("tailscale");
                let check = run_cli(&ts_bin, &["status", "--json"]);
                if let Ok(o) = check {
                    if o.status.success() {
                        daemon_ready = true;
                        break;
                    }
                }
                if i < 14 {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }

            if daemon_ready {
                Ok("安装完成，守护进程已就绪".to_string())
            } else {
                // Daemon not ready yet — might need Network Extension approval
                Ok("安装完成，请在「系统设置 → 隐私与安全性」中允许 Tailscale 网络扩展".to_string())
            }
        } else {
            // Fallback: open the Tailscale download page in browser
            let download_url = crate::utils::tailscale_download_url();
            let _ = Command::new("open").arg(download_url).output();
            Err(format!(
                "未找到内嵌安装包，已为你打开下载页面: {}",
                download_url
            ))
        }
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, the official .exe installer is more reliable than embedded MSI.
        // Open the download page and let the user install the standard way.
        let download_url = crate::utils::tailscale_download_url();
        let _ = Command::new("cmd")
            .args(["/c", "start", download_url])
            .output();
        Ok(format!("OPEN_BROWSER:请从浏览器下载安装 Tailscale，安装完成后点击「重试」"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err(format!(
            "请手动安装 Tailscale: {}",
            crate::utils::tailscale_download_url()
        ))
    }
}

fn config_path() -> std::path::PathBuf {
    let home = dirs_next::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join(".oneberry").join("config.json")
}

#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    let path = config_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(AppConfig::default())
    }
}

#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<String, String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok("配置已保存".to_string())
}
