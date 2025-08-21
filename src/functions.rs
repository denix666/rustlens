use eframe::egui::Color32;
use futures::{AsyncBufReadExt};
use k8s_openapi::api::rbac::v1::{ClusterRole, Role};
use k8s_openapi::{ClusterResourceScope, Metadata, NamespaceResourceScope, Resource};
use kube::runtime::reflector::Lookup;
use kube::{Api, Client, Config};
use kube::config::{Kubeconfig, NamedContext};
use kube::discovery;
use k8s_openapi::api::core::v1::{ConfigMap, Event, Namespace, Node, PersistentVolumeClaim, Pod, Secret, ServiceAccount};
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet, ReplicaSet};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use futures_util::StreamExt;
use serde_json::json;
use kube::api::{DeleteParams, ListParams, LogParams, Patch, PatchParams, PostParams, PropagationPolicy};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use chrono::{Utc, DateTime};
use serde_yaml;
use crate::ui::OverviewStats;

pub fn compute_overview_stats(
    pods: &Vec<crate::watchers::PodItem>,
    deployments: &Vec<crate::watchers::DeploymentItem>,
    daemonsets: &Vec<crate::watchers::DaemonSetItem>,
    statefulsets: &Vec<crate::watchers::StatefulSetItem>,
    replicasets: &Vec<crate::watchers::ReplicaSetItem>,
) -> OverviewStats {
    let mut stats = OverviewStats::default();

    // Pods
    for pod in pods {
        if pod.ready_containers < pod.total_containers {
            stats.pods_pending += 1;
        } else {
            stats.pods_running += 1;
        }
    }

    // Deployments
    for deployment in deployments {
        stats.deployments_pending += deployment.unavailable_replicas as usize;
    }
    stats.deployments_running = deployments.len() - stats.deployments_pending;

    // Daemonsets
    for daemonset in daemonsets {
        if daemonset.ready < daemonset.desired {
            stats.pods_pending += 1;
        } else {
            stats.daemonsets_running += 1;
        }
    }

    // Statefulsets
    for statefulset in statefulsets {
        if statefulset.ready_replicas < statefulset.replicas {
            stats.statefulsets_pending += 1;
        } else {
            stats.statefulsets_running += 1;
        }
    }

    // Replicasets
    for replicaset in replicasets {
        if replicaset.ready < replicaset.desired {
            stats.replicasets_pending += 1;
        } else {
            stats.replicasets_running += 1;
        }
    }

    stats
}

pub async fn get_resource_events(client: Arc<Client>, kind: &str, namespace: &str, name: &str) -> Result<Vec<Event>, kube::Error> {
    let events: Api<Event> = Api::namespaced(client.as_ref().clone(), namespace);

    let lp = ListParams::default().fields(&format!(
        "involvedObject.kind={},involvedObject.name={}",
        kind, name
    ));

    let event_list = events.list(&lp).await?;
    Ok(event_list.items)
}

pub fn load_embedded_icon() -> Result<crate::egui::IconData, String> {
    let img = image::load_from_memory(super::ICON_BYTES).map_err(|e| e.to_string())?.into_rgba8();
    let (width, height) = img.dimensions();
    let rgba = img.into_raw();
    Ok(crate::egui::IconData { rgba, width, height })
}

