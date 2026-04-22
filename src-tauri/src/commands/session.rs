use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::BufReader;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};
use crate::utils::find_bin;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub service: String,
    pub port: u16,
    pub mode: String, // "exchange" or "mesh"
    pub started_at: String,
    pub version_header: Option<String>,
    pub status: String, // "starting", "running", "stopped", "error"
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionLogLine {
    pub session_id: String,
    pub stream: String, // "stdout" or "stderr"
    pub line: String,
    pub timestamp: String,
}

/// Manages all active ktctl sessions
pub struct SessionManager {
    sessions: Mutex<HashMap<String, ManagedSession>>,
}

struct ManagedSession {
    info: SessionInfo,
    child: Option<Child>,
    pid: Option<u32>,  // For macOS osascript-spawned processes
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.lock().unwrap();
        sessions.values().map(|s| s.info.clone()).collect()
    }

    #[allow(dead_code)]
    pub fn update_status(&self, id: &str, status: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(id) {
            session.info.status = status.to_string();
        }
    }

    pub fn remove(&self, id: &str) -> Option<SessionInfo> {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.remove(id).map(|mut s| {
            // Kill child process if still running
            if let Some(ref mut child) = s.child {
                let _ = child.kill();
                let _ = child.wait();
            }
            // Kill by PID (macOS osascript-spawned root processes)
            if let Some(pid) = s.pid {
                kill_root_process(pid);
            }
            s.info
        })
    }

    pub fn kill_all(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        for (_, session) in sessions.iter_mut() {
            if let Some(ref mut child) = session.child {
                let _ = child.kill();
                let _ = child.wait();
            }
            if let Some(pid) = session.pid {
                kill_root_process(pid);
            }
            session.info.status = "stopped".to_string();
        }
        sessions.clear();
    }
}

/// Kill a root-owned process. On macOS, uses osascript for privilege elevation.
fn kill_root_process(pid: u32) {
    #[cfg(target_os = "macos")]
    {
        // Try osascript first (may use cached credentials, no dialog)
        let script = format!(
            "do shell script \"kill {}\" with administrator privileges",
            pid
        );
        let _ = Command::new("osascript")
            .args(["-e", &script])
            .output();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = Command::new("kill")
            .arg(pid.to_string())
            .output();
    }
}

/// Build the ktctl command with proper privilege elevation per platform
/// Reserved for future use with osascript-based GUI sudo prompts
#[allow(dead_code)]
fn build_ktctl_command(args: &[&str]) -> Command {
    let ktctl_bin = find_bin("ktctl");

    #[cfg(target_os = "macos")]
    {
        // Use osascript for GUI-based sudo prompt on macOS
        let ktctl_args = std::iter::once(ktctl_bin.as_str())
            .chain(args.iter().copied())
            .collect::<Vec<&str>>()
            .join(" ");

        let mut cmd = Command::new("osascript");
        cmd.args([
            "-e",
            &format!(
                "do shell script \"{}\" with administrator privileges",
                ktctl_args.replace('"', "\\\"")
            ),
        ]);
        cmd
    }

    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new(&ktctl_bin);
        cmd.args(args);
        cmd
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let mut cmd = Command::new("sudo");
        cmd.arg(&ktctl_bin).args(args);
        cmd
    }
}

