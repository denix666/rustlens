use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::apps::v1::{Deployment}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher};
use kube::runtime::watcher::Event as WatcherEvent;

#[derive(Clone)]
pub struct DeploymentItem {
    pub name: String,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub unavailable_replicas: i32,
    pub updated_replicas: i32,
    pub replicas: i32,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

fn convert_deployment(deploy: Deployment) -> Option<DeploymentItem> {
    let name = deploy.metadata.name.unwrap_or_default();
    let status = deploy.status.unwrap_or_default();
    let namespace = deploy.metadata.namespace.clone();
    Some(DeploymentItem {
        name,
        namespace,
        ready_replicas: status.ready_replicas.unwrap_or(0),
        available_replicas: status.available_replicas.unwrap_or(0),
        unavailable_replicas: status.unavailable_replicas.unwrap_or(0),
        updated_replicas: status.updated_replicas.unwrap_or(0),
        replicas: status.replicas.unwrap_or(0),
        creation_timestamp: deploy.metadata.creation_timestamp,
    })
}

pub async fn watch_deployments(client: Arc<Client>, deployments_list: Arc<Mutex<Vec<DeploymentItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Deployment> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = deployments_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_deployment).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(watch_event) => match watch_event {
                WatcherEvent::Init => initial.clear(),

                WatcherEvent::InitApply(deploy) => {
                    if let Some(item) = convert_deployment(deploy) {
                        initial.push(item);
                    }
                }

                WatcherEvent::InitDone => {
                    let mut list = deployments_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }

                WatcherEvent::Apply(deploy) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_deployment(deploy) {
                        let mut list = deployments_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }

                WatcherEvent::Delete(deploy) => {
                    if let Some(item) = deploy.metadata.name {
                        let mut deploy_vec = deployments_list.lock().unwrap();
                        deploy_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => {
                eprintln!("Deployment watch error: {:?}", e);
            }
        }
    }
}
