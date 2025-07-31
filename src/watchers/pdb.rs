use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::policy::v1::PodDisruptionBudget, apimachinery::pkg::apis::meta::v1::Time};
use kube::Client;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::{Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct PodDisruptionBudgetItem {
    pub name: String,
    pub min_available: Option<String>,
    pub max_unavailable: Option<String>,
    pub allowed_disruptions: i32,
    pub current_healthy: i32,
    pub desired_healthy: i32,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_pdb(pdb: PodDisruptionBudget) -> Option<PodDisruptionBudgetItem> {
    let metadata = &pdb.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp = metadata.creation_timestamp.clone();

    let spec = pdb.spec?;
    let status = pdb.status?;

    let namespace = pdb.metadata.namespace.clone();

    Some(PodDisruptionBudgetItem {
        name,
        min_available: spec.min_available.map(|v| match v {
            IntOrString::Int(i) => i.to_string(),
            IntOrString::String(s) => s,
        }),
        max_unavailable: spec.max_unavailable.map(|v| match v {
            IntOrString::Int(i) => i.to_string(),
            IntOrString::String(s) => s,
        }),
        allowed_disruptions: status.disruptions_allowed,
        current_healthy: status.current_healthy,
        desired_healthy: status.desired_healthy,
        creation_timestamp,
        namespace,
    })
}

pub async fn watch_pod_disruption_budgets(client: Arc<Client>, list: Arc<Mutex<Vec<PodDisruptionBudgetItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<PodDisruptionBudget> = Api::all(client.as_ref().clone());
    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(pdb) => {
                    if let Some(item) = convert_pdb(pdb) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list_guard = list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(pdb) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_pdb(pdb) {
                        let mut list = list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(pdb) => {
                    if let Some(item) = pdb.metadata.name {
                        let mut pdb_vec = list.lock().unwrap();
                        pdb_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("PDB watch error: {:?}", e),
        }
    }
}
