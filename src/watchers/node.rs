use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures::stream::FuturesUnordered;
use k8s_openapi::{api::core::v1::Node, apimachinery::pkg::apis::meta::v1::Time};
use futures_util::StreamExt;
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::task;
use tokio::sync::Semaphore;
use http::{Request, Method};
use kube::{Api, Client, runtime::watcher, runtime::watcher::Event};

const NODE_METRICS_REFRESH_INTERVAL_SECS: u64 = 20;
const NODE_METRICS_POLL_INTERVAL_SECS: u64 = 1;
const NODE_METRICS_MAX_CONCURRENCY: usize = 4;

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

fn parse_cpu_quantity(value: &str) -> Option<f32> {
    if let Some(value) = value.strip_suffix('m') {
        value.parse::<f32>().ok().map(|value| value / 1_000.0)
    } else if let Some(value) = value.strip_suffix('u') {
        value.parse::<f32>().ok().map(|value| value / 1_000_000.0)
    } else if let Some(value) = value.strip_suffix('n') {
        value.parse::<f32>().ok().map(|value| value / 1_000_000_000.0)
    } else {
        value.parse::<f32>().ok()
    }
}

fn parse_cpu_capacity(node: &Node) -> Option<f32> {
    node.status
        .as_ref()
        .and_then(|status| status.capacity.as_ref())
        .and_then(|capacity| capacity.get("cpu"))
        .and_then(|quantity| parse_cpu_quantity(&quantity.0))
}

fn parse_memory_capacity(node: &Node) -> Option<f32> {
    node.status
        .as_ref()
        .and_then(|status| status.capacity.as_ref())
        .and_then(|capacity| capacity.get("memory"))
        .and_then(|quantity| {
            let value = quantity.0.as_str();
            let bytes = if let Some(value) = value.strip_suffix("Ki") {
                value.parse::<f32>().ok().map(|value| value * 1024.0)
            } else if let Some(value) = value.strip_suffix("Mi") {
                value.parse::<f32>().ok().map(|value| value * 1_048_576.0)
            } else if let Some(value) = value.strip_suffix("Gi") {
                value.parse::<f32>().ok().map(|value| value * 1_073_741_824.0)
            } else {
                value.parse::<f32>().ok()
            };

            bytes.map(|bytes| ((bytes / 1_073_741_824.0) * 100.0).round() / 100.0)
        })
}

#[cfg(test)]
mod tests {
    use super::parse_cpu_quantity;

    #[test]
    fn parses_kubernetes_cpu_quantities_as_cores() {
        assert_eq!(parse_cpu_quantity("4"), Some(4.0));
        assert_eq!(parse_cpu_quantity("4000m"), Some(4.0));
        assert_eq!(parse_cpu_quantity("2500000u"), Some(2.5));
        assert_eq!(parse_cpu_quantity("500000000n"), Some(0.5));
    }
}

pub async fn fetch_node_metrics(
    client: kube::Client,
    node_name: &str,
    cpu_total: Option<f32>,
    mem_total: Option<f32>,
) -> anyhow::Result<(
    Option<f32>, // disk_used
    Option<f32>, // disk_total
    Option<f32>, // disk_percent
    Option<f32>, // cpu_used_cores
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

    // CPU usage is a cumulative counter. Use the first sample from the summary
    // request above and take one additional sample after the one-second interval.
    let usage1 = summary.node.cpu
        .as_ref()
        .and_then(|cpu| cpu.usage_core_nano_seconds)
        .ok_or_else(|| anyhow::anyhow!("Missing usage_core_nano_seconds in first sample"))?;
    let sample1_at = Instant::now();

    tokio::time::sleep(Duration::from_secs(1)).await;

    let usage2 = match tokio::time::timeout(
        Duration::from_secs(3),
        get_cpu_usage_nanos(&client, node_name)
    ).await {
        Ok(Ok(usage)) => usage,
        Ok(Err(e)) => {
            log::error!("Error getting CPU usage 2: {}", e);
            return Err(e);
        },
        Err(_) => {
            log::error!("Timeout getting CPU usage 2");
            return Err(anyhow::anyhow!("Timeout getting CPU usage 2"));
        }
    };
    let sample2_at = Instant::now();

    let elapsed_secs = sample2_at.duration_since(sample1_at).as_secs_f32();
    let delta_nanos = usage2.saturating_sub(usage1) as f32;
    let cpu_used = if elapsed_secs > 0.0 {
        Some((delta_nanos / 1_000_000_000.0 / elapsed_secs * 100.0).round() / 100.0)
    } else {
        None
    };

    let cpu_percent = match (cpu_used, cpu_total) {
        (Some(used), Some(total)) if total > 0.0 => {
            Some(((used / total) * 100.0 * 100.0).round() / 100.0)
        }
        (Some(_), Some(_)) => Some(0.0),
        _ => None,
    };

    // Memory
    let (mem_used, mem_total, mem_percent) = if let Some(memory) = summary.node.memory {
        let used = memory.working_set_bytes.map(|b| ((b as f32 / 1_073_741_824.0) * 100.0).round() / 100.0);
        let percent = match (used, mem_total) {
            (Some(u), Some(t)) if t > 0.0 => Some(((u / t) * 100.0 * 100.0).round() / 100.0),
            _ => None,
        };

        (used, mem_total, percent)
    } else {
        (None, mem_total, None)
    };

    Ok((disk_used, disk_total, disk_percent, cpu_used, cpu_total, cpu_percent, mem_used, mem_total, mem_percent))
}

