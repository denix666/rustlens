use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures::stream::FuturesUnordered;
use k8s_openapi::{api::core::v1::Node, apimachinery::pkg::apis::meta::v1::Time};
use futures_util::StreamExt;
use serde::Deserialize;
use std::time::Duration;
use tokio::task;
use http::{Request, Method};
use kube::{Api, Client, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Deserialize)]
struct Summary {
    node: NodeStats,
}

#[derive(Debug, Deserialize)]
struct NodeStats {
    fs: Option<FileSystemStats>,
    cpu: Option<CPUStats>,
    memory: Option<MemoryStats>,
}

#[derive(Debug, Deserialize)]
struct FileSystemStats {
    #[serde(rename = "capacityBytes")]
    capacity_bytes: Option<u64>,
    #[serde(rename = "usedBytes")]
    used_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CPUStats {
    #[serde(rename = "usageCoreNanoSeconds")]
    usage_core_nano_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct MemoryStats {
    #[serde(rename = "workingSetBytes")]
    working_set_bytes: Option<u64>,
}

#[derive(Clone)]
pub struct NodeItem {
    pub name: String,
    pub status: String, // "Ready", "NotReady", "Unknown"
    pub roles: Vec<String>,
    pub labels: Vec<String>,
    pub scheduling_disabled: bool,
    pub taints: Option<Vec<k8s_openapi::api::core::v1::Taint>>,
    pub creation_timestamp: Option<Time>,
    pub cpu_total: Option<f32>,
    pub cpu_used: Option<f32>,
    pub cpu_percent: Option<f32>,
    pub mem_total: Option<f32>,
    pub mem_used: Option<f32>,
    pub mem_percent: Option<f32>,
    pub version: Option<String>,
    pub storage_total: Option<f32>,
    pub storage_used: Option<f32>,
    pub storage_percent: Option<f32>,
}

async fn get_cpu_usage_nanos(client: &Client, node_name: &str) -> anyhow::Result<u64> {
    let path = format!("/api/v1/nodes/{}/proxy/stats/summary", node_name);
    let req = Request::builder()
        .method(Method::GET)
        .uri(path)
        .body(Vec::new())?;
    let summary: Summary = client.request(req).await?;
    summary.node.cpu
        .and_then(|c| c.usage_core_nano_seconds)
        .ok_or_else(|| anyhow::anyhow!("Missing usage_core_nano_seconds"))
}

pub async fn fetch_node_metrics(
    client: kube::Client,
    node_name: &str,
) -> anyhow::Result<(
    Option<f32>, // disk_used
    Option<f32>, // disk_total
    Option<f32>, // disk_percent
    Option<f32>, // cpu_usage
    Option<f32>, // cpu_capacity
    Option<f32>, // cpu_percent
    Option<f32>, // mem_used
    Option<f32>, // mem_total
    Option<f32>, // mem_percent
)> {
    let path = format!("/api/v1/nodes/{}/proxy/stats/summary", node_name);
    let req = Request::builder()
        .method(Method::GET)
        .uri(path)
        .body(Vec::new())?;
    let summary: Summary = client.request(req).await?;

    // Disk
    let (disk_used, disk_total, disk_percent) = if let Some(fs) = summary.node.fs {
        let used = fs.used_bytes.map(|b| ((b as f32 / 1_073_741_824.0) * 100.0).round() / 100.0);
        let total = fs.capacity_bytes.map(|b| ((b as f32 / 1_073_741_824.0) * 100.0).round() / 100.0);
        let percent = match (used, total) {
            (Some(u), Some(t)) if t > 0.0 => Some(((u / t) * 100.0 * 100.0).round() / 100.0),
            _ => None,
        };
        (used, total, percent)
    } else {
        (None, None, None)
    };

    // CPU
    let usage1 = match tokio::time::timeout(
        Duration::from_secs(3),
        get_cpu_usage_nanos(&client, node_name)
    ).await {
        Ok(Ok(usage)) => usage,
        Ok(Err(e)) => {
            log::error!("Error getting CPU usage 1: {}", e);
            0
        },
        Err(_) => {
            log::error!("Timeout getting CPU usage 1");
            0
        }
    };

    tokio::time::sleep(Duration::from_secs(1)).await;

    let usage2 = match tokio::time::timeout(
        Duration::from_secs(3),
        get_cpu_usage_nanos(&client, node_name)
    ).await {
        Ok(Ok(usage)) => usage,
        Ok(Err(e)) => {
            log::error!("Error getting CPU usage 2: {}", e);
            usage1
        },
        Err(_) => {
            log::error!("Timeout getting CPU usage 2");
            usage1
        }
    };

    let delta_nanos = usage2.saturating_sub(usage1) as f32;
    let cpu_usage = Some((delta_nanos / 1_000_000_000.0 * 100.0).round() / 100.0);

    let api: Api<Node> = Api::all(client.clone());
    let node = match api.get(node_name).await {
        Ok(n) => n,
        Err(e) => {
            log::error!("Error getting node info: {}", e);
            return Ok((None, None, None, None, None, None, None, None, None));
        }
    };

    let cpu_total = node.status
        .as_ref()
        .and_then(|s| s.capacity.as_ref())
        .and_then(|c| c.get("cpu"))
        .and_then(|q| q.0.parse::<f32>().ok());

    let cpu_percent = if let Some(total) = cpu_total {
        if total > 0.0 {
            Some(((delta_nanos / (1_000_000_000.0 * total) * 100.0) * 100.0).round() / 100.0)
        } else {
            Some(0.0)
        }
    } else {
        Some(((delta_nanos / 1_000_000_000.0) * 100.0).round() / 100.0)
    };

    // Memory
    let (mem_used, mem_total, mem_percent) = if let Some(memory) = summary.node.memory {
        let used = memory.working_set_bytes.map(|b| ((b as f32 / 1_073_741_824.0) * 100.0).round() / 100.0);

        let api: Api<Node> = Api::all(client.clone());
        let node = api.get(node_name).await?;

        let total = node.status
            .as_ref()
            .and_then(|s| s.capacity.as_ref())
            .and_then(|c| c.get("memory"))
            .and_then(|q| {
                // Convert Ki, Mi, Gi to bytes
                let s = q.0.clone();
                if s.ends_with("Ki") {
                    s[..s.len()-2].parse::<f32>().ok().map(|v| v * 1024.0)
                } else if s.ends_with("Mi") {
                    s[..s.len()-2].parse::<f32>().ok().map(|v| v * 1_048_576.0)
                } else if s.ends_with("Gi") {
                    s[..s.len()-2].parse::<f32>().ok().map(|v| v * 1_073_741_824.0)
                } else {
                    s.parse::<f32>().ok()
                }
            })
            .map(|bytes| ((bytes / 1_073_741_824.0) * 100.0).round() / 100.0);

        let percent = match (used, total) {
            (Some(u), Some(t)) if t > 0.0 => Some(((u / t) * 100.0 * 100.0).round() / 100.0),
            _ => None,
        };

        (used, total, percent)
    } else {
        (None, None, None)
    };

    Ok((disk_used, disk_total, disk_percent, cpu_usage, cpu_total, cpu_percent, mem_used, mem_total, mem_percent))
}

pub fn convert_node(node: Node) -> Option<NodeItem> {
    let metadata = &node.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();
    let version = node.status
        .as_ref()
        .and_then(|status| status.node_info.as_ref())
        .and_then(|info| Some(info.kubelet_version.clone()));
    let scheduling_disabled = node.spec.as_ref().and_then(|spec| spec.unschedulable).unwrap_or(false);
    let taints = node.spec.as_ref().and_then(|spec| spec.taints.clone());
    let labels: Vec<String> = node.metadata.labels.as_ref()
        .map(|labels_map| { // If labels_map == Some(BTreeMap), do:
            labels_map.iter()
                .filter(|(k, _v)| !k.contains("kubernetes"))
                .map(|(k, v)| format!("{}={}", k.to_string(), v.to_string()))
                .collect()
        }).unwrap_or_else(Vec::new);
    let roles = node.metadata.labels.unwrap_or_default()
        .iter()
        .filter_map(|(key, value)| {
            if let Some(s) = key.strip_prefix("node-role.kubernetes.io/") {
                Some(s.to_string())
            } else if key == "kubernetes.io/role" {
                Some(value.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let status = node
        .status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .and_then(|conds| {
            conds.iter().find(|c| c.type_ == "Ready").and_then(|c| {
                match c.status.as_str() {
                    "True" => Some("Ready"),
                    "False" => Some("NotReady"),
                    _ => Some("Unknown"),
                }
            })
        })
        .unwrap_or("Unknown")
        .to_string();


    Some(NodeItem {
        name,
        creation_timestamp,
        status,
        roles,
        scheduling_disabled,
        taints,
        version,
        labels,
        storage_total: None,
        storage_used: None,
        storage_percent: None,
        cpu_total: None,
        cpu_used: None,
        cpu_percent: None,
        mem_total: None,
        mem_used: None,
        mem_percent: None,
    })
}

pub async fn watch_nodes(client: Arc<Client>, list: Arc<Mutex<Vec<NodeItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Node> = Api::all(client.as_ref().clone());
    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;
    load_status.store(true, Ordering::Relaxed);

    {
        let client = client.clone();
        let list = Arc::clone(&list);
        task::spawn(async move {
            loop {
                let nodes_snapshot = {
                    let guard = list.lock().unwrap();
                    guard.clone()
                };

                let mut tasks = FuturesUnordered::new();
                for node in nodes_snapshot {
                    let client = client.clone();
                    let list = Arc::clone(&list);

                    tasks.push(async move {
                        if let Ok((
                            disk_used,
                            disk_total,
                            disk_percent,
                            cpu_usage,
                            cpu_total,
                            cpu_percent,
                            mem_used,
                            mem_total,
                            mem_percent,
                        )) = fetch_node_metrics(client.as_ref().clone(), &node.name).await {
                            let mut list_guard = list.lock().unwrap();
                            if let Some(target) = list_guard.iter_mut().find(|n| n.name == node.name) {
                                target.storage_used = disk_used;
                                target.storage_total = disk_total;
                                target.storage_percent = disk_percent;
                                target.cpu_used = cpu_usage;
                                target.cpu_total = cpu_total;
                                target.cpu_percent = cpu_percent;
                                target.mem_used = mem_used;
                                target.mem_total = mem_total;
                                target.mem_percent = mem_percent;
                            }
                        }
                    });
                }

                while tasks.next().await.is_some() {}
                tokio::time::sleep(Duration::from_secs(180)).await;
            }
        });
    }

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),

                Event::InitApply(obj) => {
                    if let Some(item) = convert_node(obj.clone()) {
                        let item_clone = item.clone();
                        initial.push(item);

                        let client = client.clone();
                        let list = Arc::clone(&list);
                        task::spawn(async move {
                            if let Ok((
                                disk_used,
                                disk_total,
                                disk_percent,
                                cpu_usage,
                                cpu_total,
                                cpu_percent,
                                mem_used,
                                mem_total,
                                mem_percent,
                            )) = fetch_node_metrics(client.as_ref().clone(), &item_clone.name).await {
                                let mut list_guard = list.lock().unwrap();
                                if let Some(target) = list_guard.iter_mut().find(|n| n.name == item_clone.name) {
                                    target.storage_used = disk_used;
                                    target.storage_total = disk_total;
                                    target.storage_percent = disk_percent;
                                    target.cpu_used = cpu_usage;
                                    target.cpu_total = cpu_total;
                                    target.cpu_percent = cpu_percent;
                                    target.mem_used = mem_used;
                                    target.mem_total = mem_total;
                                    target.mem_percent = mem_percent;
                                }
                            }
                        });
                    }
                }

                Event::InitDone => {
                    let mut list_guard = list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;
                    load_status.store(false, Ordering::Relaxed);
                }

                Event::Apply(obj) => {
                    if !initialized {
                        continue;
                    }

                    if let Some(item) = convert_node(obj.clone()) {
                        let item_clone = item.clone();
                        let mut list_guard = list.lock().unwrap();
                        if let Some(existing) = list_guard.iter_mut().find(|n| n.name == item.name) {
                            *existing = item;
                        } else {
                            list_guard.push(item);
                        }

                        let client = client.clone();
                        let list = Arc::clone(&list);
                        task::spawn(async move {
                            if let Ok((
                                disk_used,
                                disk_total,
                                disk_percent,
                                cpu_usage,
                                cpu_total,
                                cpu_percent,
                                mem_used,
                                mem_total,
                                mem_percent,
                            )) = fetch_node_metrics(client.as_ref().clone(), &item_clone.name).await {
                                let mut list_guard = list.lock().unwrap();
                                if let Some(target) = list_guard.iter_mut().find(|n| n.name == item_clone.name) {
                                    target.storage_used = disk_used;
                                    target.storage_total = disk_total;
                                    target.storage_percent = disk_percent;
                                    target.cpu_used = cpu_usage;
                                    target.cpu_total = cpu_total;
                                    target.cpu_percent = cpu_percent;
                                    target.mem_used = mem_used;
                                    target.mem_total = mem_total;
                                    target.mem_percent = mem_percent;
                                }
                            }
                        });
                    }
                }

                Event::Delete(obj) => {
                    if let Some(name) = obj.metadata.name {
                        let mut list_guard = list.lock().unwrap();
                        list_guard.retain(|n| n.name != name);
                    }
                }
            },

            Err(e) => {
                log::error!("Nodes watch error: {:?}", e);
            }
        }
    }
}