pub fn spawn_watcher<T, F>(client: Arc<Client>, state: Arc<Mutex<Vec<T>>>, loading_flag: Arc<AtomicBool>, watch_fn: F) where
    T: Send + 'static,
    F: FnOnce(Arc<Client>, Arc<Mutex<Vec<T>>>, Arc<AtomicBool>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
{
    tokio::spawn(watch_fn(client, state, loading_flag));
}

pub fn format_age(ts: &Time) -> String {
    let now = Utc::now();
    let created: DateTime<Utc> = ts.0;
    let duration = now - created;

    let total_seconds = duration.num_seconds();

    if total_seconds < 60 {
        format!("{}s", total_seconds)
    } else if total_seconds < 3600 {
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{}m {}s", minutes, seconds)
    } else if total_seconds < 86400 {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    } else if total_seconds < 31536000 {
        let days = total_seconds / 86400;
        let hours = (total_seconds % 86400) / 3600;
        format!("{}d {}h", days, hours)
    } else {
        let years = total_seconds / 31536000;
        let days = (total_seconds % 31536000) / 86400;
        format!("{}y {}d", years, days)
    }
}

// get yaml for namespaced resources
pub async fn get_yaml_namespaced<T>(client: Arc<Client>, namespace: &str, name: &str, ) -> Result<String, kube::Error> where
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

pub async fn get_yaml_global<T>(client: Arc<Client>, name: &str, ) -> Result<String, kube::Error> where
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
        "Burstable" => Color32::from_rgb(137, 90, 9), // close to orange
        "Guaranteed" => Color32::from_rgb(6, 140, 0), // green
        "BestEffort" => Color32::from_rgb(112, 135, 9), // close to red
        "Cancelled" => Color32::from_rgb(116, 116, 116), // gray
        "RW" => Color32::from_rgb(137, 90, 9), // close to orange
        "RO" => Color32::from_rgb(6, 140, 0), // green
        "CrashLoop" => Color32::RED,
        "NotReady" => Color32::RED,
        "Running" => Color32::GREEN,
        "Waiting" => Color32::YELLOW,
        "Terminated" => Color32::RED,
        "Complete" => Color32::GREEN,
        "Completed" => Color32::GREEN,
        "Succeeded" => Color32::GREEN,
        "Failed" => Color32::RED,
        "Bound" => Color32::GREEN,
        "Progressing" => Color32::LIGHT_BLUE,
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

pub fn edit_yaml_for<K>(name: String, namespace: String, yaml_editor_window: Arc<Mutex<crate::YamlEditorWindow>>, client: Arc<Client>) where
    K: Clone
        + serde::de::DeserializeOwned
        + Serialize
        + Metadata<Ty = kube::core::ObjectMeta>
        + Resource<Scope = kube::core::NamespaceResourceScope>
        + std::fmt::Debug
        + 'static,
{
    tokio::spawn(async move {
        match get_yaml_namespaced::<K>(client, &namespace, &name).await {
            Ok(yaml) => {
                let mut editor = yaml_editor_window.lock().unwrap();
                editor.content = yaml;
                editor.show = true;
            }
            Err(e) => {
                eprintln!("Failed to get YAML: {}", e);
            }
        }
    });
}

pub fn edit_cluster_yaml_for<K>(name: String, yaml_editor_window: Arc<Mutex<crate::YamlEditorWindow>>, client: Arc<Client>) where
    K: Clone
        + serde::de::DeserializeOwned
        + Serialize
        + Metadata<Ty = kube::core::ObjectMeta>
        + Resource<Scope = kube::core::ClusterResourceScope>
        + std::fmt::Debug
        + 'static,
{
    tokio::spawn(async move {
        match get_yaml_global::<K>(client, &name).await {
            Ok(yaml) => {
                let mut editor = yaml_editor_window.lock().unwrap();
                editor.content = yaml;
                editor.show = true;
            }
            Err(e) => {
                eprintln!("Failed to get YAML: {}", e);
            }
        }
    });
}

pub fn open_logs_for_pod(pod_name: String, namespace: String, containers: Vec<crate::ContainerStatusItem>, log_window: Arc<Mutex<crate::LogWindow>>, client: Arc<Client>) {
    let mut logs = log_window.lock().unwrap();
    logs.pod_name = pod_name.clone();
    logs.namespace = namespace.clone();
    logs.containers = containers.clone();
    logs.selected_container = containers.get(0).map(|c| c.name.clone()).unwrap_or_default();
    logs.last_container = None;
    logs.buffer = Arc::new(Mutex::new(String::new()));
    logs.show = true;

    let selected_container = containers.get(0).map(|c| c.name.clone()).unwrap_or_default();
    let buffer = Arc::new(Mutex::new(String::new()));

    tokio::spawn(async move {
        crate::fetch_logs(
            client,
            namespace.as_str(),
            pod_name.as_str(),
            selected_container.as_str(),
            buffer,
        )
        .await;
    });
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
        crate::ResourceType::ServiceAccount => {
            let obj: ServiceAccount = serde_yaml::from_value(value)?;
            let ns = obj.namespace().unwrap();
            let api: Api<ServiceAccount> = Api::namespaced(client.as_ref().clone(), &ns);
            api.create(&PostParams::default(), &obj).await?;
        },
        crate::ResourceType::Role => {
            let obj: Role = serde_yaml::from_value(value)?;
            let ns = obj.namespace().unwrap();
            let api: Api<Role> = Api::namespaced(client.as_ref().clone(), &ns);
            api.create(&PostParams::default(), &obj).await?;
        },
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
        crate::ResourceType::ClusterRole => {
            let obj: ClusterRole = serde_yaml::from_value(value)?;
            let api: Api<ClusterRole> = Api::all(client.as_ref().clone());
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

pub async fn delete_service_account(client: Arc<Client>, service_account_name: &str, namespace: Option<&str>) -> Result<(), kube::Error> {
    let ns = namespace.unwrap_or("default");
    let service_accounts: Api<ServiceAccount> = Api::namespaced(client.as_ref().clone(), ns);
    service_accounts.delete(service_account_name, &DeleteParams::default()).await?;
    Ok(())
}

pub async fn delete_role(client: Arc<Client>, role_name: &str, namespace: Option<&str>) -> Result<(), kube::Error> {
    let ns = namespace.unwrap_or("default");
    let roles: Api<Role> = Api::namespaced(client.as_ref().clone(), ns);
    roles.delete(role_name, &DeleteParams::default()).await?;
    Ok(())
}

pub async fn delete_cluster_role(client: Arc<Client>, cluster_role_name: &str) -> Result<(), kube::Error> {
    let roles: Api<Role> = Api::all(client.as_ref().clone());
    roles.delete(cluster_role_name, &DeleteParams::default()).await?;
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