pub fn convert_node(node: Node) -> Option<NodeItem> {
    let metadata = &node.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();
    let cpu_total = parse_cpu_capacity(&node);
    let mem_total = parse_memory_capacity(&node);
    let version = node.status
        .as_ref()
        .and_then(|status| status.node_info.as_ref()).map(|info| info.kubelet_version.clone());
    let scheduling_disabled = node.spec.as_ref().and_then(|spec| spec.unschedulable).unwrap_or(false);
    let taints = node.spec.as_ref().and_then(|spec| spec.taints.clone());
    let labels: Vec<String> = node.metadata.labels.as_ref()
        .map(|labels_map| { // If labels_map == Some(BTreeMap), do:
            labels_map.iter()
                .filter(|(k, _v)| !k.contains("kubernetes"))
                .map(|(k, v)| format!("{}={}", k, v))
                .collect()
        }).unwrap_or_default();
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
            conds.iter().find(|c| c.type_ == "Ready").map(|c| {
                match c.status.as_str() {
                    "True" => "Ready",
                    "False" => "NotReady",
                    _ => "Unknown",
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
        cpu_total,
        cpu_used: None,
        cpu_percent: None,
        mem_total,
        mem_used: None,
        mem_percent: None,
    })
}

pub async fn watch_nodes(
    client: Arc<Client>,
    list: Arc<Mutex<Vec<NodeItem>>>,
    load_status: Arc<AtomicBool>,
    metrics_active: Arc<AtomicBool>,
) {
    let api: Api<Node> = Api::all(client.as_ref().clone());
    let mut stream = watcher(api, watcher::Config::default().page_size(crate::WATCHER_PAGE_SIZE)).boxed();

    let mut initial = vec![];
    let mut initialized = false;
    load_status.store(true, Ordering::Relaxed);

    {
        let client = client.clone();
        let list = Arc::clone(&list);
        let metrics_active = Arc::clone(&metrics_active);
        let metrics_semaphore = Arc::new(Semaphore::new(NODE_METRICS_MAX_CONCURRENCY));
        task::spawn(async move {
            loop {
                while !metrics_active.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_secs(NODE_METRICS_POLL_INTERVAL_SECS)).await;
                }

                let nodes_snapshot = {
                    let guard = list.lock().unwrap();
                    guard.clone()
                };

                if nodes_snapshot.is_empty() {
                    tokio::time::sleep(Duration::from_secs(NODE_METRICS_POLL_INTERVAL_SECS)).await;
                    continue;
                }

                let mut tasks = FuturesUnordered::new();
                for node in nodes_snapshot {
                    let client = client.clone();
                    let list = Arc::clone(&list);
                    let metrics_active = Arc::clone(&metrics_active);
                    let metrics_semaphore = Arc::clone(&metrics_semaphore);

                    tasks.push(async move {
                        let Ok(_permit) = metrics_semaphore.acquire_owned().await else {
                            return;
                        };

                        if !metrics_active.load(Ordering::Relaxed) {
                            return;
                        }

                        if let Ok((
                            disk_used,
                            disk_total,
                            disk_percent,
                            cpu_used_cores,
                            cpu_total,
                            cpu_percent,
                            mem_used,
                            mem_total,
                            mem_percent,
                        )) = fetch_node_metrics(
                            client.as_ref().clone(),
                            &node.name,
                            node.cpu_total,
                            node.mem_total,
                        ).await {
                            if !metrics_active.load(Ordering::Relaxed) {
                                return;
                            }

                            let mut list_guard = list.lock().unwrap();
                            if let Some(target) = list_guard.iter_mut().find(|n| n.name == node.name) {
                                target.storage_used = disk_used;
                                target.storage_total = disk_total;
                                target.storage_percent = disk_percent;
                                target.cpu_used = cpu_used_cores;
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

                for _ in 0..NODE_METRICS_REFRESH_INTERVAL_SECS {
                    if !metrics_active.load(Ordering::Relaxed) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(NODE_METRICS_POLL_INTERVAL_SECS)).await;
                }
            }
        });
    }

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),

                Event::InitApply(obj) => {
                    if let Some(item) = convert_node(obj.clone()) {
                        initial.push(item);
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
                        let mut list_guard = list.lock().unwrap();
                        if let Some(existing) = list_guard.iter_mut().find(|n| n.name == item.name) {
                            *existing = item;
                        } else {
                            list_guard.push(item);
                        }
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
