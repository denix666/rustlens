use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{apps::v1::ReplicaSet}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct ReplicaSetItem {
    pub name: String,
    pub desired: i32,
    pub current: i32,
    pub ready: i32,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_replicaset(rs: ReplicaSet) -> Option<ReplicaSetItem> {
    Some(ReplicaSetItem {
        name: rs.metadata.name.clone()?,
        desired: rs.spec.as_ref()?.replicas.unwrap_or(0),
        current: rs.status.as_ref()?.replicas,
        ready: rs.status.as_ref()?.ready_replicas.unwrap_or(0),
        creation_timestamp: rs.metadata.creation_timestamp,
        namespace: rs.metadata.namespace.clone()
    })
}

pub async fn watch_replicasets(client: Arc<Client>, rs_list: Arc<Mutex<Vec<ReplicaSetItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<ReplicaSet> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = rs_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_replicaset).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(rs) => {
                    if let Some(item) = convert_replicaset(rs) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = rs_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(rs) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_replicaset(rs) {
                        let mut list = rs_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(rs) => {
                    if !initialized {
                        continue;
                    }
                    if let (Some(name), Some(namespace)) = (rs.metadata.name, rs.metadata.namespace) {
                        let mut list = rs_list.lock().unwrap();
                        list.retain(|item| !(item.name == name && item.namespace.as_ref() == Some(&namespace)));
                    }
                }
            },
            Err(e) => eprintln!("ReplicaSet watch error: {:?}", e),
        }
    }
}
