use std::{collections::BTreeMap, sync::{Arc, Mutex}};
use k8s_openapi::api::core::v1::Node;
use kube::{Api, Client};

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
