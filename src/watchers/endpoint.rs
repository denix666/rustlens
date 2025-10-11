use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{core::v1::Endpoints}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct EndpointItem {
    pub name: String,
    pub addresses: String,
    pub ports: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_endpoint(ep: Endpoints) -> Option<EndpointItem> {
    let metadata = &ep.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();

    let mut all_addresses = Vec::new();
    let mut all_ports = Vec::new();

    if let Some(subsets) = ep.subsets {
        for subset in subsets {
            if let Some(addresses) = subset.addresses {
                for addr in addresses {
                    all_addresses.push(addr.ip);
                }
            }

            if let Some(ports) = subset.ports {
                for port in ports {
                    let port_str = format!(
                        "{}:{}",
                        port.name.unwrap_or_else(|| "-".to_string()),
                        port.port
                    );
                    all_ports.push(port_str);
                }
            }
        }
    }

    Some(EndpointItem {
        name,
        addresses: if all_addresses.is_empty() {
            "-".into()
        } else {
            all_addresses.join(", ")
        },
        ports: if all_ports.is_empty() {
            "-".into()
        } else {
            all_ports.join(", ")
        },
        creation_timestamp,
        namespace: ep.metadata.namespace.clone(),
    })
}

pub async fn watch_endpoints(client: Arc<Client>, endpoints_list: Arc<Mutex<Vec<EndpointItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Endpoints> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = endpoints_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_endpoint).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(ep) => {
                    if let Some(item) = convert_endpoint(ep) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = endpoints_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(ep) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_endpoint(ep) {
                        let mut list = endpoints_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(ep) => {
                    if !initialized {
                        continue;
                    }
                    if let (Some(name), Some(namespace)) = (ep.metadata.name, ep.metadata.namespace) {
                        let mut list = endpoints_list.lock().unwrap();
                        list.retain(|item| !(item.name == name && item.namespace.as_ref() == Some(&namespace)));
                    }
                }
            },
            Err(e) => eprintln!("Endpoint watch error: {:?}", e),
        }
    }
}
