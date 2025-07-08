use futures::AsyncBufReadExt;
use kube::runtime::reflector::Lookup;
use kube::{Api, Client, Config};
use kube::config::{Kubeconfig, NamedContext};
use kube::runtime::watcher;
use k8s_openapi::api::core::v1::{Namespace, Node, Pod, Event, Secret, ConfigMap};
use k8s_openapi::api::apps::v1::Deployment;
use std::sync::{Arc, Mutex};
use futures_util::StreamExt;
use serde_json::json;
use kube::api::{DeleteParams, ListParams, LogParams, Patch, PatchParams, PostParams, PropagationPolicy};
use kube::runtime::watcher::Event as WatcherEvent;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use chrono::{Utc, DateTime};
use std::{time::Duration};
use tokio::{time::sleep};


pub fn load_embedded_icon() -> Result<crate::egui::IconData, String> {
    let img = image::load_from_memory(super::ICON_BYTES).map_err(|e| e.to_string())?.into_rgba8();
    let (width, height) = img.dimensions();
    let rgba = img.into_raw();
    Ok(crate::egui::IconData { rgba, width, height })
}

pub async fn apply_yaml(yaml: &str) -> Result<(), anyhow::Error> {
    let client = Client::try_default().await?;
    let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;
    let obj: Namespace = serde_yaml::from_value(value)?;

    let api: Api<Namespace> = Api::all(client);
    api.create(&PostParams::default(), &obj).await?;

    Ok(())
}

pub fn get_current_context_info() -> Result<NamedContext, anyhow::Error> {
    let home_path = match home::home_dir() {
        Some(path) => path.to_string_lossy().to_string(),
        None => panic!("Impossible to get your home dir!"),
    };
    let config_path = format!("{}/.kube/config", home_path);
    let config = Kubeconfig::read_from(config_path)?;

    let current_context = config
        .current_context
        .ok_or_else(|| anyhow::anyhow!("No current context set"))?;

    let context = config
        .contexts
        .iter()
        .find(|ctx| ctx.name == current_context)
        .ok_or_else(|| anyhow::anyhow!("Context '{}' not found", current_context))?;

    Ok(context.clone())
}

pub async fn get_cluster_name() -> Result<String, anyhow::Error> {
    let config = Config::infer().await?;
    Ok(config.cluster_url.host().unwrap_or("unknown").to_string())
}

pub async fn cordon_node(node_name: &str, cordoned: bool) -> Result<(), kube::Error> {
    let client = Client::try_default().await?;
    let nodes: Api<Node> = Api::all(client);
    let patch = json!({ "spec": { "unschedulable": cordoned } });
    nodes.patch(node_name, &PatchParams::apply("rustlens"), &Patch::Merge(&patch)).await?;
    Ok(())
}

pub async fn delete_pod(pod_name: &str, namespace: Option<&str>, force: bool) -> Result<(), kube::Error> {
    let client = Client::try_default().await?;
    let ns = namespace.unwrap_or("default");
    let pods: Api<Pod> = Api::namespaced(client, ns);
    let dp = if force {
        DeleteParams {
            grace_period_seconds: Some(0),
            propagation_policy: Some(PropagationPolicy::Background),
            ..DeleteParams::default()
        }
    } else {
        DeleteParams::default()
    };
    pods.delete(pod_name, &dp).await?;
    Ok(())
}

pub async fn drain_node(node_name: &str) -> anyhow::Result<()> {
    let client = Client::try_default().await?;

    // Cordon node
    let nodes: Api<Node> = Api::all(client.clone());
    let patch = json!({ "spec": { "unschedulable": true } });
    nodes.patch(node_name, &PatchParams::apply("rustlens"), &Patch::Merge(&patch)).await?;

    // Evict pods
    let pods: Api<Pod> = Api::all(client.clone());
    let lp = ListParams::default().fields(&format!("spec.nodeName={}", node_name));
    let pod_list = pods.list(&lp).await?;

    for pod in pod_list.items {
        let is_mirror = pod.metadata.annotations.as_ref()
            .and_then(|a| a.get("kubernetes.io/config.mirror"))
            .is_some();

        let is_daemonset = pod.metadata.owner_references
            .as_ref()
            .map(|owners| owners.iter().any(|o| o.kind == "DaemonSet"))
            .unwrap_or(false);

        if is_mirror || is_daemonset {
            continue;
        }

        if let Some(name) = pod.metadata.name {
            let dp = DeleteParams {
                grace_period_seconds: Some(30),
                ..DeleteParams::default()
            };
            let _ = pods.delete(&name, &dp).await;
        }
    }

    Ok(())
}

