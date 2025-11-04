use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use k8s_openapi::{api::core::v1::PersistentVolumeClaim, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher, runtime::watcher::Event};
use futures_util::StreamExt;

#[derive(Debug, Clone)]
pub struct PvcItem {
    pub name: String,
    pub storage_class: String,
    pub size: String,
    pub volume_name: String,
    pub status: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_pvc(pvc: PersistentVolumeClaim) -> Option<PvcItem> {
    Some(PvcItem {
        name: pvc.metadata.name.clone()?,
        storage_class: pvc
            .spec
            .as_ref()
            .and_then(|s| s.storage_class_name.clone())
            .unwrap_or_else(|| "-".to_string()),
        size: pvc.spec.as_ref()
            .and_then(|s| s.resources.as_ref().unwrap().requests.as_ref())
            .and_then(|r| r.get("storage")).map(|q| q.0.to_string()).unwrap_or_else(|| "".to_string()),
        volume_name: pvc
            .spec
            .as_ref()
            .and_then(|s| s.volume_name.clone())
            .unwrap_or_else(|| "-".to_string()),
        status: pvc
            .status
            .as_ref()
            .and_then(|s| s.phase.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
        creation_timestamp: pvc.metadata.creation_timestamp,
        namespace: pvc.metadata.namespace.clone(),
    })
}

pub async fn watch_pvcs(client: Arc<Client>, pvc_list: Arc<Mutex<Vec<PvcItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<PersistentVolumeClaim> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = pvc_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_pvc).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(pvc) => {
                    if let Some(item) = convert_pvc(pvc) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = pvc_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(pvc) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_pvc(pvc) {
                        let mut list = pvc_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(pvc) => {
                    if !initialized {
                        continue;
                    }
                    if let (Some(name), Some(namespace)) = (pvc.metadata.name, pvc.metadata.namespace) {
                        let mut list = pvc_list.lock().unwrap();
                        list.retain(|item| !(item.name == name && item.namespace.as_ref() == Some(&namespace)));
                    }
                }
            },
            Err(e) => log::error!("PVC watch error: {:?}", e),
        }
    }
}
