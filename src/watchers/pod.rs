use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::core::v1::{Container, Pod}, apimachinery::pkg::apis::meta::v1::Time};
use kube::Client;
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
    pub cpu_request: f32,
    pub mem_request: f32,
    pub cpu_limit: f32,
    pub mem_limit: f32,
}

#[derive(Default)]
struct PodResourceTotals {
    cpu_request: f32,
    mem_request: f32,
    cpu_limit: f32,
    mem_limit: f32,
}

fn parse_cpu_quantity(value: &str) -> Option<f32> {
    if let Some(value) = value.strip_suffix('m') {
        value.parse::<f32>().ok().map(|value| value / 1_000.0)
    } else if let Some(value) = value.strip_suffix('u') {
        value.parse::<f32>().ok().map(|value| value / 1_000_000.0)
    } else if let Some(value) = value.strip_suffix('n') {
        value.parse::<f32>().ok().map(|value| value / 1_000_000_000.0)
    } else {
        value.parse::<f32>().ok()
    }
}

fn parse_memory_quantity(value: &str) -> Option<f32> {
    let (number, multiplier) = if let Some(value) = value.strip_suffix("Ki") {
        (value, 1_024.0)
    } else if let Some(value) = value.strip_suffix("Mi") {
        (value, 1_048_576.0)
    } else if let Some(value) = value.strip_suffix("Gi") {
        (value, 1_073_741_824.0)
    } else if let Some(value) = value.strip_suffix("Ti") {
        (value, 1_099_511_627_776.0)
    } else if let Some(value) = value.strip_suffix("Pi") {
        (value, 1_125_899_906_842_624.0)
    } else if let Some(value) = value.strip_suffix("Ei") {
        (value, 1_152_921_504_606_846_976.0)
    } else if let Some(value) = value.strip_suffix('K') {
        (value, 1_000.0)
    } else if let Some(value) = value.strip_suffix('M') {
        (value, 1_000_000.0)
    } else if let Some(value) = value.strip_suffix('G') {
        (value, 1_000_000_000.0)
    } else if let Some(value) = value.strip_suffix('T') {
        (value, 1_000_000_000_000.0)
    } else if let Some(value) = value.strip_suffix('P') {
        (value, 1_000_000_000_000_000.0)
    } else if let Some(value) = value.strip_suffix('E') {
        (value, 1_000_000_000_000_000_000.0)
    } else {
        (value, 1.0)
    };

    number
        .parse::<f32>()
        .ok()
        .map(|number| number * multiplier / 1_073_741_824.0)
}

fn container_resource(container: &Container, resource: &str, limits: bool) -> f32 {
    let Some(resources) = container.resources.as_ref() else {
        return 0.0;
    };

    let quantity = if limits {
        resources.limits.as_ref().and_then(|values| values.get(resource))
    } else {
        resources.requests.as_ref().and_then(|values| values.get(resource))
    };

    match (resource, quantity) {
        ("cpu", Some(quantity)) => parse_cpu_quantity(&quantity.0).unwrap_or(0.0),
        ("memory", Some(quantity)) => parse_memory_quantity(&quantity.0).unwrap_or(0.0),
        _ => 0.0,
    }
}

fn sum_container_resources(containers: &[Container]) -> PodResourceTotals {
    containers.iter().fold(PodResourceTotals::default(), |mut total, container| {
        total.cpu_request += container_resource(container, "cpu", false);
        total.mem_request += container_resource(container, "memory", false);
        total.cpu_limit += container_resource(container, "cpu", true);
        total.mem_limit += container_resource(container, "memory", true);
        total
    })
}

fn pod_resource_totals(pod: &Pod) -> PodResourceTotals {
    let Some(spec) = pod.spec.as_ref() else {
        return PodResourceTotals::default();
    };

    let app_resources = sum_container_resources(&spec.containers);
    let init_resources = spec.init_containers.as_ref()
        .map(|containers| {
            containers.iter().fold(PodResourceTotals::default(), |mut max_resources, container| {
                max_resources.cpu_request = max_resources.cpu_request.max(container_resource(container, "cpu", false));
                max_resources.mem_request = max_resources.mem_request.max(container_resource(container, "memory", false));
                max_resources.cpu_limit = max_resources.cpu_limit.max(container_resource(container, "cpu", true));
                max_resources.mem_limit = max_resources.mem_limit.max(container_resource(container, "memory", true));
                max_resources
            })
        })
        .unwrap_or_default();

    let overhead_cpu = spec.overhead.as_ref()
        .and_then(|resources| resources.get("cpu"))
        .and_then(|quantity| parse_cpu_quantity(&quantity.0))
        .unwrap_or(0.0);
    let overhead_memory = spec.overhead.as_ref()
        .and_then(|resources| resources.get("memory"))
        .and_then(|quantity| parse_memory_quantity(&quantity.0))
        .unwrap_or(0.0);

    PodResourceTotals {
        cpu_request: app_resources.cpu_request.max(init_resources.cpu_request) + overhead_cpu,
        mem_request: app_resources.mem_request.max(init_resources.mem_request) + overhead_memory,
        cpu_limit: app_resources.cpu_limit.max(init_resources.cpu_limit),
        mem_limit: app_resources.mem_limit.max(init_resources.mem_limit),
    }
}

fn convert_pod(pod: Pod) -> Option<PodItem> {
    let resource_totals = pod_resource_totals(&pod);
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
                .map(|o| o.kind.to_string()) // can be name, uid etc...
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
        cpu_request: resource_totals.cpu_request,
        mem_request: resource_totals.mem_request,
        cpu_limit: resource_totals.cpu_limit,
        mem_limit: resource_totals.mem_limit,
    })
}

pub async fn watch_pods(client: Arc<Client>, pods_list: Arc<Mutex<Vec<PodItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Pod> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);


    let mut stream = watcher(api, watcher::Config::default().page_size(crate::WATCHER_PAGE_SIZE)).boxed();

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
            Err(e) => log::error!("Pod watch error: {:?}", e),
        }
    }
}
