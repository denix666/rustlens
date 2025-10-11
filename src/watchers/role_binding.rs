use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{rbac::v1::RoleBinding}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher};
use kube::runtime::reflector::Lookup;

#[derive(Clone, Debug)]
pub struct RoleBindingItem {
    pub name: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

fn convert_rb(rb: RoleBinding) -> Option<RoleBindingItem> {
    let name = rb.name().unwrap().to_string();
    let namespace = rb.metadata.namespace.clone();
    Some(RoleBindingItem {
        name,
        creation_timestamp: rb.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_rbs(client: Arc<Client>, rbs_list: Arc<Mutex<Vec<RoleBindingItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<RoleBinding> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = rbs_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_rb).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(rb) => {
                    if let Some(item) = convert_rb(rb) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = rbs_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(rb) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_rb(rb) {
                        let mut list = rbs_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|p| p.name == item.name && p.namespace == item.namespace) {
                            *existing = item;
                        } else {
                            list.push(item);
                        }
                    }
                }
                watcher::Event::Delete(rb) => {
                    if !initialized {
                        continue;
                    }
                    if let (Some(name), Some(namespace)) = (rb.metadata.name, rb.metadata.namespace) {
                        let mut list = rbs_list.lock().unwrap();
                        list.retain(|item| !(item.name == name && item.namespace.as_ref() == Some(&namespace)));
                    }
                }
            },
            Err(e) => eprintln!("RoleBinding watch error: {:?}", e),
        }
    }
}
