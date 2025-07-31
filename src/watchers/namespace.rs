use std::{collections::BTreeMap, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};
use futures_util::StreamExt;
use k8s_openapi::{api::core::v1::{Namespace}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{Client};
use kube::{Api, runtime::watcher};

#[derive(Clone, PartialEq)]
pub struct NamespaceItem {
    pub name: String,
    pub creation_timestamp: Option<Time>,
    pub phase: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
}

pub fn convert_namespace(ns: Namespace) -> Option<NamespaceItem> {
    Some(NamespaceItem {
        creation_timestamp: ns.metadata.creation_timestamp,
        phase: ns.status.as_ref().and_then(|s| s.phase.clone()),
        labels: ns.metadata.labels.clone(),
        name: ns.metadata.name.unwrap(),
    })
}

pub async fn watch_namespaces(client: Arc<Client>, ns_list: Arc<Mutex<Vec<NamespaceItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Namespace> = Api::all(client.as_ref().clone());
    let mut ns_stream = watcher(api, watcher::Config::default()).boxed();

    load_status.store(true, Ordering::Relaxed);

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = ns_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(ns) => {
                    if let Some(item) = convert_namespace(ns) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut ns_vec = ns_list.lock().unwrap();
                    *ns_vec = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(ns) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_namespace(ns) {
                        let mut list = ns_list.lock().unwrap();

                        // Renew if exists. Esle add
                        if let Some(existing) = list.iter_mut().find(|n| n.name == item.name) {
                            *existing = item;
                        } else {
                            list.push(item);
                        }
                    }
                }
                watcher::Event::Delete(ns) => {
                    if let Some(name) = ns.metadata.name {
                        let mut ns_vec = ns_list.lock().unwrap();
                        ns_vec.retain(|n| n.name != name);
                    }
                }
            },
            Err(e) => {
                eprintln!("Namespace watch error: {:?}", e);
            }
        }
    }
}
