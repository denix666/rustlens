use std::{sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};
use futures_util::StreamExt;
use k8s_openapi::{api::{core::v1::ConfigMap}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher};

#[derive(Debug, Clone)]
pub struct ConfigMapItem {
    pub name: String,
    //pub labels: BTreeMap<String, String>,
    pub keys: Vec<String>,
    pub type_: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_configmap(cm: ConfigMap) -> Option<ConfigMapItem> {
    Some(ConfigMapItem {
        name: cm.metadata.name.clone()?,
        //labels: cm.metadata.labels.unwrap_or_default(),
        keys: cm.data.as_ref().map(|d| d.keys().cloned().collect()).unwrap_or_default(),
        type_: "Opaque".to_string(),
        creation_timestamp: cm.metadata.creation_timestamp,
        namespace: cm.metadata.namespace.clone(),
    })
}

pub async fn watch_configmaps(client: Arc<Client>, configmaps_list: Arc<Mutex<Vec<ConfigMapItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<ConfigMap> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = configmaps_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_configmap).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(cm) => {
                    if let Some(item) = convert_configmap(cm) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = configmaps_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(cm) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_configmap(cm) {
                        let mut list = configmaps_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                watcher::Event::Delete(cm) => {
                    if !initialized {
                        continue;
                    }
                    if let (Some(name), Some(namespace)) = (cm.metadata.name, cm.metadata.namespace) {
                        let mut list = configmaps_list.lock().unwrap();
                        list.retain(|item| !(item.name == name && item.namespace.as_ref() == Some(&namespace)));
                    }
                }
            },
            Err(e) => log::error!("ConfigMap watch error: {:?}", e),
        }
    }
}
