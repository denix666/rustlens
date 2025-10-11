use kube::{Api, Client};
use k8s_openapi::api::core::v1::Service;
use std::{collections::BTreeMap, sync::{Arc, Mutex}};

#[derive(Debug, Clone)]
pub struct EventDetails {
    pub reason: Option<String>,
    pub message: Option<String>,
    pub event_type: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct ServiceDetails {
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
    pub events: Vec<EventDetails>,
    pub selector: Option<BTreeMap<String, String>>,
}

pub async fn get_service_details(client: Arc<Client>, name: &str, ns: Option<String>, details: Arc<Mutex<ServiceDetails>>) -> Result<(), kube::Error> {
    let ns = ns.unwrap_or("default".to_string());
    let api: Api<Service> = Api::namespaced(client.as_ref().clone(), ns.as_str());
    let service = api.get(name).await.unwrap();
    let service_events = crate::get_resource_events(client.clone(), "Service", ns.clone().as_str(), name).await.unwrap();
    let mut details_items = details.lock().unwrap();
    let metadata = service.metadata.clone();
    let spec = service.spec.as_ref();

    details_items.name = metadata.name;
    details_items.namespace = Some(ns);
    details_items.labels = metadata.labels.clone();
    details_items.annotations = metadata.annotations.clone();
    details_items.selector = spec.unwrap().selector.clone();

    details_items.events = service_events.iter().map(|e| {
        EventDetails {
            reason: e.reason.clone(),
            message: e.message.clone(),
            event_type: e.type_.clone(),
            timestamp: e.last_timestamp.as_ref().map(|ts| ts.0.to_rfc3339()),
        }
    }).collect();

    Ok(())
}
