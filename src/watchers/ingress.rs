use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::networking::v1::Ingress, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct IngressItem {
    pub name: String,
    pub host: String,
    pub paths: String,
    pub service: String,
    pub tls: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_ingress(ing: Ingress) -> Option<IngressItem> {
    let metadata = &ing.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();
    let ing_spec = ing.spec.clone();

    let mut hosts = vec![];
    let mut paths = vec![];
    let mut services = vec![];

    if let Some(spec) = ing.spec {
        if let Some(rules) = spec.rules {
            for rule in rules {
                if let Some(host) = rule.host {
                    hosts.push(host.clone());
                }
                if let Some(http) = rule.http {
                    for path in http.paths {
                        let p = path.path.unwrap_or_else(|| "/".to_string());
                        paths.push(p.clone());

                        if let Some(backend) = path.backend.service {
                            services.push(backend.name);
                        }
                    }
                }
            }
        }
    }

    let tls = ing_spec
        .as_ref()
        .and_then(|s| s.tls.as_ref())
        .map(|tls| {
            tls.iter()
                .filter_map(|entry| entry.hosts.clone())
                .flatten()
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "-".to_string());

    Some(IngressItem {
        name,
        host: if hosts.is_empty() { "-".into() } else { hosts.join(", ") },
        paths: if paths.is_empty() { "-".into() } else { paths.join(", ") },
        service: if services.is_empty() { "-".into() } else { services.join(", ") },
        tls,
        creation_timestamp,
        namespace: ing.metadata.namespace.clone(),
    })
}

pub async fn watch_ingresses(client: Arc<Client>, ingresses_list: Arc<Mutex<Vec<IngressItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Ingress> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = ingresses_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_ingress).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(ing) => {
                    if let Some(item) = convert_ingress(ing) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = ingresses_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(ing) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_ingress(ing) {
                        let mut list = ingresses_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(ing) => {
                    if !initialized {
                        continue;
                    }
                    if let (Some(name), Some(namespace)) = (ing.metadata.name, ing.metadata.namespace) {
                        let mut list = ingresses_list.lock().unwrap();
                        list.retain(|item| !(item.name == name && item.namespace.as_ref() == Some(&namespace)));
                    }
                }
            },
            Err(e) => eprintln!("Ingress watch error: {:?}", e),
        }
    }
}