// fn parse_quantity_to_f64(q: &Quantity) -> Option<f64> {
//     let s = &q.0;
//     if s.ends_with("n") {
//         s[..s.len()-1].parse::<f64>().ok().map(|v| v / 1e9)
//     } else if s.ends_with("u") {
//         s[..s.len()-1].parse::<f64>().ok().map(|v| v / 1e6)
//     } else if s.ends_with("m") {
//         s[..s.len()-1].parse::<f64>().ok().map(|v| v / 1000.0)
//     } else {
//         s.parse::<f64>().ok()
//     }
// }

// fn percentage(used: f64, allocatable: f64) -> u8 {
//     ((used / allocatable) * 100.0).round().min(100.0) as u8
// }

pub fn format_age(ts: &Time) -> String {
    let now = Utc::now();
    let created: DateTime<Utc> = ts.0;
    let duration = now - created;

    if duration.num_days() > 0 {
        format!("{}d", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{}h", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{}m", duration.num_minutes())
    } else {
        format!("{}s", duration.num_seconds())
    }
}

pub async fn watch_nodes(client: Arc<Client>, nodes_list: Arc<Mutex<Vec<super::NodeItem>>>) {
    let api: Api<Node> = Api::all(client.as_ref().clone());
    let mut node_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    let percent = |alloc: &str, cap: &str| -> Option<u8> {
        let parse_quantity = |q: &str| -> Option<f64> {
            // very naive parser
            if q.ends_with("m") {
                q.trim_end_matches('m').parse::<f64>().ok().map(|v| v / 1000.0)
            } else if q.ends_with("Ki") {
                q.trim_end_matches("Ki").parse::<f64>().ok().map(|v| v * 1024.0)
            } else if q.ends_with("Mi") {
                q.trim_end_matches("Mi").parse::<f64>().ok().map(|v| v * 1024.0 * 1024.0)
            } else {
                q.parse::<f64>().ok()
            }
        };

        let a = parse_quantity(alloc)?;
        let c = parse_quantity(cap)?;
        if c > 0.0 {
            Some((100.0 * (1.0 - a / c)).round() as u8)
        } else {
            None
        }
    };

    while let Some(event) = node_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(node) => {
                    if let Some(name) = node.metadata.name {
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

                        let roles = node.metadata.labels.unwrap_or_default()
                            .iter()
                            .filter_map(|(key, _)| {
                                key.strip_prefix("node-role.kubernetes.io/").map(|s| s.to_string())
                            })
                            .collect::<Vec<_>>();

                        let scheduling_disabled = node.spec.as_ref().and_then(|spec| spec.unschedulable).unwrap_or(false);
                        let taints = node.spec.as_ref().and_then(|spec| spec.taints.clone());

                        let alloc = node.status.as_ref().unwrap().allocatable.as_ref().unwrap();
                        let cap = node.status.as_ref().unwrap().capacity.as_ref().unwrap();
                        let cpu_percent = percent(alloc.get("cpu").unwrap().0.as_str(), cap.get("cpu").unwrap().0.as_str()).unwrap_or(0) as f32;
                        let mem_percent = percent(alloc.get("memory").unwrap().0.as_str(), cap.get("memory").unwrap().0.as_str()).unwrap_or(0) as f32;

                        let storage = node.status.as_ref()
                            .and_then(|s| s.allocatable.as_ref())
                            .and_then(|res| res.get("ephemeral-storage"))
                            .map(|q| q.0.clone());

                        let creation_timestamp = node.metadata.creation_timestamp.clone();

                        initial.push(super::NodeItem {
                            name,
                            status,
                            roles,
                            scheduling_disabled,
                            taints,
                            cpu_percent,
                            mem_percent,
                            storage,
                            creation_timestamp,
                        });
                    }
                }
                watcher::Event::InitDone => {
                    let mut nodes = nodes_list.lock().unwrap();
                    *nodes = initial.clone();
                    initialized = true;
                }
                watcher::Event::Apply(node) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(name) = node.metadata.name {
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

                        let roles = node.metadata.labels.unwrap_or_default()
                            .iter()
                            .filter_map(|(key, _)| {
                                key.strip_prefix("node-role.kubernetes.io/").map(|s| s.to_string())
                            })
                            .collect::<Vec<_>>();

                        let scheduling_disabled = node.spec.as_ref().and_then(|spec| spec.unschedulable).unwrap_or(false);
                        let taints = node.spec.as_ref().and_then(|spec| spec.taints.clone());

                        let mut nodes = nodes_list.lock().unwrap();

                        let alloc = node.status.as_ref().unwrap().allocatable.as_ref().unwrap();
                        let cap = node.status.as_ref().unwrap().capacity.as_ref().unwrap();
                        let cpu_percent = percent(alloc.get("cpu").unwrap().0.as_str(), cap.get("cpu").unwrap().0.as_str()).unwrap_or(0) as f32;
                        let mem_percent = percent(alloc.get("memory").unwrap().0.as_str(), cap.get("memory").unwrap().0.as_str()).unwrap_or(0) as f32;

                        // DEBUG
                        //let cpu_percent = 44.0;
                        //let mem_percent = 44.0;

                        let storage = node.status.as_ref()
                            .and_then(|s| s.allocatable.as_ref())
                            .and_then(|res| res.get("ephemeral-storage"))
                                .map(|q| q.0.clone());

                        let creation_timestamp = node.metadata.creation_timestamp.clone();

                        if let Some(existing) = nodes.iter_mut().find(|n| n.name == name) {
                            // renew existing node
                            existing.status = status;
                            existing.taints = taints;
                            existing.roles = roles;
                            existing.cpu_percent = cpu_percent;
                            existing.mem_percent = mem_percent;
                            existing.storage = storage;
                            existing.creation_timestamp = creation_timestamp;
                            existing.scheduling_disabled = scheduling_disabled;
                        } else {
                            // add new node
                            nodes.push(super::NodeItem {
                                name,
                                status,
                                roles,
                                scheduling_disabled,
                                taints,
                                cpu_percent,
                                mem_percent,
                                storage,
                                creation_timestamp,
                            });
                        }
                    }
                }
                watcher::Event::Delete(node) => {
                    if let Some(name) = node.metadata.name {
                        let mut nodes = nodes_list.lock().unwrap();
                        nodes.retain(|n| n.name != name);
                    }
                }
            },
            Err(e) => {
                eprintln!("Node watch error: {:?}", e);
            }
        }
    }
}

