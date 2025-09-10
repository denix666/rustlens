use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use k8s_openapi::{api::networking::v1::NetworkPolicy, apimachinery::pkg::apis::meta::v1::Time};
use kube::{Client, Api, runtime::watcher, runtime::watcher::Event};
use futures_util::StreamExt;
use kube::api::ListParams;

#[derive(Debug, Clone)]
pub struct NetworkPolicyItem {
    pub name: String,
    pub pod_selector: String,
    pub policy_types: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_network_policy(policy: NetworkPolicy) -> Option<NetworkPolicyItem> {
    let metadata = &policy.metadata;
    let name = metadata.name.clone()?;

    let pod_selector = policy
        .spec
        .as_ref()
        .and_then(|spec| spec.pod_selector.as_ref())
        .and_then(|sel| sel.match_labels.as_ref())
        .map(|labels| {
            labels
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "None".to_string());

    let policy_types = policy
        .spec
        .as_ref()
        .and_then(|spec| spec.policy_types.as_ref())
        .map(|types| types.join(", "))
        .unwrap_or_else(|| "None".to_string());

    Some(NetworkPolicyItem {
        name,
        pod_selector,
        policy_types,
        creation_timestamp: metadata.creation_timestamp.clone(),
        namespace: metadata.namespace.clone(),
    })
}

pub async fn watch_network_policies(client: Arc<Client>, list: Arc<Mutex<Vec<NetworkPolicyItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<NetworkPolicy> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_network_policy).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(policy) => {
                    if let Some(item) = convert_network_policy(policy) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list_guard = list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(policy) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_network_policy(policy) {
                        let mut list = list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(policy) => {
                    if let Some(item) = policy.metadata.name {
                        let mut policy_vec = list.lock().unwrap();
                        policy_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("NetworkPolicy watch error: {:?}", e),
        }
    }
}
