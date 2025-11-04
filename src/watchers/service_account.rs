use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{core::v1::ServiceAccount}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher};
use kube::runtime::reflector::Lookup;

#[derive(Clone, Debug)]
pub struct ServiceAccountItem {
    pub name: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

fn convert_service_account(service_account: ServiceAccount) -> Option<ServiceAccountItem> {
    let name = service_account.name().unwrap().to_string();
    let namespace = service_account.metadata.namespace.clone();
    Some(ServiceAccountItem {
        name,
        creation_timestamp: service_account.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_service_accounts(client: Arc<Client>, service_accounts_list: Arc<Mutex<Vec<ServiceAccountItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<ServiceAccount> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = service_accounts_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_service_account).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(service_account) => {
                    if let Some(item) = convert_service_account(service_account) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = service_accounts_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(service_account) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_service_account(service_account) {
                        let mut list = service_accounts_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|p| p.name == item.name && p.namespace == item.namespace) {
                            *existing = item;
                        } else {
                            list.push(item);
                        }
                    }
                }
                watcher::Event::Delete(service_account) => {
                    if !initialized {
                        continue;
                    }
                    if let (Some(name), Some(namespace)) = (service_account.metadata.name, service_account.metadata.namespace) {
                        let mut list = service_accounts_list.lock().unwrap();
                        list.retain(|item| !(item.name == name && item.namespace.as_ref() == Some(&namespace)));
                    }
                }
            },
            Err(e) => log::error!("ServiceAccount watch error: {:?}", e),
        }
    }
}