pub async fn watch_events(client: Arc<Client>, events_list: Arc<Mutex<Vec<super::EventItem>>>) {
    let api: Api<Event> = Api::all(client.as_ref().clone());
    let mut event_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(ev) => {
                    if let Some(message) = ev.message.clone() {
                        let reason = ev.reason.clone().unwrap_or_else(|| "Unknown".to_string());
                        let involved = format!(
                            "{}/{}",
                            ev.involved_object.kind.clone().unwrap_or_else(|| "Unknown".to_string()),
                            ev.involved_object.name.clone().unwrap_or_else(|| "Unknown".to_string())
                        );
                        let type_ = ev.type_.clone().unwrap_or_else(|| "Normal".to_string());
                        let timestamp = ev.event_time
                            .as_ref()
                            .map(|t| t.0.to_rfc3339())
                            .or_else(|| ev.last_timestamp.as_ref().map(|t| t.0.to_rfc3339()))
                            .unwrap_or_else(|| "N/A".to_string());

                        let namespace = ev.involved_object.namespace.clone().unwrap_or_else(|| "default".to_string());
                        let creation_timestamp = ev.metadata.creation_timestamp.clone();

                        initial.push(super::EventItem {
                            message,
                            reason,
                            involved_object: involved,
                            event_type: type_,
                            timestamp,
                            namespace,
                            creation_timestamp,
                        });
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = events_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;
                }
                watcher::Event::Apply(ev) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(message) = ev.message.clone() {
                        let reason = ev.reason.clone().unwrap_or_else(|| "Unknown".to_string());
                        let involved = format!(
                            "{}/{}",
                            ev.involved_object.kind.clone().unwrap_or_else(|| "Unknown".to_string()),
                            ev.involved_object.name.clone().unwrap_or_else(|| "Unknown".to_string())
                        );
                        let type_ = ev.type_.clone().unwrap_or_else(|| "Normal".to_string());
                        let timestamp = ev.event_time
                            .as_ref()
                            .map(|t| t.0.to_rfc3339())
                            .or_else(|| ev.last_timestamp.as_ref().map(|t| t.0.to_rfc3339()))
                            .unwrap_or_else(|| "N/A".to_string());

                        let mut list = events_list.lock().unwrap();
                        // check and clear length before adding
                        let list_len = list.len();
                        if list_len >= 500 {
                            list.drain(0..list_len - 499);
                        }
                        let namespace = ev.involved_object.namespace.clone().unwrap_or_else(|| "default".to_string());
                        let creation_timestamp = ev.metadata.creation_timestamp.clone();

                        list.push(super::EventItem {
                            message,
                            reason,
                            involved_object: involved,
                            event_type: type_,
                            timestamp,
                            namespace,
                            creation_timestamp,
                        });
                    }
                }
                watcher::Event::Delete(_) => {}
            },
            Err(e) => {
                eprintln!("Event watch error: {:?}", e);
            }
        }
    }
}

