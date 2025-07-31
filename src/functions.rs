use eframe::egui::Color32;
use futures::{AsyncBufReadExt};
use k8s_openapi::{ClusterResourceScope, Metadata, NamespaceResourceScope, Resource};
use kube::runtime::reflector::Lookup;
use kube::{Api, Client, Config};
use kube::config::{Kubeconfig, NamedContext};
use kube::discovery;
use k8s_openapi::api::core::v1::{Namespace, Node, Pod, Secret, ConfigMap, PersistentVolumeClaim};
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet, ReplicaSet};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use futures_util::StreamExt;
use serde_json::json;
use kube::api::{DeleteParams, ListParams, LogParams, Patch, PatchParams, PostParams, PropagationPolicy};
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
    let total_days = duration.num_days();
    if total_days >= 365 {
        let years = total_days / 365;
        let remaining_days = total_days % 365;
        if remaining_days > 0 {
            format!("{}y {}d", years, remaining_days)
        } else {
            format!("{}y", years)
        }
    } else if total_days > 0 {
        format!("{}d", total_days)
    } else if duration.num_hours() > 0 {
        format!("{}h", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{}m", duration.num_minutes())
    } else {
        format!("{}s", duration.num_seconds())
    }
}

// get yaml for namespaced resources
pub async fn get_yaml_namespaced<T>(client: Arc<Client>, namespace: &str, name: &str, ) -> Result<String, kube::Error>
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

