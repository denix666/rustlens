use kube::{Api, Client};
use k8s_openapi::api::storage::v1::StorageClass;
use std::{collections::BTreeMap, sync::{Arc, Mutex}};

#[derive(Debug, Clone)]
pub struct EventDetails {
    pub reason: Option<String>,
    pub message: Option<String>,
    pub event_type: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct ScDetails {
    pub name: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
    pub finalizers: Option<Vec<String>>,
    pub mount_options: Option<Vec<String>>,
    pub parameters: Option<BTreeMap<String, String>>,
    pub events: Vec<EventDetails>,
}

pub async fn get_sc_details(client: Arc<Client>, name: &str, details: Arc<Mutex<ScDetails>>) -> Result<(), kube::Error> {
    let api: Api<StorageClass> = Api::all(client.as_ref().clone());
    let sc = api.get(name).await.unwrap();
    let sc_events = crate::get_cluster_resource_events(client.clone(), "StorageClass", name).await.unwrap();
    let mut details_items = details.lock().unwrap();
    let metadata = sc.metadata.clone();

    details_items.name = metadata.name;
    details_items.labels = metadata.labels.clone();
    details_items.annotations = metadata.annotations.clone();
    details_items.finalizers = metadata.finalizers;
    details_items.mount_options = sc.mount_options;
    details_items.parameters = sc.parameters;

    details_items.events = sc_events.iter().map(|e| {
        EventDetails {
            reason: e.reason.clone(),
            message: e.message.clone(),
            event_type: e.type_.clone(),
            timestamp: e.last_timestamp.as_ref().map(|ts| ts.0.to_string()),
        }
    }).collect();

    Ok(())
}
