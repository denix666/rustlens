use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{rbac::v1::Role}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher};
use kube::runtime::reflector::Lookup;

#[derive(Clone, Debug)]
pub struct RoleItem {
    pub name: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

fn convert_role(role: Role) -> Option<RoleItem> {
    let name = role.name().unwrap().to_string();
    let namespace = role.metadata.namespace.clone();
    Some(RoleItem {
        name,
        creation_timestamp: role.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_roles(client: Arc<Client>, roles_list: Arc<Mutex<Vec<RoleItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Role> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = roles_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_role).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(role) => {
                    if let Some(item) = convert_role(role) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = roles_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(role) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_role(role) {
                        let mut list = roles_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|p| p.name == item.name && p.namespace == item.namespace) {
                            *existing = item;
                        } else {
                            list.push(item);
                        }
                    }
                }
                watcher::Event::Delete(role) => {
                    if let Some(item) = role.metadata.name {
                        let mut roles_vec = roles_list.lock().unwrap();
                        roles_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("Role watch error: {:?}", e),
        }
    }
}