/// Spawn a session and stream its logs to the frontend via Tauri events.
///
/// On macOS, `ktctl` needs root for network manipulation. Since this is a GUI app
/// with no terminal, we use `osascript` to show the native macOS password dialog,
/// then background the process and tail its log file for streaming output.
fn spawn_session(
    app: &AppHandle,
    manager: &SessionManager,
    info: SessionInfo,
    args: Vec<String>,
) -> Result<SessionInfo, String> {
    let ktctl_bin = find_bin("ktctl");
    let session_id = info.id.clone();

    #[cfg(target_os = "macos")]
    {
        use std::io::BufRead as _;

        // Create log directory
        let log_dir = std::env::temp_dir().join("oneberry-devkit");
        std::fs::create_dir_all(&log_dir)
            .map_err(|e| format!("无法创建日志目录: {}", e))?;
        let log_path = log_dir.join(format!("{}.log", session_id));
        let pid_path = log_dir.join(format!("{}.pid", session_id));
        let script_path = log_dir.join(format!("{}.sh", session_id));

        // Write a helper script — eliminates all osascript escaping issues
        let args_str = args.iter()
            .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join(" ");
        let script_content = format!(
            "#!/bin/bash\n'{}' {} > '{}' 2>&1 &\necho $! > '{}'\n",
            ktctl_bin.replace('\'', "'\\''"),
            args_str,
            log_path.display(),
            pid_path.display(),
        );
        std::fs::write(&script_path, &script_content)
            .map_err(|e| format!("无法写入启动脚本: {}", e))?;

        // Use osascript to run the script with admin privileges (shows macOS password dialog)
        let osascript_cmd = format!(
            "do shell script \"/bin/bash '{}'\" with administrator privileges",
            script_path.display()
        );
        let output = Command::new("osascript")
            .args(["-e", &osascript_cmd])
            .output()
            .map_err(|e| format!("需要管理员权限: {}", e))?;

        // Clean up script file
        let _ = std::fs::remove_file(&script_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("User canceled") || stderr.contains("-128") {
                return Err("用户取消了授权".to_string());
            }
            return Err(format!("启动 ktctl 失败: {}", stderr));
        }

        // Read PID from pid file
        let pid_str = std::fs::read_to_string(&pid_path)
            .map_err(|e| format!("无法读取进程 PID: {}", e))?;
        let pid: u32 = pid_str.trim().parse()
            .map_err(|_| format!("无效的 PID: {}", pid_str.trim()))?;

        // Start log file tailing thread
        let app_clone = app.clone();
        let sid = session_id.clone();
        let log_path_clone = log_path.clone();
        thread::spawn(move || {
            // Wait for log file to appear
            for _ in 0..50 {
                if log_path_clone.exists() { break; }
                thread::sleep(std::time::Duration::from_millis(100));
            }

            let file = match std::fs::File::open(&log_path_clone) {
                Ok(f) => f,
                Err(_) => return,
            };
            let mut reader = BufReader::new(file);
            let mut line_buf = String::new();

            loop {
                line_buf.clear();
                match reader.read_line(&mut line_buf) {
                    Ok(0) => {
                        // EOF — check if process is still alive.
                        // NOTE: can't use `kill -0` because ktctl runs as root
                        // and the DevKit runs as a normal user — kill would get EPERM.
                        // Use `ps -p` instead, which works regardless of user.
                        let alive = Command::new("ps")
                            .args(["-p", &pid.to_string()])
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or(false);
                        if !alive { break; }
                        thread::sleep(std::time::Duration::from_millis(300));
                    }
                    Ok(_) => {
                        let log = SessionLogLine {
                            session_id: sid.clone(),
                            stream: "stdout".to_string(),
                            line: line_buf.trim_end().to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        let _ = app_clone.emit("session:log", &log);
                    }
                    Err(_) => break,
                }
            }
            // Process ended
            let _ = app_clone.emit("session:ended", &sid);
            // Cleanup log files
            let _ = std::fs::remove_file(&log_path_clone);
            let _ = std::fs::remove_file(log_path_clone.with_extension("pid"));
        });

        // Store in manager (no Child handle — we track by PID)
        let mut sessions = manager.sessions.lock().unwrap();
        let mut stored_info = info.clone();
        stored_info.status = "running".to_string();
        sessions.insert(session_id, ManagedSession {
            info: stored_info.clone(),
            child: None,
            pid: Some(pid),
        });

        return Ok(stored_info);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mut cmd;

        #[cfg(target_os = "windows")]
        {
            cmd = Command::new(&ktctl_bin);
        }

        #[cfg(not(target_os = "windows"))]
        {
            cmd = Command::new("sudo");
            cmd.arg(&ktctl_bin);
        }

        let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        cmd.args(&str_args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| format!("启动 ktctl 失败: {}", e))?;

        // Capture stdout
        let app_clone = app.clone();
        let sid = session_id.clone();
        if let Some(stdout) = child.stdout.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let log = SessionLogLine {
                            session_id: sid.clone(),
                            stream: "stdout".to_string(),
                            line,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        let _ = app_clone.emit("session:log", &log);
                    }
                }
                let _ = app_clone.emit("session:ended", &sid);
            });
        }

        // Capture stderr
        let app_clone2 = app.clone();
        let sid2 = session_id.clone();
        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let log = SessionLogLine {
                            session_id: sid2.clone(),
                            stream: "stderr".to_string(),
                            line,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        let _ = app_clone2.emit("session:log", &log);
                    }
                }
            });
        }

        // Store in manager
        let mut sessions = manager.sessions.lock().unwrap();
        let mut stored_info = info.clone();
        stored_info.status = "running".to_string();
        sessions.insert(session_id, ManagedSession {
            info: stored_info.clone(),
            child: Some(child),
            pid: None,
        });

        Ok(stored_info)
    }
}

