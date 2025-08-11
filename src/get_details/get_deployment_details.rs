use std::{collections::BTreeMap, sync::{Arc, Mutex}};
use kube::{Api, Client};
use k8s_openapi::api::{apps::v1::Deployment};

#[derive(Debug, Clone, Default)]
pub struct ConditionDetails {
    pub type_: String,
    // pub status: String,
    // pub reason: Option<String>,
    // pub message: Option<String>,
    // pub last_transition_time: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EventDetails {
    pub reason: Option<String>,
    pub message: Option<String>,
    pub event_type: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct DeploymentDetails {
    pub name: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub namespace: Option<String>,
    pub annotations: Option<BTreeMap<String, String>>,
    pub strategy: Option<String>,
    pub selector: Vec<(String, String)>,
    pub conditions: Vec<ConditionDetails>,
    pub events: Vec<EventDetails>,
}

pub async fn get_deployment_details(client: Arc<Client>, name: &str, ns: Option<String>, details: Arc<Mutex<DeploymentDetails>>) -> Result<(), kube::Error> {
    let ns = ns.unwrap_or("default".to_string());
    let api: Api<Deployment> = Api::namespaced(client.as_ref().clone(), ns.as_str());
    let deployment = api.get(name).await.unwrap();
    let deployment_events = crate::get_resource_events(client.clone(), "Deployment", ns.clone().as_str(), name).await.unwrap();
    let mut details_items = details.lock().unwrap();

    let metadata = deployment.metadata.clone();
    let deployment_spec = deployment.spec;
    let status = deployment.status.as_ref();

    details_items.name = metadata.name;
    details_items.labels = metadata.labels;
    details_items.namespace = Some(ns);
    details_items.annotations = metadata.annotations;
    if let Some(spec) = deployment_spec {
        details_items.strategy = spec.strategy.unwrap().type_;
        details_items.selector = spec.selector.match_labels
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect();
    }

    if let Some(status) = status {
        details_items.conditions = status.conditions.clone().unwrap_or_default().into_iter().map(|c| {
            ConditionDetails {
                type_: c.type_,
                // status: c.status,
                // reason: c.reason,
                // message: c.message,
                // last_transition_time: c.last_transition_time.map(|t| t.0.to_rfc3339()),
            }
        }).collect();
    }

    details_items.events = deployment_events.iter().map(|e| {
        EventDetails {
            reason: e.reason.clone(),
            message: e.message.clone(),
            event_type: e.type_.clone(),
            timestamp: e.last_timestamp.as_ref().map(|ts| ts.0.to_rfc3339()),
        }
    }).collect();

    Ok(())
}
