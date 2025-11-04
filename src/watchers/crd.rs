use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::api::{DynamicObject, GroupVersionKind};
use kube::{Client, Api, runtime::watcher, runtime::watcher::Event};
use kube::{discovery, ResourceExt};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct CRDItem {
    pub name: String,
    pub group: String,
    pub version: String,
    pub scope: String,
    pub kind: String,
    pub plural: String,
    pub creation_timestamp: Option<Time>,
}

fn convert_crd(obj: &kube::api::DynamicObject) -> Option<CRDItem> {
    let name = obj.name_any();
    let spec: &Value = obj.data.get("spec")?;
    let group = spec.get("group")?.as_str()?.to_string();
    let scope = spec.get("scope")?.as_str()?.to_string();
    let creation_timestamp = obj.metadata.creation_timestamp.clone();

    let version = spec
        .get("versions")?
        .as_array()?
        .get(0)?
        .get("name")?
        .as_str()?
        .to_string();

    let kind = spec
        .get("names")?
        .get("kind")?
        .as_str()?
        .to_string();

    let plural = spec
        .get("names")?
        .get("plural")?
        .as_str()?
        .to_string();

    Some(CRDItem {
        name,
        group,
        plural,
        version,
        scope,
        kind,
        creation_timestamp,
    })
}

pub async fn watch_crds(client: Arc<Client>, list: Arc<Mutex<Vec<CRDItem>>>, load_status: Arc<AtomicBool>) {
    let (ar, _caps) = discovery::pinned_kind(&client, &GroupVersionKind::gvk("apiextensions.k8s.io", "v1", "CustomResourceDefinition")).await.unwrap();
    let api: Api<DynamicObject> = Api::all_with(client.as_ref().clone(), &ar);

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(obj) => {
                    if let Some(item) = convert_crd(&obj) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list_guard = list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(obj) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_crd(&obj) {
                        let mut list_guard = list.lock().unwrap();
                        if let Some(existing) = list_guard.iter_mut().find(|f| f.name == item.name) {
                            *existing = item; // renew
                        } else {
                            list_guard.push(item); // add new
                        }
                    }
                }
                Event::Delete(obj) => {
                    if let Some(item) = obj.metadata.name {
                        let mut obj_vec = list.lock().unwrap();
                        obj_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => log::error!("CRDs watch error: {:?}", e),
        }
    }
}
