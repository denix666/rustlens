use std::{collections::BTreeMap, sync::{Arc, Mutex}};
use kube::{Api, Client};
use k8s_openapi::api::core::v1::{Affinity, Pod, PodCondition};
use k8s_openapi::api::core::v1::Toleration;

#[derive(Debug, Clone)]
pub struct ContainerDetails {
    pub name: String,
    pub image: Option<String>,
    pub state: Option<String>, // e.g. "Running", "Terminated", "Waiting"
    pub message: Option<String>,
    pub cpu_request: Option<String>,
    pub mem_request: Option<String>,
    pub cpu_limit: Option<String>,
    pub mem_limit: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PodDetails {
    pub name: Option<String>,
    pub uid: Option<String>,
    pub annotations: Option<BTreeMap<String, String>>,
    pub labels: Option<BTreeMap<String, String>>,
    pub service_account: Option<String>,
    pub pod_ip: Option<String>,
    pub namespace: Option<String>,
    pub host_ip: Option<String>,
    pub tolerations: Vec<Toleration>,
    pub affinity: Option<Affinity>,
    pub node_selector: Option<BTreeMap<String, String>>,
    pub conditions: Vec<PodCondition>,
    pub containers: Vec<ContainerDetails>,
}

impl PodDetails {
    pub fn new() -> Self {
        Self {
            name: None,
            namespace: None,
            uid: None,
            annotations: None,
            labels: None,
            service_account: None,
            pod_ip: None,
            host_ip: None,
            tolerations: vec![],
            affinity: None,
            node_selector: None,
            conditions: vec![],
            containers: vec![],
        }
    }
}

pub async fn get_pod_details(client: Arc<Client>, name: &str, ns: Option<String>, details: Arc<Mutex<PodDetails>>) -> Result<(), kube::Error> {
    let ns = ns.unwrap_or("default".to_string());
    let api: Api<Pod> = Api::namespaced(client.as_ref().clone(), ns.as_str());
    let pod = api.get(name).await.unwrap();
    let mut details_items = details.lock().unwrap();

    let metadata = pod.metadata.clone();
    let spec = pod.spec.as_ref();
    let status = pod.status.as_ref();

    details_items.annotations = metadata.annotations.clone();
    details_items.uid = metadata.uid;
    details_items.name = metadata.name;
    details_items.namespace = Some(ns);
    details_items.labels = metadata.labels;
    details_items.service_account = spec.and_then(|s| s.service_account_name.clone());
    details_items.pod_ip = status.and_then(|s| s.pod_ip.clone());
    details_items.host_ip = status.and_then(|s| s.host_ip.clone());
    details_items.tolerations = spec.map(|s| s.tolerations.clone().unwrap_or_default()).unwrap_or_default();
    details_items.affinity = spec.and_then(|s| s.affinity.clone());
    details_items.node_selector = spec.map(|s| s.node_selector.clone().unwrap_or_default());
    details_items.conditions = status.map(|s| s.conditions.clone().unwrap_or_default()).unwrap_or_default();

    if let (Some(pod_spec), Some(statuses)) = (pod.spec.as_ref(), pod.status.as_ref().and_then(|s| s.container_statuses.clone())) {
        details_items.containers.clear();

        for cs in statuses {
            let spec_container = pod_spec.containers.iter().find(|c| c.name == cs.name);

            let (cpu_request, mem_request, cpu_limit, mem_limit) = if let Some(container) = spec_container {
                if let Some(resources) = &container.resources {
                    let cpu_req = resources.requests.as_ref()
                        .and_then(|r| r.get("cpu"))
                        .map(|q| q.0.clone());

                    let mem_req = resources.requests.as_ref()
                        .and_then(|r| r.get("memory"))
                        .map(|q| q.0.clone());

                    let cpu_lim = resources.limits.as_ref()
                        .and_then(|l| l.get("cpu"))
                        .map(|q| q.0.clone());

                    let mem_lim = resources.limits.as_ref()
                        .and_then(|l| l.get("memory"))
                        .map(|q| q.0.clone());

                    (cpu_req, mem_req, cpu_lim, mem_lim)
                } else {
                    (None, None, None, None)
                }
            } else {
                (None, None, None, None)
            };

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

            details_items.containers.push(ContainerDetails {
                name: cs.name,
                image: Some(cs.image),
                state,
                message,
                mem_limit,
                mem_request,
                cpu_limit,
                cpu_request,
            });
        }
    }

    Ok(())
}
