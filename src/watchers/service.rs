use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::core::v1::{Service}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct ServiceItem {
    pub name: String,
    pub svc_type: String,
    pub cluster_ip: String,
    pub ports: String,
    pub external_ip: String,
    pub selector: String,
    pub creation_timestamp: Option<Time>,
    pub status: String,
    pub namespace: Option<String>,
}

pub fn convert_service(svc: Service) -> Option<ServiceItem> {
    use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;

    let metadata = &svc.metadata;
    let spec = svc.spec.as_ref()?;

    let name = metadata.name.clone()?;
    let svc_type = spec.type_.clone().unwrap_or_else(|| "ClusterIP".to_string());
    let cluster_ip = spec.cluster_ip.clone().unwrap_or_else(|| "None".to_string());

    let ports = spec
        .ports
        .as_ref()
        .map(|ports| {
            ports
                .iter()
                .map(|p| {
                    let port = p.port;
                    let target_port = p.target_port.as_ref().map_or("".to_string(), |tp| match tp {
                        IntOrString::Int(i) => i.to_string(),
                        IntOrString::String(s) => s.to_string(),
                    });
                    let protocol = p.protocol.as_ref().map_or("TCP".to_string(), |s| s.clone());
                    format!("{}/{}→{}", port, protocol, target_port)
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "-".to_string());

    let external_ip = if let Some(eips) = &spec.external_ips {
        eips.join(", ")
    } else if let Some(lb) = &svc.status.and_then(|s| s.load_balancer) {
        if let Some(ing) = &lb.ingress {
            ing.iter()
                .map(|i| {
                    i.ip.clone().or_else(|| i.hostname.clone()).unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            "None".to_string()
        }
    } else {
        "None".to_string()
    };

    let selector = spec
        .selector
        .as_ref()
        .map(|s| {
            s.iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "-".to_string());

    Some(ServiceItem {
        name,
        svc_type,
        cluster_ip,
        ports,
        external_ip,
        selector,
        creation_timestamp: svc.metadata.creation_timestamp,
        status: "OK".to_string(),
        namespace: svc.metadata.namespace.clone(),
    })
}

pub async fn watch_services(client: Arc<Client>, services_list: Arc<Mutex<Vec<ServiceItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Service> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = services_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_service).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(svc) => {
                    if let Some(item) = convert_service(svc) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = services_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(svc) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_service(svc) {
                        let mut list = services_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(svc) => {
                    if let Some(item) = svc.metadata.name {
                        let mut svcs_vec = services_list.lock().unwrap();
                        svcs_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("Service watch error: {:?}", e),
        }
    }
}
