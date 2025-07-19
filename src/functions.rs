use eframe::egui::Color32;
use futures::{AsyncBufReadExt};
use k8s_openapi::{Metadata, NamespaceResourceScope, Resource};
use kube::runtime::reflector::Lookup;
use kube::{Api, Client, Config};
use kube::config::{Kubeconfig, NamedContext};
use kube::runtime::watcher;
use kube::discovery;
use k8s_openapi::api::core::v1::{Namespace, Node, Pod, Event, Secret, ConfigMap, PersistentVolumeClaim, PersistentVolume, Service, Endpoints};
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet, ReplicaSet, DaemonSet};
use k8s_openapi::api::storage::v1::{StorageClass, CSIDriver};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::networking::v1::{Ingress, NetworkPolicy};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use futures_util::StreamExt;
use serde_json::json;
use kube::api::{DeleteParams, ListParams, LogParams, Patch, PatchParams, PostParams, PropagationPolicy};
use kube::runtime::watcher::Event as WatcherEvent;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use chrono::{Utc, DateTime};
use serde_yaml;
use egui::{Ui, TextBuffer, TextFormat, FontId};
use egui::epaint::{text::LayoutJob};

pub fn search_layouter(search: String) -> Box<dyn FnMut(&Ui, &dyn TextBuffer, f32) -> Arc<egui::Galley>> {
    let search_lower = search.to_lowercase();

    Box::new(move |ui: &egui::Ui, text: &dyn TextBuffer, _wrap_width: f32| {
        let mut job = LayoutJob::default();
        let text_str = text.as_str();

        if search_lower.is_empty() {
            job.append(text_str, 0.0, TextFormat::default());
        } else {
            for line in text_str.lines() {
                let line_lower = line.to_lowercase();
                let mut cursor = 0;

                while cursor < line.len() {
                    let rel_pos = line_lower[cursor..].find(&search_lower);

                    match rel_pos {
                        Some(found_pos) => {
                            let found_start = cursor + found_pos;
                            let found_end = found_start + search.len(); // ok since search and search_lower same len

                            // Добавляем текст до найденного совпадения
                            if cursor < found_start {
                                job.append(
                                    &line[cursor..found_start],
                                    0.0,
                                    TextFormat::default(),
                                );
                            }

                            // Подсветка совпадения
                            job.append(
                                &line[found_start..found_end],
                                0.0,
                                TextFormat {
                                    background: ui.visuals().selection.bg_fill,
                                    font_id: FontId::monospace(14.0),
                                    ..Default::default()
                                },
                            );

                            cursor = found_end;
                        }
                        None => {
                            job.append(&line[cursor..], 0.0, TextFormat::default());
                            break;
                        }
                    }
                }

                job.append("\n", 0.0, TextFormat::default());
            }
        }

        ui.fonts(|f| f.layout_job(job))
    })
}

pub fn load_embedded_icon() -> Result<crate::egui::IconData, String> {
    let img = image::load_from_memory(super::ICON_BYTES).map_err(|e| e.to_string())?.into_rgba8();
    let (width, height) = img.dimensions();
    let rgba = img.into_raw();
    Ok(crate::egui::IconData { rgba, width, height })
}

