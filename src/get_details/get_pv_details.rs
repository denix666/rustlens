use kube::{Api, Client};
use k8s_openapi::api::core::v1::PersistentVolume;
use std::{collections::BTreeMap, sync::{Arc, Mutex}};

#[derive(Debug, Clone)]
pub struct EventDetails {
    pub reason: Option<String>,
    pub message: Option<String>,
    pub event_type: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct PvDetails {
    pub name: Option<String>,
    pub reclaim_policy: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
    pub finalizers: Option<Vec<String>>,
    pub access_modes: Option<Vec<String>>,
    pub events: Vec<EventDetails>,
}

pub async fn get_pv_details(client: Arc<Client>, name: &str, details: Arc<Mutex<PvDetails>>) -> Result<(), kube::Error> {
    let api: Api<PersistentVolume> = Api::all(client.as_ref().clone());
    let pv = api.get(name).await.unwrap();
    let pv_events = crate::get_cluster_resource_events(client.clone(), "PersistentVolume", name).await.unwrap();
    let mut details_items = details.lock().unwrap();
    let metadata = pv.metadata.clone();
    let spec = pv.spec.as_ref();

    details_items.name = metadata.name;
    details_items.labels = metadata.labels.clone();
    details_items.annotations = metadata.annotations.clone();
    details_items.finalizers = metadata.finalizers;

    details_items.reclaim_policy = spec.and_then(|s| s.persistent_volume_reclaim_policy.clone());
    details_items.access_modes = spec.and_then(|s| s.access_modes.clone());

    details_items.events = pv_events.iter().map(|e| {
        EventDetails {
            reason: e.reason.clone(),
            message: e.message.clone(),
            event_type: e.type_.clone(),
            timestamp: e.last_timestamp.as_ref().map(|ts| ts.0.to_rfc3339()),
        }
    }).collect();

    Ok(())
}