pub async fn watch_namespaces(client: Arc<Client>, ns_list: Arc<Mutex<Vec<super::NamespaceItem>>>) {
    let api: Api<Namespace> = Api::all(client.as_ref().clone());
    let mut ns_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = ns_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(ns) => {
                    if let Some(name) = ns.metadata.name {
                        let creation_timestamp = ns.metadata.creation_timestamp.clone();
                        let phase = ns
                            .status
                            .as_ref()
                            .and_then(|s| s.phase.clone());
                        let labels = ns.metadata.labels.clone();
                        initial.push(super::NamespaceItem {
                            name,
                            creation_timestamp,
                            phase,
                            labels,
                        });
                    }
                }
                watcher::Event::InitDone => {
                    let mut ns_vec = ns_list.lock().unwrap();
                    *ns_vec = initial.clone();
                    initialized = true;
                }
                watcher::Event::Apply(ns) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(name) = ns.metadata.name {
                        let mut ns_vec = ns_list.lock().unwrap();
                        if !ns_vec.iter().any(|n| n.name == name) {
                            let creation_timestamp = ns.metadata.creation_timestamp.clone();
                            let phase = ns
                                .status
                                .as_ref()
                                .and_then(|s| s.phase.clone());
                            let labels = ns.metadata.labels.clone();
                            ns_vec.push(super::NamespaceItem {
                                name,
                                creation_timestamp,
                                phase,
                                labels,
                            });
                        }
                    }
                }
                watcher::Event::Delete(ns) => {
                    if let Some(name) = ns.metadata.name {
                        let mut ns_vec = ns_list.lock().unwrap();
                        ns_vec.retain(|n| n.name != name);
                    }
                }
            },
            Err(e) => {
                eprintln!("Namespace watch error: {:?}", e);
            }
        }
    }
}

fn convert_deployment_to_item(deploy: Deployment) -> super::DeploymentItem {
    let name = deploy.metadata.name.unwrap_or_default();
    let namespace = deploy.metadata.namespace.unwrap_or_else(|| "default".to_string());
    let status = deploy.status.unwrap_or_default();

    super::DeploymentItem {
        name,
        namespace,
        ready_replicas: status.ready_replicas.unwrap_or(0),
        available_replicas: status.available_replicas.unwrap_or(0),
        updated_replicas: status.updated_replicas.unwrap_or(0),
        replicas: status.replicas.unwrap_or(0),
        creation_timestamp: deploy.metadata.creation_timestamp,
    }
}

pub async fn watch_deployments(client: Arc<Client>, deployments_list: Arc<Mutex<Vec<super::DeploymentItem>>>, selected_ns: String) {
    let api: Api<Deployment> = Api::namespaced(client.as_ref().clone(), &selected_ns);
    let mut watcher_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = watcher_stream.next().await {
        match event {
            Ok(watch_event) => match watch_event {
                WatcherEvent::Init => initial.clear(),

                WatcherEvent::InitApply(deploy) => {
                    let item = convert_deployment_to_item(deploy);
                    initial.push(item);
                }

                WatcherEvent::InitDone => {
                    let mut list = deployments_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;
                }

                WatcherEvent::Apply(deploy) => {
                    if !initialized {
                        continue;
                    }

                    let item = convert_deployment_to_item(deploy);
                    let mut list = deployments_list.lock().unwrap();

                    list.push(item);
                }

                WatcherEvent::Delete(_) => {}
            },
            Err(e) => {
                eprintln!("Deployment watch error: {:?}", e);
            }
        }
    }
}

