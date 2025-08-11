use std::{collections::BTreeMap, sync::{Arc, Mutex}};
use kube::{Api, Client};
use k8s_openapi::{api::apps::v1::Deployment};


#[derive(Debug, Clone)]
pub struct DeploymentDetails {
    pub name: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub namespace: Option<String>,
}

impl DeploymentDetails {
    pub fn new() -> Self {
        Self {
            name: None,
            labels: None,
            namespace: None,
        }
    }
}

pub async fn get_deployment_details(client: Arc<Client>, name: &str, ns: Option<String>, details: Arc<Mutex<DeploymentDetails>>) -> Result<(), kube::Error> {
    let ns = ns.unwrap_or("default".to_string());
    let api: Api<Deployment> = Api::namespaced(client.as_ref().clone(), ns.as_str());
    let deployment = api.get(name).await.unwrap();
    let mut details_items = details.lock().unwrap();

    let metadata = deployment.metadata.clone();

    details_items.name = metadata.name;
    details_items.labels = metadata.labels;
    details_items.namespace = Some(ns);

    Ok(())
}
