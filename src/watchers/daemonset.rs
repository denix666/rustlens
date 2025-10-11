use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::apps::v1::DaemonSet, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct DaemonSetItem {
    pub name: String,
    pub desired: i32,
    pub current: i32,
    pub ready: i32,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_daemonset(ds: DaemonSet) -> Option<DaemonSetItem> {
    let metadata = &ds.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();

    let status = ds.status?;
    Some(DaemonSetItem {
        name,
        desired: status.desired_number_scheduled,
        current: status.current_number_scheduled,
        ready: status.number_ready,
        creation_timestamp,
        namespace: ds.metadata.namespace.clone(),
    })
}

pub async fn watch_daemonsets(client: Arc<Client>, daemonsets_list: Arc<Mutex<Vec<DaemonSetItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<DaemonSet> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = daemonsets_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_daemonset).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(ds) => {
                    if let Some(item) = convert_daemonset(ds) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list_guard = daemonsets_list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(ds) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_daemonset(ds) {
                        let mut list = daemonsets_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(ds) => {
                    if !initialized {
                        continue;
                    }
                    if let (Some(name), Some(namespace)) = (ds.metadata.name, ds.metadata.namespace) {
                        let mut list = daemonsets_list.lock().unwrap();
                        list.retain(|item| !(item.name == name && item.namespace.as_ref() == Some(&namespace)));
                    }
                }
            },
            Err(e) => eprintln!("DaemonSet watch error: {:?}", e),
        }
    }
}
