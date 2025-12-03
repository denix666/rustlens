use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use k8s_openapi::{api::coordination::v1::Lease, apimachinery::pkg::apis::meta::v1::Time};
use kube::Client;
use futures_util::StreamExt;
use kube::{Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct LeaseItem {
    pub name: String,
    pub holder: Option<String>,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_lease(lease: Lease) -> Option<LeaseItem> {
    let metadata = &lease.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();
    let spec = lease.spec?;
    let namespace = lease.metadata.namespace.clone();

    Some(LeaseItem {
        name,
        holder: spec.holder_identity,
        creation_timestamp,
        namespace,
    })
}

pub async fn watch_leases(client: Arc<Client>, list: Arc<Mutex<Vec<LeaseItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Lease> = Api::all(client.as_ref().clone());
    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(lease) => {
                    if let Some(item) = convert_lease(lease) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list_guard = list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(lease) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_lease(lease) {
                        let mut list = list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(lease) => {
                    if !initialized {
                        continue;
                    }
                    if let (Some(name), Some(namespace)) = (lease.metadata.name, lease.metadata.namespace) {
                        let mut list = list.lock().unwrap();
                        list.retain(|item| !(item.name == name && item.namespace.as_ref() == Some(&namespace)));
                    }
                }
            },
            Err(e) => log::error!("Lease watch error: {:?}", e),
        }
    }
}