pub async fn get_yaml_global<T>(client: Arc<Client>, name: &str, ) -> Result<String, kube::Error>
where
    T: Clone
        + Serialize
        + DeserializeOwned
        + std::fmt::Debug
        + Metadata<Ty = kube::core::ObjectMeta>
        + Resource<Scope = ClusterResourceScope>
        + 'static,
{
    let api: Api<T> = Api::all(client.as_ref().clone());
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
        "Succeeded" => Color32::GREEN,
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

    // Remove metadata.managedFields, if exists
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

    // a lkube-discovery
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

#[derive(Debug, Clone)]
pub enum ScaleTarget {
    Deployment,
    StatefulSet,
    ReplicaSet,
}

pub async fn scale_workload(client: Arc<Client>, name: &str, namespace: &str, replicas: i32, kind: ScaleTarget) -> Result<(), kube::Error> {
    let patch = serde_json::json!({
            "spec": {
                "replicas": replicas
            }
    });

    match kind {
        ScaleTarget::Deployment => {
            let api: Api<Deployment> = Api::namespaced(client.as_ref().clone(), namespace);
            api.patch(name, &PatchParams::apply("scaler"), &Patch::Merge(&patch)).await?;
        }
        ScaleTarget::StatefulSet => {
            let api: Api<StatefulSet> = Api::namespaced(client.as_ref().clone(), namespace);
            api.patch(name, &PatchParams::apply("scaler"), &Patch::Merge(&patch)).await?;
        }
        ScaleTarget::ReplicaSet => {
            let api: Api<ReplicaSet> = Api::namespaced(client.as_ref().clone(), namespace);
            api.patch(name, &PatchParams::apply("scaler"), &Patch::Merge(&patch)).await?;
        }
    }
    Ok(())
}

pub async fn cordon_node(client: Arc<Client>, node_name: &str, cordoned: bool) -> Result<(), kube::Error> {
    let nodes: Api<Node> = Api::all(client.as_ref().clone());
    let patch = json!({ "spec": { "unschedulable": cordoned } });
    nodes.patch(node_name, &PatchParams::apply("rustlens"), &Patch::Merge(&patch)).await?;
    Ok(())
}

pub async fn delete_node(client: Arc<Client>, node_name: &str) -> Result<(), kube::Error> {
    // or maybe pass the list of nodes, and not to get them again?
    let nodes: Api<Node> = Api::all(client.as_ref().clone());

    // delete the node
    nodes.delete(node_name, &Default::default()).await?;
    eprintln!("Node {} deletion requested", node_name);
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

#[derive(Debug, Clone)]
pub struct NodeDetails {
    pub name: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
    pub taints: Vec<String>,
    pub kubelet_version: Option<String>,
    pub addresses: BTreeMap<String, String>,
    pub os: Option<String>,
    pub os_image: Option<String>,
    pub kernel_version: Option<String>,
    pub container_runtime: Option<String>,
}

impl NodeDetails {
    pub fn new() -> Self {
        Self {
            name: None,
            kubelet_version: None,
            taints: vec![],
            labels: None,
            annotations: None,
            addresses: BTreeMap::new(),
            os: None,
            os_image: None,
            kernel_version: None,
            container_runtime: None,
        }
    }
}

pub async fn get_node_details(client: Arc<Client>, name: &str, details: Arc<Mutex<NodeDetails>>) -> Result<(), kube::Error> {
    let api: Api<Node> = Api::all(client.as_ref().clone());
    let node = api.get(name).await.unwrap();
    let mut details_items = details.lock().unwrap();

    let metadata = node.metadata.clone();
    details_items.name = metadata.name;
    details_items.labels = metadata.labels;
    details_items.annotations = metadata.annotations;

    if let Some(addrs) = node.status.clone().unwrap().addresses {
        for addr in addrs {
            details_items.addresses.insert(addr.type_, addr.address);
        }
    }

    let spec = node.spec.clone();
    details_items.taints = spec.and_then(|s| s.taints).unwrap_or_default()
        .into_iter()
        .map(|t| format!("{}={}:{}", t.key, t.value.unwrap_or_default(), t.effect))
        .collect::<Vec<_>>();


    details_items.kubelet_version = node.status.clone()
        .as_ref()
        .and_then(|s| s.node_info.as_ref())
        .map(|info| info.kubelet_version.clone());

    let status = node.status.ok_or_else(|| anyhow::anyhow!("Missing node status")).unwrap();
    let node_info =  status.node_info.ok_or_else(|| anyhow::anyhow!("Missing nodeInfo")).unwrap();

    details_items.os = Some(format!("{} ({})", node_info.operating_system, node_info.architecture));
    details_items.os_image = Some(node_info.os_image);
    details_items.kernel_version = Some(node_info.kernel_version);
    details_items.container_runtime = Some(node_info.container_runtime_version);

    Ok(())
}

pub async fn delete_namespace(client: Arc<Client>, name: &str) -> Result<(), kube::Error> {
    let namespaces: Api<Namespace> = Api::all(client.as_ref().clone());
    namespaces.delete(name, &DeleteParams::default()).await?;
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

#[derive(Debug, Clone)]
pub struct HelmReleaseItem {
    pub name: String,
    pub chart_name: Option<String>,
    pub version:  Option<String>,
    pub namespace: Option<String>,
    pub creation_timestamp: Option<Time>,
}

pub async fn get_helm_releases(client: Arc<Client>, list: Arc<Mutex<Vec<HelmReleaseItem>>>, load_status: Arc<AtomicBool>) -> Result<(), anyhow::Error> {
    let namespaces: Api<k8s_openapi::api::core::v1::Namespace> = Api::all(client.as_ref().clone());
    let ns_list = namespaces.list(&ListParams::default()).await?;
    load_status.store(true, Ordering::Relaxed);

    let mut result = vec![];

    for ns in ns_list {
        if let Some(ns_name) = ns.metadata.name {
            let secrets: Api<Secret> = Api::namespaced(client.as_ref().clone(), &ns_name);

            let lp = ListParams::default().labels("owner=helm").fields("type=helm.sh/release.v1");
            let secret_list = secrets.list(&lp).await.unwrap();

            for s in secret_list {
                if let Some(name) = s.metadata.name {
                    let unzipped = Vec::new();
                    let as_str = String::from_utf8_lossy(&unzipped);
                    let chart_line = as_str.lines().find(|l| l.contains("chart"));
                    let chart = chart_line
                        .and_then(|line| line.split('"').nth(1))
                        .map(|s| s.to_string());

                    let chart_name = chart
                        .as_ref()
                        .and_then(|c| c.split('-').next())
                        .map(|s| s.to_string());

                    let version = chart
                        .as_ref()
                        .and_then(|c| c.split('-').last())
                        .map(|s| s.to_string());

                    let created_at = s.metadata.creation_timestamp;

                    result.push(HelmReleaseItem {
                        name,
                        chart_name,
                        version,
                        namespace: Some(ns_name.clone()),
                        creation_timestamp: created_at,
                    });
                }
            }
        }
    }
    load_status.store(false, Ordering::Relaxed);

    let mut list_guard = list.lock().unwrap();
    *list_guard = result;

    Ok(())
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
