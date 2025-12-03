use kube::{Api, Client};
use std::sync::{Arc, Mutex};
use k8s_openapi::api::coordination::v1::Lease;

#[derive(Debug, Clone)]
pub struct EventDetails {
    pub reason: Option<String>,
    pub message: Option<String>,
    pub event_type: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct LeaseDetails {
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub events: Vec<EventDetails>,
}

pub async fn get_lease_details(client: Arc<Client>, name: &str, ns: Option<String>, details: Arc<Mutex<LeaseDetails>>) -> Result<(), kube::Error> {
    let ns = ns.unwrap_or("default".to_string());
    let api: Api<Lease> = Api::namespaced(client.as_ref().clone(), ns.as_str());
    let lease = api.get(name).await.unwrap();
    let lease_events = crate::get_resource_events(client.clone(), "Lease", ns.clone().as_str(), name).await.unwrap();
    let mut details_items = details.lock().unwrap();
    let metadata = lease.metadata.clone();

    details_items.name = metadata.name;
    details_items.namespace = Some(ns);

    details_items.events = lease_events.iter().map(|e| {
        EventDetails {
            reason: e.reason.clone(),
            message: e.message.clone(),
            event_type: e.type_.clone(),
            timestamp: e.last_timestamp.as_ref().map(|ts| ts.0.to_rfc3339()),
        }
    }).collect();

    Ok(())
}
