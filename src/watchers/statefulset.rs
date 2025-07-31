use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::apps::v1::{StatefulSet}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct StatefulSetItem {
    pub name: String,
    //pub labels: BTreeMap<String, String>,
    pub replicas: i32,
    pub service_name: String,
    pub ready_replicas: i32,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_statefulset(ss: StatefulSet) -> Option<StatefulSetItem> {
    let spec = ss.spec.unwrap();
    let namespace = ss.metadata.namespace.clone();
    Some(StatefulSetItem {
        name: ss.metadata.name.clone()?,
        //labels: ss.metadata.labels.unwrap_or_default(),
        service_name: spec.service_name.unwrap_or("-".to_string()),
        replicas: spec.replicas.unwrap_or(0),
        ready_replicas: ss.status.as_ref()?.ready_replicas.unwrap_or(0),
        creation_timestamp: ss.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_statefulsets(client: Arc<Client>, ss_list: Arc<Mutex<Vec<StatefulSetItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<StatefulSet> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = ss_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_statefulset).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(ss) => {
                    if let Some(item) = convert_statefulset(ss) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = ss_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(ss) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_statefulset(ss) {
                        let mut list = ss_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(ss) => {
                    if let Some(item) = ss.metadata.name {
                        let mut ss_vec = ss_list.lock().unwrap();
                        ss_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("StatefulSet watch error: {:?}", e),
        }
    }
}
