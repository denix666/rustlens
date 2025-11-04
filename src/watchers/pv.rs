use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use k8s_openapi::{api::core::v1::PersistentVolume, apimachinery::pkg::apis::meta::v1::Time};
use kube::{Client, Api, runtime::watcher, runtime::watcher::Event};
use futures_util::StreamExt;

#[derive(Debug, Clone)]
pub struct PvItem {
    pub name: String,
    pub storage_class: String,
    pub capacity: String,
    pub reclaim_policy: String,
    pub claim: String,
    pub status: String,
    pub creation_timestamp: Option<Time>,
}

pub fn convert_pv(pv: PersistentVolume) -> Option<PvItem> {
    Some(PvItem {
        name: pv.metadata.name.clone()?,
        storage_class: pv
            .spec
            .as_ref()
            .and_then(|s| s.storage_class_name.clone())
            .unwrap_or_else(|| "-".to_string()),
        capacity: pv
            .spec
            .as_ref()
            .and_then(|s| s.capacity.as_ref())
            .and_then(|cap| cap.get("storage"))
            .map(|q| q.0.to_string())
            .unwrap_or_else(|| "-".to_string()),
        reclaim_policy: pv
            .spec
            .as_ref()
            .and_then(|s| s.persistent_volume_reclaim_policy.clone())
            .unwrap_or_else(|| "-".to_string()),
        claim: pv
            .spec
            .as_ref()
            .and_then(|s| s.claim_ref.as_ref())
            .map(|c| format!("{}/{}", c.namespace.clone().unwrap_or_default(), c.name.clone().unwrap_or_default()))
            .unwrap_or_else(|| "-".to_string()),
        status: pv
            .status
            .as_ref()
            .and_then(|s| s.phase.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
        creation_timestamp: pv.metadata.creation_timestamp,
    })
}

pub async fn watch_pvs(client: Arc<Client>, pv_list: Arc<Mutex<Vec<PvItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<PersistentVolume> = Api::all(client.as_ref().clone());
    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(pv) => {
                    if let Some(item) = convert_pv(pv) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = pv_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(pv) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_pv(pv) {
                        let mut list = pv_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|p| p.name == item.name) {
                            *existing = item;
                        } else {
                            list.push(item);
                        }
                    }
                }
                Event::Delete(pv) => {
                    if let Some(item) = pv.metadata.name {
                        let mut pv_vec = pv_list.lock().unwrap();
                        pv_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => log::error!("PV watch error: {:?}", e),
        }
    }
}