pub fn spawn_watcher<T, F>(
    client: Arc<Client>,
    state: Arc<Mutex<Vec<T>>>,
    loading_flag: Arc<AtomicBool>,
    watch_fn: F,
) where
    T: Send + 'static,
    F: FnOnce(Arc<Client>, Arc<Mutex<Vec<T>>>, Arc<AtomicBool>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
{
    tokio::spawn(watch_fn(client, state, loading_flag));
}

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

// get yaml for namespaced resources
pub async fn get_yaml<T>(client: Arc<Client>, namespace: &str, name: &str, ) -> Result<String, kube::Error>
where
    T: Clone
        + Serialize
        + DeserializeOwned
        + std::fmt::Debug
        + Metadata<Ty = kube::core::ObjectMeta>
        + Resource<Scope = NamespaceResourceScope>
        + 'static,
{
    let api: Api<T> = Api::namespaced(client.as_ref().clone(), namespace);
    let obj = api.get(name).await?;
    Ok(serde_yaml::to_string(&obj).unwrap())
}

pub fn item_color(item: &str) -> Color32 {
    let ret_color = match item {
        "Ready" => Color32::GREEN,
        "NotReady" => Color32::RED,
        "Running" => Color32::GREEN,
        "Waiting" => Color32::YELLOW,
        "Terminated" => Color32::RED,
        "Complete" => Color32::GREEN,
        "Failed" => Color32::RED,
        "Bound" => Color32::GREEN,
        "Available" => Color32::LIGHT_GREEN,
        "Released" => Color32::GRAY,
        "Pending" => Color32::ORANGE,
        "SchedulingDisabled" => Color32::ORANGE,
        "Lost" => Color32::LIGHT_RED,
        "Active" => Color32::GREEN,
        "Terminating" => Color32::RED,
        "Warning" => Color32::ORANGE,
        "Normal" => Color32::GREEN,
        _ => Color32::LIGHT_GRAY,
    };

    return ret_color
}

pub async fn patch_resource(cl: Arc<Client>, yaml_str: &str) -> Result<(), anyhow::Error> {
    let client = cl.as_ref().clone();
    let mut value: serde_yaml::Value = serde_yaml::from_str(yaml_str)?;
    let meta: kube_discovery::kube::api::TypeMeta = serde_yaml::from_value(value.clone())?;

    // Удаляем metadata.managedFields, если оно есть
    if let Some(metadata) = value.get_mut("metadata") {
        if let Some(obj) = metadata.as_mapping_mut() {
            obj.remove(&serde_yaml::Value::String("managedFields".to_string()));
        }
    }

    let (group, version) = if meta.api_version.contains('/') {
        let mut parts = meta.api_version.splitn(2, '/');
        (parts.next().unwrap(), parts.next().unwrap())
    } else {
        ("", meta.api_version.as_str())
    };
    let gvk = kube::api::GroupVersionKind::gvk(group, version, &meta.kind);

    // Discovery из kube-discovery
    let discovery = discovery::Discovery::new(client.clone()).run().await?;
    let (ar, _caps) = discovery
        .resolve_gvk(&gvk)
        .ok_or_else(|| anyhow::anyhow!("GVK not found: {:?}", gvk))?;

    let namespace = value.get("metadata").and_then(|m| m.get("namespace")).and_then(|ns| ns.as_str());

    let api: Api<kube::api::DynamicObject> = if let Some(ns) = namespace {
        Api::namespaced_with(client, ns, &ar)
    } else {
        Api::all_with(client, &ar)
    };

    let patch = Patch::Apply(&value);
    let pp = PatchParams::apply("rustlens").force();

    let name = value.get("metadata").and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .ok_or_else(|| anyhow::anyhow!("metadata.name is required"))?;

    api.patch(name, &pp, &patch).await?;

    Ok(())
}

pub async fn apply_yaml(client: Arc<Client>, yaml: &str, resource_type: super::ResourceType) -> Result<(), anyhow::Error> {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;

    match resource_type {
        crate::ResourceType::Blank => {},
        crate::ResourceType::ExternalSecret => {
            use kube::{api::{Api, DynamicObject, GroupVersionKind}};
            let (ar, _caps) = discovery::pinned_kind(&client, &GroupVersionKind::gvk("apiextensions.k8s.io", "v1", "CustomResourceDefinition")).await.unwrap();
            let obj: kube::api::DynamicObject = serde_yaml::from_value(value.clone())?;
            let ns = obj.namespace().unwrap_or("default".into());
            let api: Api<DynamicObject> = Api::namespaced_with(client.as_ref().clone(), &ns, &ar);
            api.create(&PostParams::default(), &obj).await?;
        },
        crate::ResourceType::NameSpace => {
            let obj: Namespace = serde_yaml::from_value(value)?;
            let api: Api<Namespace> = Api::all(client.as_ref().clone());
            api.create(&PostParams::default(), &obj).await?;
        },
        crate::ResourceType::PersistenceVolumeClaim => {
            let obj: PersistentVolumeClaim = serde_yaml::from_value(value)?;
            let ns = obj.namespace().unwrap();
            let api: Api<PersistentVolumeClaim> = Api::namespaced(client.as_ref().clone(), &ns);
            api.create(&PostParams::default(), &obj).await?;
        },
        crate::ResourceType::Pod => {
            let obj: Pod = serde_yaml::from_value(value)?;
            let ns = obj.namespace().unwrap();
            let api: Api<Pod> = Api::namespaced(client.as_ref().clone(), &ns);
            api.create(&PostParams::default(), &obj).await?;
        },
        crate::ResourceType::Secret => {
            let obj: Secret = serde_yaml::from_value(value)?;
            let ns = obj.namespace().unwrap();
            let api: Api<Secret> = Api::namespaced(client.as_ref().clone(), &ns);
            api.create(&PostParams::default(), &obj).await?;
        }
    };

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

pub async fn cordon_node(client: Arc<Client>, node_name: &str, cordoned: bool) -> Result<(), kube::Error> {
    let nodes: Api<Node> = Api::all(client.as_ref().clone());
    let patch = json!({ "spec": { "unschedulable": cordoned } });
    nodes.patch(node_name, &PatchParams::apply("rustlens"), &Patch::Merge(&patch)).await?;
    Ok(())
}

pub async fn delete_pod(client: Arc<Client>, pod_name: &str, namespace: Option<&str>, force: bool) -> Result<(), kube::Error> {
    let ns = namespace.unwrap_or("default");
    let pods: Api<Pod> = Api::namespaced(client.as_ref().clone(), ns);
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

pub async fn delete_secret(client: Arc<Client>, secret_name: &str, namespace: Option<&str>) -> Result<(), kube::Error> {
    let ns = namespace.unwrap_or("default");
    let secrets: Api<Secret> = Api::namespaced(client.as_ref().clone(), ns);
    secrets.delete(secret_name, &DeleteParams::default()).await?;
    Ok(())
}

pub async fn delete_configmap(client: Arc<Client>, configmap_name: &str, namespace: Option<&str>) -> Result<(), kube::Error> {
    let ns = namespace.unwrap_or("default");
    let configmaps: Api<ConfigMap> = Api::namespaced(client.as_ref().clone(), ns);
    configmaps.delete(configmap_name, &DeleteParams::default()).await?;
    Ok(())
}

pub async fn drain_node(client: Arc<Client>, node_name: &str) -> anyhow::Result<()> {
    // Cordon node
    let nodes: Api<Node> = Api::all(client.as_ref().clone());
    let patch = json!({ "spec": { "unschedulable": true } });
    nodes.patch(node_name, &PatchParams::apply("rustlens"), &Patch::Merge(&patch)).await?;

    // Evict pods
    let pods: Api<Pod> = Api::all(client.as_ref().clone());
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

pub async fn watch_nodes(client: Arc<Client>, nodes_list: Arc<Mutex<Vec<super::NodeItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Node> = Api::all(client.as_ref().clone());
    let mut node_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;
    load_status.store(true, Ordering::Relaxed);

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
                    load_status.store(false, Ordering::Relaxed);
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

fn convert_crd(obj: &kube::api::DynamicObject) -> Option<super::CRDItem> {
    use serde_json::Value;

    let name = obj.metadata.name.as_ref().unwrap().clone();
    let spec: &Value = obj.data.get("spec")?;
    let group = spec.get("group")?.as_str()?.to_string();
    let scope = spec.get("scope")?.as_str()?.to_string();
    let creation_timestamp = obj.metadata.creation_timestamp.clone();

    let version = spec
        .get("versions")?
        .as_array()?
        .get(0)?
        .get("name")?
        .as_str()?
        .to_string();

    let kind = spec
        .get("names")?
        .get("kind")?
        .as_str()?
        .to_string();

    Some(super::CRDItem {
        name,
        group,
        version,
        scope,
        kind,
        creation_timestamp,
    })
}

pub async fn watch_crds(client: Arc<Client>, list: Arc<Mutex<Vec<super::CRDItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{api::{Api, DynamicObject, GroupVersionKind}, runtime::watcher::{self, Event}};
    use kube::discovery;

    let (ar, _caps) = discovery::pinned_kind(&client, &GroupVersionKind::gvk("apiextensions.k8s.io", "v1", "CustomResourceDefinition")).await.unwrap();
    let api: Api<DynamicObject> = Api::all_with(client.as_ref().clone(), &ar);

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(obj) => {
                    if let Some(item) = convert_crd(&obj) {
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
                    if let Some(item) = convert_crd(&obj) {
                        let mut list_guard = list.lock().unwrap();
                        list_guard.push(item);
                    }
                }
                Event::Delete(obj) => {
                    if let Some(item) = obj.metadata.name {
                        let mut obj_vec = list.lock().unwrap();
                        obj_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("CRDs watch error: {:?}", e),
        }
    }
}

pub fn convert_network_policy(policy: NetworkPolicy) -> Option<super::NetworkPolicyItem> {
    let metadata = &policy.metadata;
    let name = metadata.name.clone()?;

    let pod_selector = match &policy.spec {
        Some(spec) => {
            let labels = &spec.pod_selector.match_labels;
            if let Some(lbls) = labels {
                lbls.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                "None".to_string()
            }
        }
        None => "None".to_string(),
    };

    let policy_types = policy
        .spec
        .as_ref()
        .map(|spec| spec.policy_types.as_ref().unwrap().join(", "))
        .unwrap_or_else(|| "None".to_string());

    let creation_timestamp = metadata.creation_timestamp.clone();

    Some(super::NetworkPolicyItem {
        name,
        pod_selector,
        policy_types,
        creation_timestamp,
        namespace: policy.metadata.namespace.clone(),
    })
}

pub async fn watch_network_policies(client: Arc<Client>, list: Arc<Mutex<Vec<super::NetworkPolicyItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<NetworkPolicy> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_network_policy).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(policy) => {
                    if let Some(item) = convert_network_policy(policy) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list_guard = list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(policy) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_network_policy(policy) {
                        let mut list = list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(policy) => {
                    if let Some(item) = policy.metadata.name {
                        let mut policy_vec = list.lock().unwrap();
                        policy_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("NetworkPolicy watch error: {:?}", e),
        }
    }
}

pub fn convert_pdb(pdb: PodDisruptionBudget) -> Option<super::PodDisruptionBudgetItem> {
    use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;

    let metadata = &pdb.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();

    let spec = pdb.spec?;
    let status = pdb.status?;

    let namespace = pdb.metadata.namespace.clone();

    Some(super::PodDisruptionBudgetItem {
        name,
        min_available: spec.min_available.map(|v| match v {
            IntOrString::Int(i) => i.to_string(),
            IntOrString::String(s) => s,
        }),
        max_unavailable: spec.max_unavailable.map(|v| match v {
            IntOrString::Int(i) => i.to_string(),
            IntOrString::String(s) => s,
        }),
        allowed_disruptions: status.disruptions_allowed,
        current_healthy: status.current_healthy,
        desired_healthy: status.desired_healthy,
        creation_timestamp,
        namespace,
    })
}

pub async fn watch_pod_disruption_budgets(client: Arc<Client>, list: Arc<Mutex<Vec<super::PodDisruptionBudgetItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<PodDisruptionBudget> = Api::all(client.as_ref().clone());
    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(pdb) => {
                    if let Some(item) = convert_pdb(pdb) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list_guard = list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(pdb) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_pdb(pdb) {
                        let mut list = list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(pdb) => {
                    if let Some(item) = pdb.metadata.name {
                        let mut pdb_vec = list.lock().unwrap();
                        pdb_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("PDB watch error: {:?}", e),
        }
    }
}

pub fn convert_daemonset(ds: DaemonSet) -> Option<super::DaemonSetItem> {
    let metadata = &ds.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();

    let status = ds.status?;
    Some(super::DaemonSetItem {
        name,
        desired: status.desired_number_scheduled,
        current: status.current_number_scheduled,
        ready: status.number_ready,
        creation_timestamp,
        namespace: ds.metadata.namespace.clone(),
    })
}

pub async fn watch_daemonsets(client: Arc<Client>, daemonsets_list: Arc<Mutex<Vec<super::DaemonSetItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<DaemonSet> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = daemonsets_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_daemonset).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(ds) => {
                    if let Some(item) = convert_daemonset(ds) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list_guard = daemonsets_list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(ds) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_daemonset(ds) {
                        let mut list = daemonsets_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(ds) => {
                    if let Some(item) = ds.metadata.name {
                        let mut ds_vec = daemonsets_list.lock().unwrap();
                        ds_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("DaemonSet watch error: {:?}", e),
        }
    }
}

pub fn convert_cronjob(cj: CronJob) -> Option<super::CronJobItem> {
    let metadata = &cj.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp =  metadata.creation_timestamp.clone();
    let namespace = cj.metadata.namespace.clone();
    let spec = cj.spec?;
    let schedule = spec.schedule;
    let suspend = spec.suspend.unwrap_or(false);

    let active = cj.status
        .as_ref()
        .and_then(|s| s.active.as_ref()
        .map(|a| a.len())).unwrap_or(0);

    let last_schedule = cj.status
        .as_ref()
        .and_then(|s| s.last_schedule_time.as_ref())
        .map(|t| t.0.to_rfc3339())
        .unwrap_or_else(|| "-".to_string());

    Some(super::CronJobItem {
        name,
        schedule,
        suspend: if suspend { "true".into() } else { "false".into() },
        active,
        last_schedule,
        creation_timestamp,
        namespace,
    })
}

pub async fn watch_cronjobs(client: Arc<Client>, cronjob_list: Arc<Mutex<Vec<super::CronJobItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<CronJob> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = cronjob_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_cronjob).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(cronjob) => {
                    if let Some(item) = convert_cronjob(cronjob) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list_guard = cronjob_list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(cronjob) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_cronjob(cronjob) {
                        let mut list = cronjob_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(cronjob) => {
                    if let Some(item) = cronjob.metadata.name {
                        let mut cronjobs_vec = cronjob_list.lock().unwrap();
                        cronjobs_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("CronJob watch error: {:?}", e),
        }
    }
}


pub fn convert_ingress(ing: Ingress) -> Option<super::IngressItem> {
    let metadata = &ing.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();
    let ing_spec = ing.spec.clone();

    let mut hosts = vec![];
    let mut paths = vec![];
    let mut services = vec![];

    if let Some(spec) = ing.spec {
        if let Some(rules) = spec.rules {
            for rule in rules {
                if let Some(host) = rule.host {
                    hosts.push(host.clone());
                }
                if let Some(http) = rule.http {
                    for path in http.paths {
                        let p = path.path.unwrap_or_else(|| "/".to_string());
                        paths.push(p.clone());

                        if let Some(backend) = path.backend.service {
                            services.push(backend.name);
                        }
                    }
                }
            }
        }
    }

    let tls = ing_spec
        .as_ref()
        .and_then(|s| s.tls.as_ref())
        .map(|tls| {
            tls.iter()
                .filter_map(|entry| entry.hosts.clone())
                .flatten()
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "-".to_string());

    Some(super::IngressItem {
        name,
        host: if hosts.is_empty() { "-".into() } else { hosts.join(", ") },
        paths: if paths.is_empty() { "-".into() } else { paths.join(", ") },
        service: if services.is_empty() { "-".into() } else { services.join(", ") },
        tls,
        creation_timestamp,
        namespace: ing.metadata.namespace.clone(),
    })
}

pub async fn watch_ingresses(client: Arc<Client>, ingresses_list: Arc<Mutex<Vec<super::IngressItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<Ingress> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = ingresses_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_ingress).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(ing) => {
                    if let Some(item) = convert_ingress(ing) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = ingresses_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(ing) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_ingress(ing) {
                        let mut list = ingresses_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(ing) => {
                    if let Some(item) = ing.metadata.name {
                        let mut ings_vec = ingresses_list.lock().unwrap();
                        ings_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("Ingress watch error: {:?}", e),
        }
    }
}

pub fn convert_endpoint(ep: Endpoints) -> Option<super::EndpointItem> {
    let metadata = &ep.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();

    let mut all_addresses = Vec::new();
    let mut all_ports = Vec::new();

    if let Some(subsets) = ep.subsets {
        for subset in subsets {
            if let Some(addresses) = subset.addresses {
                for addr in addresses {
                    all_addresses.push(addr.ip);
                }
            }

            if let Some(ports) = subset.ports {
                for port in ports {
                    let port_str = format!(
                        "{}:{}",
                        port.name.unwrap_or_else(|| "-".to_string()),
                        port.port
                    );
                    all_ports.push(port_str);
                }
            }
        }
    }

    Some(super::EndpointItem {
        name,
        addresses: if all_addresses.is_empty() {
            "-".into()
        } else {
            all_addresses.join(", ")
        },
        ports: if all_ports.is_empty() {
            "-".into()
        } else {
            all_ports.join(", ")
        },
        creation_timestamp,
        namespace: ep.metadata.namespace.clone(),
    })
}

pub async fn watch_endpoints(client: Arc<Client>, endpoints_list: Arc<Mutex<Vec<super::EndpointItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<Endpoints> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = endpoints_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_endpoint).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(ep) => {
                    if let Some(item) = convert_endpoint(ep) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = endpoints_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(ep) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_endpoint(ep) {
                        let mut list = endpoints_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(ep) => {
                    if let Some(item) = ep.metadata.name {
                        let mut eps_vec = endpoints_list.lock().unwrap();
                        eps_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("Endpoint watch error: {:?}", e),
        }
    }
}

pub fn convert_service(svc: Service) -> Option<super::ServiceItem> {
    use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;

    let metadata = &svc.metadata;
    let spec = svc.spec.as_ref()?;

    let name = metadata.name.clone()?;
    let svc_type = spec.type_.clone().unwrap_or_else(|| "ClusterIP".to_string());
    let cluster_ip = spec.cluster_ip.clone().unwrap_or_else(|| "None".to_string());

    let ports = spec
        .ports
        .as_ref()
        .map(|ports| {
            ports
                .iter()
                .map(|p| {
                    let port = p.port;
                    let target_port = p.target_port.as_ref().map_or("".to_string(), |tp| match tp {
                        IntOrString::Int(i) => i.to_string(),
                        IntOrString::String(s) => s.to_string(),
                    });
                    let protocol = p.protocol.as_ref().map_or("TCP".to_string(), |s| s.clone());
                    format!("{}/{}→{}", port, protocol, target_port)
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "-".to_string());

    let external_ip = if let Some(eips) = &spec.external_ips {
        eips.join(", ")
    } else if let Some(lb) = &svc.status.and_then(|s| s.load_balancer) {
        if let Some(ing) = &lb.ingress {
            ing.iter()
                .map(|i| {
                    i.ip.clone().or_else(|| i.hostname.clone()).unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            "None".to_string()
        }
    } else {
        "None".to_string()
    };

    let selector = spec
        .selector
        .as_ref()
        .map(|s| {
            s.iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "-".to_string());

    Some(super::ServiceItem {
        name,
        svc_type,
        cluster_ip,
        ports,
        external_ip,
        selector,
        creation_timestamp: svc.metadata.creation_timestamp,
        status: "OK".to_string(),
        namespace: svc.metadata.namespace.clone(),
    })
}

pub async fn watch_services(client: Arc<Client>, services_list: Arc<Mutex<Vec<super::ServiceItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<Service> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = services_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_service).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(svc) => {
                    if let Some(item) = convert_service(svc) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = services_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(svc) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_service(svc) {
                        let mut list = services_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(svc) => {
                    if let Some(item) = svc.metadata.name {
                        let mut svcs_vec = services_list.lock().unwrap();
                        svcs_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("Service watch error: {:?}", e),
        }
    }
}

pub fn convert_csi_driver(driver: CSIDriver) -> Option<super::CSIDriverItem> {
    Some(super::CSIDriverItem {
        name: driver.metadata.name.clone()?,
        attach_required: driver
            .spec
            .attach_required
            .map_or("Unknown".to_string(), |b| if b { "Yes" } else { "No" }.to_string()),
        pod_info_on_mount: driver
            .spec
            .pod_info_on_mount
            .map_or("Unknown".to_string(), |b| if b { "Yes" } else { "No" }.to_string()),
        storage_capacity: driver
            .spec
            .storage_capacity
            .map_or("Unknown".to_string(), |b| if b { "Yes" } else { "No" }.to_string()),
        fs_group_policy: driver
            .spec
            .fs_group_policy
            .as_ref()
            .map_or("Unknown".to_string(), |s| s.clone()),
        creation_timestamp: driver.metadata.creation_timestamp,
    })
}

pub async fn watch_csi_drivers(client: Arc<Client>, csi_list: Arc<Mutex<Vec<super::CSIDriverItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<CSIDriver> = Api::all(client.as_ref().clone());
    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(driver) => {
                    if let Some(item) = convert_csi_driver(driver) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = csi_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(driver) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_csi_driver(driver) {
                        let mut list = csi_list.lock().unwrap();
                        list.push(item);
                    }
                }
                Event::Delete(driver) => {
                    if let Some(item) = driver.metadata.name {
                        let mut drivers_vec = csi_list.lock().unwrap();
                        drivers_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("CSIDriver watch error: {:?}", e),
        }
    }
}

pub fn convert_event(ev: Event) -> Option<super::EventItem> {
    let involved_object = format!(
        "{}/{}",
        ev.involved_object.kind.clone().unwrap_or_else(|| "Unknown".to_string()),
        ev.involved_object.name.clone().unwrap_or_else(|| "Unknown".to_string())
    );
    Some(super::EventItem {
        creation_timestamp: ev.metadata.creation_timestamp,
        message: ev.message.clone().unwrap(),
        reason: ev.reason.clone().unwrap_or_else(|| "Unknown".to_string()),
        involved_object,
        event_type: ev.type_.clone().unwrap_or_else(|| "Normal".to_string()),
        timestamp: ev.event_time.as_ref().map(|t| t.0.to_rfc3339()).or_else(|| ev.last_timestamp.as_ref().map(|t| t.0.to_rfc3339()))
            .unwrap_or_else(|| "N/A".to_string()),
        namespace: ev.involved_object.namespace.clone().unwrap_or_else(|| "default".to_string()),
    })
}

pub async fn watch_events(client: Arc<Client>, events_list: Arc<Mutex<Vec<super::EventItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Event> = Api::all(client.as_ref().clone());
    let mut event_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(ev) => {
                    if let Some(item) = convert_event(ev) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = events_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(ev) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_event(ev) {
                        let mut list = events_list.lock().unwrap();
                        list.push(item);
                    }
                }
                watcher::Event::Delete(_) => {} // Events should not be deleted
            },
            Err(e) => {
                eprintln!("Event watch error: {:?}", e);
            }
        }
    }
}

pub fn convert_namespace(ns: Namespace) -> Option<super::NamespaceItem> {
    Some(super::NamespaceItem {
        creation_timestamp: ns.metadata.creation_timestamp,
        phase: ns.status.as_ref().and_then(|s| s.phase.clone()),
        labels: ns.metadata.labels.clone(),
        name: ns.metadata.name.unwrap(),
    })
}

pub async fn watch_namespaces(client: Arc<Client>, ns_list: Arc<Mutex<Vec<super::NamespaceItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Namespace> = Api::all(client.as_ref().clone());
    let mut ns_stream = watcher(api, watcher::Config::default()).boxed();

    load_status.store(true, Ordering::Relaxed);

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = ns_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(ns) => {
                    if let Some(item) = convert_namespace(ns) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut ns_vec = ns_list.lock().unwrap();
                    *ns_vec = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(ns) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_namespace(ns) {
                        let mut list = ns_list.lock().unwrap();
                        list.push(item);
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

pub fn convert_pv(pv: PersistentVolume) -> Option<super::PvItem> {
    Some(super::PvItem {
        name: pv.metadata.name.clone()?,
        labels: pv.metadata.labels.clone().unwrap_or_default(),
        storage_class: pv
            .spec
            .as_ref()
            .and_then(|s| s.storage_class_name.clone())
            .unwrap_or_else(|| "-".to_string()),
        capacity: pv
            .spec
            .as_ref()
            .and_then(|s| s.capacity.as_ref())
            .and_then(|cap| cap.get("storage"))
            .map(|q| q.0.to_string())
            .unwrap_or_else(|| "-".to_string()),
        claim: pv
            .spec
            .as_ref()
            .and_then(|s| s.claim_ref.as_ref())
            .map(|c| format!("{}/{}", c.namespace.clone().unwrap_or_default(), c.name.clone().unwrap_or_default()))
            .unwrap_or_else(|| "-".to_string()),
        status: pv
            .status
            .as_ref()
            .and_then(|s| s.phase.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
        creation_timestamp: pv.metadata.creation_timestamp,
    })
}

pub async fn watch_pvs(client: Arc<Client>, pv_list: Arc<Mutex<Vec<super::PvItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<PersistentVolume> = Api::all(client.as_ref().clone());
    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(pv) => {
                    if let Some(item) = convert_pv(pv) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = pv_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(pv) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_pv(pv) {
                        let mut list = pv_list.lock().unwrap();
                        list.push(item);
                    }
                }
                Event::Delete(pv) => {
                    if let Some(item) = pv.metadata.name {
                        let mut pv_vec = pv_list.lock().unwrap();
                        pv_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("PV watch error: {:?}", e),
        }
    }
}

pub fn convert_storage_class(sc: StorageClass) -> Option<super::StorageClassItem> {
    Some(super::StorageClassItem {
        name: sc.metadata.name.clone()?,
        labels: sc.metadata.labels.clone().unwrap_or_default(),
        provisioner: sc.provisioner.clone(),
        reclaim_policy: sc
            .reclaim_policy
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        volume_binding_mode: sc
            .volume_binding_mode
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        is_default: match sc.metadata.annotations {
            Some(ann) => {
                if let Some(val) = ann.get("storageclass.kubernetes.io/is-default-class") {
                    if val == "true" {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    }
                } else {
                    "no".to_string()
                }
            }
            None => "no".to_string(),
        },
        creation_timestamp: sc.metadata.creation_timestamp,
    })
}

pub async fn watch_storage_classes(client: Arc<Client>, sc_list: Arc<Mutex<Vec<super::StorageClassItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<StorageClass> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(sc) => {
                    if let Some(item) = convert_storage_class(sc) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = sc_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(sc) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_storage_class(sc) {
                        let mut list = sc_list.lock().unwrap();
                        list.push(item);
                    }
                }
                Event::Delete(_) => {}
            },
            Err(e) => eprintln!("StorageClass watch error: {:?}", e),
        }
    }
}

pub fn convert_pvc(pvc: PersistentVolumeClaim) -> Option<super::PvcItem> {
    Some(super::PvcItem {
        name: pvc.metadata.name.clone()?,
        labels: pvc.metadata.labels.clone().unwrap_or_default(),
        storage_class: pvc
            .spec
            .as_ref()
            .and_then(|s| s.storage_class_name.clone())
            .unwrap_or_else(|| "-".to_string()),
        size: pvc.spec.as_ref()
            .and_then(|s| s.resources.as_ref().unwrap().requests.as_ref())
            .and_then(|r| r.get("storage")).map(|q| q.0.to_string()).unwrap_or_else(|| "".to_string()),
        volume_name: pvc
            .spec
            .as_ref()
            .and_then(|s| s.volume_name.clone())
            .unwrap_or_else(|| "-".to_string()),
        status: pvc
            .status
            .as_ref()
            .and_then(|s| s.phase.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
        creation_timestamp: pvc.metadata.creation_timestamp,
        namespace: pvc.metadata.namespace.clone(),
    })
}

pub async fn watch_pvcs(client: Arc<Client>, pvc_list: Arc<Mutex<Vec<super::PvcItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<PersistentVolumeClaim> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = pvc_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_pvc).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(pvc) => {
                    if let Some(item) = convert_pvc(pvc) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = pvc_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(pvc) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_pvc(pvc) {
                        let mut list = pvc_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(_) => {}
            },
            Err(e) => eprintln!("PVC watch error: {:?}", e),
        }
    }
}


pub fn convert_replicaset(rs: ReplicaSet) -> Option<super::ReplicaSetItem> {
    Some(super::ReplicaSetItem {
        name: rs.metadata.name.clone()?,
        labels: rs.metadata.labels.unwrap_or_default(),
        desired: rs.spec.as_ref()?.replicas.unwrap_or(0),
        current: rs.status.as_ref()?.replicas,
        ready: rs.status.as_ref()?.ready_replicas.unwrap_or(0),
        creation_timestamp: rs.metadata.creation_timestamp,
        namespace: rs.metadata.namespace.clone()
    })
}

pub async fn watch_replicasets(client: Arc<Client>, rs_list: Arc<Mutex<Vec<super::ReplicaSetItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<ReplicaSet> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = rs_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_replicaset).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(rs) => {
                    if let Some(item) = convert_replicaset(rs) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = rs_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(rs) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_replicaset(rs) {
                        let mut list = rs_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(rs) => {
                    if let Some(item) = rs.metadata.name {
                        let mut rs_vec = rs_list.lock().unwrap();
                        rs_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("ReplicaSet watch error: {:?}", e),
        }
    }
}

pub fn convert_statefulset(ss: StatefulSet) -> Option<super::StatefulSetItem> {
    let spec = ss.spec.unwrap();
    let namespace = ss.metadata.namespace.clone();
    Some(super::StatefulSetItem {
        name: ss.metadata.name.clone()?,
        labels: ss.metadata.labels.unwrap_or_default(),
        service_name: spec.service_name.unwrap_or("-".to_string()),
        replicas: spec.replicas.unwrap_or(0),
        ready_replicas: ss.status.as_ref()?.ready_replicas.unwrap_or(0),
        creation_timestamp: ss.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_statefulsets(client: Arc<Client>, ss_list: Arc<Mutex<Vec<super::StatefulSetItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<StatefulSet> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = ss_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_statefulset).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(ss) => {
                    if let Some(item) = convert_statefulset(ss) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = ss_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(ss) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_statefulset(ss) {
                        let mut list = ss_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(ss) => {
                    if let Some(item) = ss.metadata.name {
                        let mut ss_vec = ss_list.lock().unwrap();
                        ss_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("StatefulSet watch error: {:?}", e),
        }
    }
}

pub fn convert_job(job: Job) -> Option<super::JobItem> {
    let condition = job
        .status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .map(|conds| {
            if conds.iter().any(|c| c.type_ == "Complete" && c.status == "True") {
                "Complete".to_string()
            } else if conds.iter().any(|c| c.type_ == "Failed" && c.status == "True") {
                "Failed".to_string()
            } else {
                "Running".to_string()
            }
        })
        .unwrap_or_else(|| "Unknown".to_string());

    let namespace = job.metadata.namespace.clone();

    Some(super::JobItem {
        name: job.metadata.name.clone()?,
        labels: job.metadata.labels.unwrap_or_default(),
        completions: job
            .status
            .as_ref()
            .and_then(|s| s.succeeded)
            .unwrap_or(0),
        condition,
        creation_timestamp: job.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_jobs(client: Arc<Client>, jobs_list: Arc<Mutex<Vec<super::JobItem>>>, load_status: Arc<AtomicBool>) {
    use kube::{Api, runtime::watcher, runtime::watcher::Event};
    let api: Api<Job> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = jobs_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_job).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();
    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(job) => {
                    if let Some(item) = convert_job(job) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = jobs_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(job) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_job(job) {
                        let mut list = jobs_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(job) => {
                    if let Some(item) = job.metadata.name {
                        let mut job_vec = jobs_list.lock().unwrap();
                        job_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("Job watch error: {:?}", e),
        }
    }
}

fn convert_deployment(deploy: Deployment) -> Option<super::DeploymentItem> {
    let name = deploy.metadata.name.unwrap_or_default();
    let status = deploy.status.unwrap_or_default();
    let namespace = deploy.metadata.namespace.clone();
    Some(super::DeploymentItem {
        name,
        namespace,
        ready_replicas: status.ready_replicas.unwrap_or(0),
        available_replicas: status.available_replicas.unwrap_or(0),
        updated_replicas: status.updated_replicas.unwrap_or(0),
        replicas: status.replicas.unwrap_or(0),
        creation_timestamp: deploy.metadata.creation_timestamp,
    })
}

pub async fn watch_deployments(client: Arc<Client>, deployments_list: Arc<Mutex<Vec<super::DeploymentItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Deployment> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = deployments_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_deployment).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(watch_event) => match watch_event {
                WatcherEvent::Init => initial.clear(),

                WatcherEvent::InitApply(deploy) => {
                    if let Some(item) = convert_deployment(deploy) {
                        initial.push(item);
                    }
                }

                WatcherEvent::InitDone => {
                    let mut list = deployments_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }

                WatcherEvent::Apply(deploy) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_deployment(deploy) {
                        let mut list = deployments_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }

                WatcherEvent::Delete(deploy) => {
                    if let Some(item) = deploy.metadata.name {
                        let mut deploy_vec = deployments_list.lock().unwrap();
                        deploy_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => {
                eprintln!("Deployment watch error: {:?}", e);
            }
        }
    }
}

pub fn convert_configmap(cm: ConfigMap) -> Option<super::ConfigMapItem> {
    Some(super::ConfigMapItem {
        name: cm.metadata.name.clone()?,
        labels: cm.metadata.labels.unwrap_or_default(),
        keys: cm.data.as_ref().map(|d| d.keys().cloned().collect()).unwrap_or_default(),
        type_: "Opaque".to_string(),
        creation_timestamp: cm.metadata.creation_timestamp,
        namespace: cm.metadata.namespace.clone(),
    })
}

pub async fn watch_configmaps(client: Arc<Client>, configmaps_list: Arc<Mutex<Vec<super::ConfigMapItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<ConfigMap> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = configmaps_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_configmap).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

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

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(cm) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_configmap(cm) {
                        let mut list = configmaps_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                watcher::Event::Delete(cm) => {
                    if let Some(item) = cm.metadata.name {
                        let mut cm_vec = configmaps_list.lock().unwrap();
                        cm_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("ConfigMap watch error: {:?}", e),
        }
    }
}

fn convert_secret(secret: Secret) -> Option<super::SecretItem> {
    let name = secret.name().unwrap().to_string();
    let namespace = secret.metadata.namespace.clone();
    Some(super::SecretItem {
        name,
        labels: secret.metadata.labels.unwrap_or_default().into_iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(", "),
        keys: secret.data.as_ref().map(|d| d.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_else(|| "-".into()),
        type_: secret.type_.unwrap_or_else(|| "-".into()),
        creation_timestamp: secret.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_secrets(client: Arc<Client>, secrets_list: Arc<Mutex<Vec<super::SecretItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Secret> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = secrets_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_secret).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

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

                    load_status.store(false, Ordering::Relaxed);
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
                watcher::Event::Delete(secret) => {
                    if let Some(item) = secret.metadata.name {
                        let mut secrets_vec = secrets_list.lock().unwrap();
                        secrets_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("Secret watch error: {:?}", e),
        }
    }
}

fn convert_pod(pod: Pod) -> Option<super::PodItem> {
    let name = pod.metadata.name.unwrap();
    let phase = pod.status.as_ref().and_then(|s| s.phase.clone());
    let node_name = pod.spec.as_ref().and_then(|s| s.node_name.clone());
    let terminating = pod.metadata.deletion_timestamp.is_some();
    let namespace = pod.metadata.namespace.clone();
    let mut containers = vec![];
    let mut ready = 0;
    let mut restart_count = 0;
    let mut pod_has_crashloop = false;

    let controller = pod.metadata.owner_references.as_ref()
        .and_then(|owners| {
            owners.iter()
                .find(|o| o.controller.unwrap_or(false))
                .map(|o| format!("{}", o.kind)) // can be name, uid etc...
        });

    if let Some(statuses) = pod.status.as_ref().and_then(|s| s.container_statuses.clone()) {
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
    }
    Some(super::PodItem {
        name,
        phase,
        ready_containers: ready,
        total_containers: containers.len() as u32,
        containers,
        restart_count,
        node_name,
        pod_has_crashloop,
        creation_timestamp: pod.metadata.creation_timestamp,
        terminating,
        controller,
        namespace,
    })
}

pub async fn watch_pods(client: Arc<Client>, pods_list: Arc<Mutex<Vec<super::PodItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Pod> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = pods_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_pod).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(pod) => {
                    if let Some(item) = convert_pod(pod) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = pods_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;
                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(pod) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_pod(pod) {
                        let mut list = pods_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|p| p.name == item.name) {
                            *existing = item;
                        } else {
                            list.push(item);
                        }
                    }
                }
                watcher::Event::Delete(pod) => {
                    if let Some(item) = pod.metadata.name {
                        let mut pods_vec = pods_list.lock().unwrap();
                        pods_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("Secret watch error: {:?}", e),
        }
    }
}

pub async fn fetch_logs(client: Arc<Client>, namespace: &str, pod_name: &str, container_name: &str, buffer: Arc<Mutex<String>>) {
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
