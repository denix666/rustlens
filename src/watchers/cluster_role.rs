use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{rbac::v1::ClusterRole}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher};
use kube::runtime::reflector::Lookup;

#[derive(Clone, Debug)]
pub struct ClusterRoleItem {
    pub name: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

fn convert_cluster_role(cluster_role: ClusterRole) -> Option<ClusterRoleItem> {
    let name = cluster_role.name().unwrap().to_string();
    let namespace = cluster_role.metadata.namespace.clone();
    Some(ClusterRoleItem {
        name,
        creation_timestamp: cluster_role.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_cluster_roles(client: Arc<Client>, cluster_roles_list: Arc<Mutex<Vec<ClusterRoleItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<ClusterRole> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = cluster_roles_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_cluster_role).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(cluster_role) => {
                    if let Some(item) = convert_cluster_role(cluster_role) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = cluster_roles_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(cluster_role) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_cluster_role(cluster_role) {
                        let mut list = cluster_roles_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|p| p.name == item.name && p.namespace == item.namespace) {
                            *existing = item;
                        } else {
                            list.push(item);
                        }
                    }
                }
                watcher::Event::Delete(cluster_role) => {
                    if let Some(item) = cluster_role.metadata.name {
                        let mut cluster_roles_vec = cluster_roles_list.lock().unwrap();
                        cluster_roles_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("ClusterRole watch error: {:?}", e),
        }
    }
}