fn convert_secret(secret: Secret) -> Option<super::SecretItem> {
    let name = secret.name().unwrap().to_string();
    let labels = secret
        .metadata
        .labels
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(", ");

    let keys = secret
        .data
        .as_ref()
        .map(|d| d.keys().cloned().collect::<Vec<_>>().join(", "))
        .unwrap_or_else(|| "-".into());

    let type_ = secret.type_.unwrap_or_else(|| "-".into());

    let age = secret
        .metadata
        .creation_timestamp
        .as_ref()
        .map(|ts| {
            let now: DateTime<Utc> = Utc::now();
            let duration = now - ts.0;
            format!("{}d", duration.num_days())
        })
        .unwrap_or_else(|| "-".into());

    Some(super::SecretItem {
        name,
        labels,
        keys,
        type_,
        age,
    })
}

type WatchFn<T> = Arc<dyn Fn(Arc<Client>, Arc<T>, String) + Send + Sync>;
pub fn spawn_namespace_watcher_loop<T: Send + Sync + 'static>(
    client: Arc<Client>,
    data: Arc<T>,
    selected_namespace: Arc<Mutex<Option<String>>>,
    watch_fn: WatchFn<T>,
    interval: Duration,
) {
    let data_outer = Arc::clone(&data);
    let namespace_outer = Arc::clone(&selected_namespace);
    let client_outer = Arc::clone(&client);
    let watch_fn = Arc::clone(&watch_fn);

    tokio::spawn(async move {
        let mut last_ns = String::new();

        loop {
            let ns = namespace_outer
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "default".to_string());

            if ns != last_ns {
                let data_clone = Arc::clone(&data_outer);
                let client_clone = Arc::clone(&client_outer);
                let fn_clone = Arc::clone(&watch_fn);
                let ns_clone = ns.clone();

                tokio::spawn(async move {
                    (fn_clone)(client_clone, data_clone, ns_clone);
                });

                last_ns = ns;
            }

            sleep(interval).await;
        }
    });
}

pub fn convert_configmap(cm: ConfigMap) -> Option<super::ConfigMapItem> {
    let age = cm
        .metadata
        .creation_timestamp
        .as_ref()
        .map(|ts| {
            let now: DateTime<Utc> = Utc::now();
            let duration = now - ts.0;
            format!("{}d", duration.num_days())
        })
        .unwrap_or_else(|| "-".into());
    Some(super::ConfigMapItem {
        name: cm.metadata.name.clone()?,
        labels: cm.metadata.labels.unwrap_or_default(),
        keys: cm.data.as_ref().map(|d| d.keys().cloned().collect()).unwrap_or_default(),
        type_: "Opaque".to_string(),
        age: age,
    })
}

pub async fn watch_configmaps(client: Arc<Client>, configmaps_list: Arc<Mutex<Vec<super::ConfigMapItem>>>, selected_ns: String) {
    let cms: Api<ConfigMap> = Api::namespaced(client.as_ref().clone(), &selected_ns);
    let mut stream = watcher(cms, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(cm) => {
                    if let Some(item) = convert_configmap(cm) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = configmaps_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;
                }
                watcher::Event::Apply(cm) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_configmap(cm) {
                        let mut list = configmaps_list.lock().unwrap();
                        list.push(item);
                    }
                }
                watcher::Event::Delete(_) => {}
            },
            Err(e) => eprintln!("ConfigMap watch error: {:?}", e),
        }
    }
}

pub async fn watch_secrets(client: Arc<Client>, secrets_list: Arc<Mutex<Vec<super::SecretItem>>>, selected_ns: String) {
    let secrets: Api<Secret> = Api::namespaced(client.as_ref().clone(), &selected_ns);
    let mut stream = watcher(secrets, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(secret) => {
                    if let Some(item) = convert_secret(secret) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = secrets_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;
                }
                watcher::Event::Apply(secret) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_secret(secret) {
                        let mut list = secrets_list.lock().unwrap();
                        list.push(item);
                    }
                }
                watcher::Event::Delete(_) => {}
            },
            Err(e) => eprintln!("Secret watch error: {:?}", e),
        }
    }
}

