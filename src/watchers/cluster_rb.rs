use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{rbac::v1::ClusterRoleBinding}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher};
use kube::runtime::reflector::Lookup;

#[derive(Clone, Debug)]
pub struct ClusterRoleBindingItem {
    pub name: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

fn convert_cluster_rb(cluster_rb: ClusterRoleBinding) -> Option<ClusterRoleBindingItem> {
    let name = cluster_rb.name().unwrap().to_string();
    let namespace = cluster_rb.metadata.namespace.clone();
    Some(ClusterRoleBindingItem {
        name,
        creation_timestamp: cluster_rb.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_cluster_rbs(client: Arc<Client>, cluster_rbs_list: Arc<Mutex<Vec<ClusterRoleBindingItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<ClusterRoleBinding> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = cluster_rbs_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_cluster_rb).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(cluster_rb) => {
                    if let Some(item) = convert_cluster_rb(cluster_rb) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = cluster_rbs_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(cluster_rb) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_cluster_rb(cluster_rb) {
                        let mut list = cluster_rbs_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|p| p.name == item.name && p.namespace == item.namespace) {
                            *existing = item;
                        } else {
                            list.push(item);
                        }
                    }
                }
                watcher::Event::Delete(cluster_rb) => {
                    if let Some(item) = cluster_rb.metadata.name {
                        let mut cluster_rbs_vec = cluster_rbs_list.lock().unwrap();
                        cluster_rbs_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("ClusterRoleBinding watch error: {:?}", e),
        }
    }
}
