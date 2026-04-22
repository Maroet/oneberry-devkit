use serde::{Deserialize, Serialize};
use std::process::Command;
use crate::utils::find_bin;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClusterStatus {
    pub status: String, // "connected", "disconnected", "error"
    pub node_count: u32,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct K8sService {
    pub name: String,
    pub ready: u32,
    pub desired: u32,
    pub status: String, // "running", "degraded", "stopped"
}

#[tauri::command]
pub async fn check_cluster() -> Result<ClusterStatus, String> {
    let output = Command::new(find_bin("kubectl"))
        .args(["get", "nodes", "-o", "json"])
        .output();

    match output {
        Err(e) => Ok(ClusterStatus {
            status: "error".to_string(),
            node_count: 0,
            message: Some(format!("kubectl 不可用: {}", e)),
        }),
        Ok(o) if !o.status.success() => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            Ok(ClusterStatus {
                status: "disconnected".to_string(),
                node_count: 0,
                message: Some(stderr.to_string()),
            })
        }
        Ok(o) => {
            let json: serde_json::Value = serde_json::from_slice(&o.stdout)
                .map_err(|e| e.to_string())?;
            let node_count = json["items"]
                .as_array()
                .map(|arr| arr.len() as u32)
                .unwrap_or(0);

            Ok(ClusterStatus {
                status: "connected".to_string(),
                node_count,
                message: None,
            })
        }
    }
}

#[tauri::command]
pub async fn list_services() -> Result<Vec<K8sService>, String> {
    let output = Command::new(find_bin("kubectl"))
        .args([
            "get", "deployments",
            "-n", "oneberry-dev",
            "-o", "json",
        ])
        .output()
        .map_err(|e| format!("kubectl 执行失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("获取服务列表失败: {}", stderr));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| e.to_string())?;

    let services: Vec<K8sService> = json["items"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|item| {
            let name = item["metadata"]["name"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let desired = item["spec"]["replicas"]
                .as_u64()
                .unwrap_or(1) as u32;
            let ready = item["status"]["readyReplicas"]
                .as_u64()
                .unwrap_or(0) as u32;
            let status = if ready >= desired {
                "running"
            } else if ready > 0 {
                "degraded"
            } else {
                "stopped"
            };

            K8sService {
                name,
                ready,
                desired,
                status: status.to_string(),
            }
        })
        .collect();

    Ok(services)
}
