use kube::{Api, Client};
use k8s_openapi::api::rbac::v1::ClusterRole;
use std::{collections::BTreeMap, sync::{Arc, Mutex}};

#[derive(Debug, Clone)]
pub struct EventDetails {
    pub reason: Option<String>,
    pub message: Option<String>,
    pub event_type: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct ClusterRoleDetails {
    pub name: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
    pub events: Vec<EventDetails>,
}

pub async fn get_cluster_role_details(client: Arc<Client>, name: &str, details: Arc<Mutex<ClusterRoleDetails>>) -> Result<(), kube::Error> {
    let api: Api<ClusterRole> = Api::all(client.as_ref().clone());
    let cluster_role = api.get(name).await.unwrap();
    let cluster_role_events = crate::get_resource_events(client.clone(), "ClusterRole", "default", name).await.unwrap();
    let mut details_items = details.lock().unwrap();
    let metadata = cluster_role.metadata.clone();

    details_items.name = metadata.name;
    details_items.labels = metadata.labels.clone();
    details_items.annotations = metadata.annotations.clone();

    details_items.events = cluster_role_events.iter().map(|e| {
        EventDetails {
            reason: e.reason.clone(),
            message: e.message.clone(),
            event_type: e.type_.clone(),
            timestamp: e.last_timestamp.as_ref().map(|ts| ts.0.to_rfc3339()),
        }
    }).collect();

    Ok(())
}
