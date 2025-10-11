use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{core::v1::Pod}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher};

#[derive(Clone)]
pub struct ContainerStatusItem {
    pub name: String,
    pub state: Option<String>, // e.g. "Running", "Terminated", "Waiting"
    pub message: Option<String>,
}

#[derive(Clone)]
pub struct PodItem {
    pub name: String,
    pub phase: Option<String>,
    pub ready_containers: u32,
    pub total_containers: u32,
    pub containers: Vec<ContainerStatusItem>,
    pub restart_count: i32,
    pub node_name: Option<String>,
    pub pod_has_crashloop: bool,
    pub creation_timestamp: Option<Time>,
    pub terminating: bool,
    pub controller: Option<String>,
    pub namespace: Option<String>,
    pub qos_class: Option<String>,
}

fn convert_pod(pod: Pod) -> Option<PodItem> {
    let name = pod.metadata.name?;
    let phase = pod.status.as_ref().and_then(|s| s.phase.clone());
    let node_name = pod.spec.as_ref().and_then(|s| s.node_name.clone());
    let terminating = pod.metadata.deletion_timestamp.is_some();
    let namespace = pod.metadata.namespace.clone();
    let mut containers = vec![];
    let mut ready = 0;
    let mut restart_count = 0;
    let mut pod_has_crashloop = false;
    let qos_class = pod.status.as_ref().and_then(|s| s.qos_class.clone());

    let controller = pod.metadata.owner_references.as_ref()
        .and_then(|owners| {
            owners.iter()
                .find(|o| o.controller.unwrap_or(false))
                .map(|o| format!("{}", o.kind)) // can be name, uid etc...
        });

    if let Some(statuses) = pod.status.as_ref().and_then(|s| s.container_statuses.clone()) {
        for cs in statuses {
            let state = cs.state.as_ref().and_then(|s| {
                if s.running.is_some() {
                    Some("Running".to_string())
                } else if s.waiting.is_some() {
                    Some("Waiting".to_string())
                } else if s.terminated.is_some() {
                    Some("Terminated".to_string())
                } else {
                    None
                }
            });

            let is_crashloop = cs.state.as_ref()
                .and_then(|s| s.waiting.as_ref())
                .and_then(|w| w.reason.clone())
                .map(|r| r == "CrashLoopBackOff")
                .unwrap_or(false);

            if is_crashloop {
                pod_has_crashloop = true;
            }

            restart_count += cs.restart_count;

            let message = cs.state.as_ref().and_then(|s| {
                if let Some(waiting) = &s.waiting {
                    waiting.message.clone()
                } else if let Some(terminated) = &s.terminated {
                    terminated.message.clone()
                } else {
                    None
                }
            });

            if cs.ready {
                ready += 1;
            }

            containers.push(ContainerStatusItem {
                name: cs.name,
                state,
                message,
            });
        }
    }
    Some(PodItem {
        name,
        phase,
        ready_containers: ready,
        total_containers: containers.len() as u32,
        containers,
        restart_count,
        node_name,
        pod_has_crashloop,
        creation_timestamp: pod.metadata.creation_timestamp,
        terminating,
        controller,
        namespace,
        qos_class,
    })
}

pub async fn watch_pods(client: Arc<Client>, pods_list: Arc<Mutex<Vec<PodItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Pod> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = pods_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_pod).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(pod) => {
                    if let Some(item) = convert_pod(pod) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = pods_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;
                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(pod) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_pod(pod) {
                        let mut list = pods_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|p| p.name == item.name && p.namespace == item.namespace) {
                            *existing = item;
                        } else {
                            list.push(item);
                        }
                    }
                }
                watcher::Event::Delete(pod) => {
                    if !initialized {
                        continue;
                    }
                    if let (Some(name), Some(namespace)) = (pod.metadata.name, pod.metadata.namespace) {
                        let mut list = pods_list.lock().unwrap();
                        list.retain(|item| !(item.name == name && item.namespace.as_ref() == Some(&namespace)));
                    }
                }
            },
            Err(e) => eprintln!("Pod watch error: {:?}", e),
        }
    }
}