pub async fn watch_pods(client: Arc<Client>, pods_list: Arc<Mutex<Vec<super::PodItem>>>, selected_ns: String) {
    let api: Api<Pod> = Api::namespaced(client.as_ref().clone(), &selected_ns);
    let mut pod_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = pod_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(pod) => {
                    if let Some(name) = pod.metadata.name {
                        let creation_timestamp = pod.status.as_ref().and_then(|s| s.start_time.clone());
                        let node_name = pod.spec.as_ref().and_then(|s| s.node_name.clone());
                        if let Some(statuses) = pod.status.as_ref().and_then(|s| s.container_statuses.clone()) {
                            let mut containers = vec![];
                            let mut ready = 0;
                            let mut restart_count = 0;
                            let mut pod_has_crashloop = false;
                            for cs in statuses {
                                let state = cs.state.as_ref().and_then(|s| {
                                    if s.running.is_some() {
                                        Some("Running".to_string())
                                    } else if s.waiting.is_some() {
                                        Some("Waiting".to_string())
                                    } else if s.terminated.is_some() {
                                        Some("Terminated".to_string())
                                    } else {
                                        None
                                    }
                                });

                                let is_crashloop = cs.state.as_ref()
                                    .and_then(|s| s.waiting.as_ref())
                                    .and_then(|w| w.reason.clone())
                                    .map(|r| r == "CrashLoopBackOff")
                                    .unwrap_or(false);
                                if is_crashloop {
                                    pod_has_crashloop = true;
                                }

                                restart_count += cs.restart_count;

                                let message = cs.state.as_ref().and_then(|s| {
                                    if let Some(waiting) = &s.waiting {
                                        waiting.message.clone()
                                    } else if let Some(terminated) = &s.terminated {
                                        terminated.message.clone()
                                    } else {
                                        None
                                    }
                                });

                                if cs.ready {
                                    ready += 1;
                                }

                                containers.push(super::ContainerStatusItem {
                                    name: cs.name,
                                    state,
                                    message,
                                });
                            }

                            initial.push(super::PodItem {
                                name,
                                phase: pod.status.as_ref().and_then(|s| s.phase.clone()),
                                creation_timestamp,
                                ready_containers: ready,
                                total_containers: containers.len() as u32,
                                containers,
                                restart_count,
                                node_name,
                                pod_has_crashloop,
                            });
                        }
                    }
                }
                watcher::Event::InitDone => {
                    let mut pods_vec = pods_list.lock().unwrap();
                    *pods_vec = initial.clone();
                    initialized = true;
                }
                watcher::Event::Apply(pod) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(name) = pod.metadata.name {
                        let mut pods_vec = pods_list.lock().unwrap();
                        match pods_vec.iter_mut().find(|p| p.name == name) {
                            Some(existing_pod) => {
                                // renew
                                existing_pod.phase = pod.status.as_ref().and_then(|s| s.phase.clone());
                                existing_pod.creation_timestamp = pod.status.as_ref().and_then(|s| s.start_time.clone());
                                if let Some(statuses) = pod.status.as_ref().and_then(|s| s.container_statuses.clone()) {
                                    let mut containers = vec![];
                                    let mut ready = 0;

                                    for cs in statuses {
                                        let state = cs.state.as_ref().and_then(|s| {
                                            if s.running.is_some() {
                                                Some("Running".to_string())
                                            } else if s.waiting.is_some() {
                                                Some("Waiting".to_string())
                                            } else if s.terminated.is_some() {
                                                Some("Terminated".to_string())
                                            } else {
                                                None
                                            }
                                        });

                                        let message = cs.state.as_ref().and_then(|s| {
                                            if let Some(waiting) = &s.waiting {
                                                waiting.message.clone()
                                            } else if let Some(terminated) = &s.terminated {
                                                terminated.message.clone()
                                            } else {
                                                None
                                            }
                                        });

                                        if cs.ready {
                                            ready += 1;
                                        }

                                        containers.push(super::ContainerStatusItem {
                                            name: cs.name,
                                            state,
                                            message,
                                        });
                                    }

                                    existing_pod.ready_containers = ready;
                                    existing_pod.total_containers = containers.len() as u32;
                                    existing_pod.containers = containers;
                                }
                            }
                            None => {
                                // add new
                                let creation_timestamp = pod.status.as_ref().and_then(|s| s.start_time.clone());
                                let node_name = pod.spec.as_ref().and_then(|s| s.node_name.clone());
                                if let Some(statuses) = pod.status.as_ref().and_then(|s| s.container_statuses.clone()) {
                                    let mut containers = vec![];
                                    let mut ready = 0;
                                    let mut restart_count = 0;
                                    let mut pod_has_crashloop = false;
                                    for cs in statuses {
                                        let state = cs.state.as_ref().and_then(|s| {
                                            if s.running.is_some() {
                                                Some("Running".to_string())
                                            } else if s.waiting.is_some() {
                                                Some("Waiting".to_string())
                                            } else if s.terminated.is_some() {
                                                Some("Terminated".to_string())
                                            } else {
                                                None
                                            }
                                        });

                                        let is_crashloop = cs.state.as_ref()
                                            .and_then(|s| s.waiting.as_ref())
                                            .and_then(|w| w.reason.clone())
                                            .map(|r| r == "CrashLoopBackOff")
                                            .unwrap_or(false);
                                        if is_crashloop {
                                            pod_has_crashloop = true;
                                        }

                                        restart_count += cs.restart_count;

                                        let message = cs.state.as_ref().and_then(|s| {
                                            if let Some(waiting) = &s.waiting {
                                                waiting.message.clone()
                                            } else if let Some(terminated) = &s.terminated {
                                                terminated.message.clone()
                                            } else {
                                                None
                                            }
                                        });

                                        if cs.ready {
                                            ready += 1;
                                        }

                                        containers.push(super::ContainerStatusItem {
                                            name: cs.name,
                                            state,
                                            message,
                                        });
                                    }

                                    pods_vec.push(super::PodItem {
                                        name,
                                        phase: pod.status.as_ref().and_then(|s| s.phase.clone()),
                                        creation_timestamp,
                                        ready_containers: ready,
                                        total_containers: containers.len() as u32,
                                        containers,
                                        restart_count,
                                        node_name,
                                        pod_has_crashloop,
                                    });
                                }
                            }
                        }

                    }
                }
                watcher::Event::Delete(pod) => {
                    if let Some(name) = pod.metadata.name {
                        let mut ns_vec = pods_list.lock().unwrap();
                        ns_vec.retain(|p| p.name != name);
                    }
                }
            },
            Err(e) => {
                eprintln!("Pods watch error: {:?}", e);
            }
        }
    }
}