#[tauri::command]
pub async fn start_exchange(
    app: AppHandle,
    manager: tauri::State<'_, Arc<SessionManager>>,
    service: String,
    port: u16,
    namespace: Option<String>,
) -> Result<SessionInfo, String> {
    let ns = namespace.unwrap_or_else(|| "oneberry-dev".to_string());

    // Check if local port is listening
    let check = std::net::TcpStream::connect(format!("127.0.0.1:{}", port));
    if check.is_err() {
        return Err(format!("本地端口 {} 无服务监听，请先启动本地服务", port));
    }

    let id = format!("exchange-{}-{}", service, chrono::Utc::now().timestamp());
    let info = SessionInfo {
        id: id.clone(),
        service: service.clone(),
        port,
        mode: "exchange".to_string(),
        started_at: chrono::Utc::now().to_rfc3339(),
        version_header: None,
        status: "starting".to_string(),
    };

    let args = vec![
        "exchange".to_string(),
        service,
        "--expose".to_string(),
        port.to_string(),
        "-n".to_string(),
        ns,
    ];

    spawn_session(&app, &manager, info, args)
}

#[tauri::command]
pub async fn start_mesh(
    app: AppHandle,
    manager: tauri::State<'_, Arc<SessionManager>>,
    service: String,
    port: u16,
    namespace: Option<String>,
) -> Result<SessionInfo, String> {
    let ns = namespace.unwrap_or_else(|| "oneberry-dev".to_string());

    let check = std::net::TcpStream::connect(format!("127.0.0.1:{}", port));
    if check.is_err() {
        return Err(format!("本地端口 {} 无服务监听，请先启动本地服务", port));
    }

    let id = format!("mesh-{}-{}", service, chrono::Utc::now().timestamp());
    let version = format!("devkit-{}", &id[id.len()-6..]);

    let info = SessionInfo {
        id: id.clone(),
        service: service.clone(),
        port,
        mode: "mesh".to_string(),
        started_at: chrono::Utc::now().to_rfc3339(),
        version_header: Some(version),
        status: "starting".to_string(),
    };

    let args = vec![
        "mesh".to_string(),
        service,
        "--expose".to_string(),
        port.to_string(),
        "-n".to_string(),
        ns,
    ];

    spawn_session(&app, &manager, info, args)
}

#[tauri::command]
pub async fn stop_session(
    manager: tauri::State<'_, Arc<SessionManager>>,
    session_id: String,
) -> Result<String, String> {
    match manager.remove(&session_id) {
        Some(_) => Ok(format!("会话 {} 已停止", session_id)),
        None => Err(format!("会话 {} 不存在", session_id)),
    }
}

#[tauri::command]
pub async fn list_sessions(
    manager: tauri::State<'_, Arc<SessionManager>>,
) -> Result<Vec<SessionInfo>, String> {
    Ok(manager.list())
}

/// Clean up a stale exchange/mesh session for a service.
/// This runs `ktctl recover <service>` as root to remove leftover shadow pods.
#[tauri::command]
pub async fn recover_service(
    service: String,
    namespace: Option<String>,
) -> Result<String, String> {
    let ktctl_bin = find_bin("ktctl");
    let ns = namespace.unwrap_or_else(|| "oneberry-dev".to_string());

    #[cfg(target_os = "macos")]
    {
        let cmd_str = format!(
            "{} recover {} -n {}",
            ktctl_bin.replace('\'', "'\\''"),
            service.replace('\'', "'\\''"),
            ns.replace('\'', "'\\''"),
        );
        let script = format!(
            "do shell script \"{}\" with administrator privileges",
            cmd_str.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("清理失败: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("User canceled") || stderr.contains("-128") {
                return Err("用户取消了授权".to_string());
            }
            // Recover might fail if nothing to recover — that's OK
        }
        Ok(format!("服务 {} 的残留会话已清理", service))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let output = Command::new("sudo")
            .args([&ktctl_bin, "recover", &service, "-n", &ns])
            .output()
            .map_err(|e| format!("清理失败: {}", e))?;

        if !output.status.success() {
            // Recover might fail if nothing to recover — that's OK
        }
        Ok(format!("服务 {} 的残留会话已清理", service))
    }
}
