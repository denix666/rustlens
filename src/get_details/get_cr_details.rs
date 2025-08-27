use std::sync::{Arc, Mutex};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use kube::{api::{Api, ApiResource, DynamicObject, GroupVersionKind},Client, ResourceExt};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CRDItem {
    pub name: String,
    pub group: String,
    pub version: String,
    pub scope: String,
    pub kind: String,
    pub plural: String,
}

pub async fn get_crds(client: Arc<Client>, _name: &str, plural: &str, kind: &str, version: &str, group: &str, scope: &str, ns: Option<String>, details: Arc<Mutex<CrdDetails>>) -> Result<Vec<CRDItem>, String> {
    //println!("getting {}|{}|{}|{}|{}|{}|{:?}", name, version, group, scope, plural, kind, ns);
    let ar = ApiResource::from_gvk_with_plural(&GroupVersionKind::gvk(group, version, kind), plural);

    let api: Api<DynamicObject> = match scope {
        "Namespaced" => Api::namespaced_with(client.as_ref().clone(), &ns.unwrap_or_default(), &ar),
        _ => Api::all_with(client.as_ref().clone(), &ar),
    };

    let list = api.list(&Default::default()).await?;
    let names: Vec<String> = list.items.iter().map(|item| item.name_any()).collect();

    for i in names.clone() {
        println!("{}", i);
    }

    //let crd_events = crate::get_cluster_resource_events(client.clone(), &ar.kind, name).await.unwrap_or_default();
    let obj = api.get("external-secrets-secret-store").await?;
    let mut details_guard = details.lock().unwrap();
    let metadata = obj.metadata.clone();

    details_guard.name = metadata.name.clone();
    details_guard.uid = metadata.uid.clone();
    details_guard.labels = Some(obj.labels().clone());
    details_guard.annotations = Some(obj.annotations().clone());

    if let Some(spec) = obj.data.get("spec") {
        details_guard.spec = Some(spec.clone());
    }



    if let Some(status) = obj.data.get("status") {
        details_guard.status = Some(status.clone());
        if let Some(conds) = status.get("conditions").and_then(|c| c.as_array()) {
            details_guard.conditions = conds.clone();
        }
    }

    // details_guard.events = crd_events.iter().map(|e| {
    //     EventDetails {
    //         reason: e.reason.clone(),
    //         message: e.message.clone(),
    //         event_type: e.type_.clone(),
    //         timestamp: e.last_timestamp.as_ref().map(|ts| ts.0.to_rfc3339()),
    //     }
    // }).collect();

    Ok(())
}