pub async fn fetch_logs(client: Arc<Client>, namespace: &str, pod_name: &str, container_name: &str, buffer: Arc<Mutex<String>>) {
    //let client = Client::try_default().await.unwrap();
    let pods: Api<Pod> = Api::namespaced(client.as_ref().clone(), namespace);

    let lp = &LogParams { tail_lines: Some(super::MAX_LOG_LINES as i64), container: Some(container_name.to_string()),..Default::default() };
    match pods.logs(pod_name, lp).await {
        Ok(initial) => {
            buffer.lock().unwrap().clear();
            let mut buf = buffer.lock().unwrap();
            *buf = initial;
            buf.push('\n');
        }
        Err(e) => eprintln!("failed to get initial logs: {:?}", e),
    }

    let lp = &LogParams { follow: true, container: Some(container_name.to_string()), since_seconds: Some(1), ..Default::default() };
    let mut log_lines = pods.log_stream(pod_name, lp)
        .await
        .unwrap()
        .lines();

    while let Some(line) = log_lines.next().await {
        match line {
            Ok(text) => {
                let mut buf = buffer.lock().unwrap();
                let mut lines: Vec<&str> = buf.lines().collect();
                lines.push(&text);
                if lines.len() > super::MAX_LOG_LINES {
                    lines = lines[lines.len() - super::MAX_LOG_LINES..].to_vec();
                }
                *buf = lines.join("\n");
                buf.push('\n');
            }
            Err(e) => {
                eprintln!("log stream error: {:?}", e);
                break;
            }
        }
    }
}
