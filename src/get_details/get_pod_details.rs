use std::{collections::BTreeMap, sync::{Arc, Mutex}};
use kube::{Api, Client};
use k8s_openapi::{api::{core::v1::Pod}};

#[derive(Debug, Clone)]
pub struct PodDetails {
    pub name: Option<String>,
    pub uid: Option<String>,
    pub annotations: Option<BTreeMap<String, String>>,
    pub labels: Option<BTreeMap<String, String>>,
    pub service_account: Option<String>,
    pub pod_ip: Option<String>,
    pub host_ip: Option<String>,
}

impl PodDetails {
    pub fn new() -> Self {
        Self {
            name: None,
            uid: None,
            annotations: None,
            labels: None,
            service_account: None,
            pod_ip: None,
            host_ip: None,
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
    details_items.labels = metadata.labels;
    details_items.service_account = spec.and_then(|s| s.service_account_name.clone());
    details_items.pod_ip = status.and_then(|s| s.pod_ip.clone());
    details_items.host_ip = status.and_then(|s| s.host_ip.clone());

    Ok(())
}
