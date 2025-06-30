use kube::{Api, Client};
use kube::runtime::watcher;
use k8s_openapi::api::core::v1::{Namespace, Node, Pod, Event};
use std::sync::{Arc, Mutex};
use futures_util::StreamExt;
use serde_json::json;
use kube::api::{Patch, PatchParams};
//use k8s_metrics::v1beta1::NodeMetrics;
//use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

pub fn load_embedded_icon() -> Result<crate::egui::IconData, String> {
    let img = image::load_from_memory(super::ICON_BYTES).map_err(|e| e.to_string())?.into_rgba8();
    let (width, height) = img.dimensions();
    let rgba = img.into_raw();
    Ok(crate::egui::IconData { rgba, width, height })
}

pub async fn cordon_node(node_name: &str, cordoned: bool) -> Result<(), kube::Error> {
    let client = Client::try_default().await?;
    let nodes: Api<Node> = Api::all(client);

    let patch = json!({
        "spec": {
            "unschedulable": cordoned
        }
    });

    nodes
        .patch(node_name, &PatchParams::apply("rustlens"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}


// fn parse_quantity_to_f64(q: &Quantity) -> Option<f64> {
//     let s = &q.0;
//     if s.ends_with("n") {
//         s[..s.len()-1].parse::<f64>().ok().map(|v| v / 1e9)
//     } else if s.ends_with("u") {
//         s[..s.len()-1].parse::<f64>().ok().map(|v| v / 1e6)
//     } else if s.ends_with("m") {
//         s[..s.len()-1].parse::<f64>().ok().map(|v| v / 1000.0)
//     } else {
//         s.parse::<f64>().ok()
//     }
// }

// fn percentage(used: f64, allocatable: f64) -> u8 {
//     ((used / allocatable) * 100.0).round().min(100.0) as u8
// }

pub async fn watch_nodes(nodes_list: Arc<Mutex<Vec<super::NodeItem>>>) {
    let client = Client::try_default().await.unwrap();
    let api: Api<Node> = Api::all(client);
    let mut node_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    let percent = |alloc: &str, cap: &str| -> Option<u8> {
        let parse_quantity = |q: &str| -> Option<f64> {
            // very naive parser
            if q.ends_with("m") {
                q.trim_end_matches('m').parse::<f64>().ok().map(|v| v / 1000.0)
            } else if q.ends_with("Ki") {
                q.trim_end_matches("Ki").parse::<f64>().ok().map(|v| v * 1024.0)
            } else if q.ends_with("Mi") {
                q.trim_end_matches("Mi").parse::<f64>().ok().map(|v| v * 1024.0 * 1024.0)
            } else {
                q.parse::<f64>().ok()
            }
        };

        let a = parse_quantity(alloc)?;
        let c = parse_quantity(cap)?;
        if c > 0.0 {
            Some((100.0 * (1.0 - a / c)).round() as u8)
        } else {
            None
        }
    };

    while let Some(event) = node_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(node) => {
                    if let Some(name) = node.metadata.name {
                        let status = node
                            .status
                            .as_ref()
                            .and_then(|s| s.conditions.as_ref())
                            .and_then(|conds| {
                                conds.iter().find(|c| c.type_ == "Ready").and_then(|c| {
                                    match c.status.as_str() {
                                        "True" => Some("Ready"),
                                        "False" => Some("NotReady"),
                                        _ => Some("Unknown"),
                                    }
                                })
                            })
                            .unwrap_or("Unknown")
                            .to_string();

                        let roles = node.metadata.labels.unwrap_or_default()
                            .iter()
                            .filter_map(|(key, _)| {
                                key.strip_prefix("node-role.kubernetes.io/").map(|s| s.to_string())
                            })
                            .collect::<Vec<_>>();

                        let scheduling_disabled = node.spec.as_ref().and_then(|spec| spec.unschedulable).unwrap_or(false);
                        let taints = node.spec.as_ref().and_then(|spec| spec.taints.clone());

                        let alloc = node.status.as_ref().unwrap().allocatable.as_ref().unwrap();
                        let cap = node.status.as_ref().unwrap().capacity.as_ref().unwrap();
                        let cpu_percent = percent(alloc.get("cpu").unwrap().0.as_str(), cap.get("cpu").unwrap().0.as_str()).unwrap_or(0) as f32;
                        let mem_percent = percent(alloc.get("memory").unwrap().0.as_str(), cap.get("memory").unwrap().0.as_str()).unwrap_or(0) as f32;

                        let storage = node.status.as_ref()
                            .and_then(|s| s.allocatable.as_ref())
                            .and_then(|res| res.get("ephemeral-storage"))
                            .map(|q| q.0.clone());

                        initial.push(super::NodeItem {
                            name,
                            status,
                            roles,
                            scheduling_disabled,
                            taints,
                            cpu_percent,
                            mem_percent,
                            storage,
                        });
                    }
                }
                watcher::Event::InitDone => {
                    let mut nodes = nodes_list.lock().unwrap();
                    *nodes = initial.clone();
                    initialized = true;
                }
                watcher::Event::Apply(node) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(name) = node.metadata.name {
                        let status = node
                            .status
                            .as_ref()
                            .and_then(|s| s.conditions.as_ref())
                            .and_then(|conds| {
                                conds.iter().find(|c| c.type_ == "Ready").and_then(|c| {
                                    match c.status.as_str() {
                                        "True" => Some("Ready"),
                                        "False" => Some("NotReady"),
                                        _ => Some("Unknown"),
                                    }
                                })
                            })
                            .unwrap_or("Unknown")
                            .to_string();

                        let roles = node.metadata.labels.unwrap_or_default()
                            .iter()
                            .filter_map(|(key, _)| {
                                key.strip_prefix("node-role.kubernetes.io/").map(|s| s.to_string())
                            })
                            .collect::<Vec<_>>();

                        let scheduling_disabled = node.spec.as_ref().and_then(|spec| spec.unschedulable).unwrap_or(false);
                        let taints = node.spec.as_ref().and_then(|spec| spec.taints.clone());

                        let mut nodes = nodes_list.lock().unwrap();

                        let alloc = node.status.as_ref().unwrap().allocatable.as_ref().unwrap();
                        let cap = node.status.as_ref().unwrap().capacity.as_ref().unwrap();
                        let cpu_percent = percent(alloc.get("cpu").unwrap().0.as_str(), cap.get("cpu").unwrap().0.as_str()).unwrap_or(0) as f32;
                        let mem_percent = percent(alloc.get("memory").unwrap().0.as_str(), cap.get("memory").unwrap().0.as_str()).unwrap_or(0) as f32;

                        // DEBUG
                        //let cpu_percent = 44.0;
                        //let mem_percent = 44.0;

                        let storage = node.status.as_ref()
                            .and_then(|s| s.allocatable.as_ref())
                            .and_then(|res| res.get("ephemeral-storage"))
                                .map(|q| q.0.clone());

                        if let Some(existing) = nodes.iter_mut().find(|n| n.name == name) {
                            // renew existing node
                            existing.status = status;
                            existing.taints = taints;
                            existing.roles = roles;
                            existing.cpu_percent = cpu_percent;
                            existing.mem_percent = mem_percent;
                            existing.storage = storage;
                            existing.scheduling_disabled = scheduling_disabled;
                        } else {
                            // add new node
                            nodes.push(super::NodeItem {
                                name,
                                status,
                                roles,
                                scheduling_disabled,
                                taints,
                                cpu_percent,
                                mem_percent,
                                storage,
                            });
                        }
                    }
                }
                watcher::Event::Delete(node) => {
                    if let Some(name) = node.metadata.name {
                        let mut nodes = nodes_list.lock().unwrap();
                        nodes.retain(|n| n.name != name);
                    }
                }
            },
            Err(e) => {
                eprintln!("Node watch error: {:?}", e);
            }
        }
    }
}

pub async fn watch_events(events_list: Arc<Mutex<Vec<super::EventItem>>>) {
    let client = Client::try_default().await.unwrap();
    let api: Api<Event> = Api::all(client);
    let mut event_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(ev) => {
                    if let Some(message) = ev.message.clone() {
                        let reason = ev.reason.clone().unwrap_or_else(|| "Unknown".to_string());
                        let involved = format!(
                            "{}/{}",
                            ev.involved_object.kind.clone().unwrap_or_else(|| "Unknown".to_string()),
                            ev.involved_object.name.clone().unwrap_or_else(|| "Unknown".to_string())
                        );
                        let type_ = ev.type_.clone().unwrap_or_else(|| "Normal".to_string());
                        let timestamp = ev.event_time
                            .as_ref()
                            .map(|t| t.0.to_rfc3339())
                            .or_else(|| ev.last_timestamp.as_ref().map(|t| t.0.to_rfc3339()))
                            .unwrap_or_else(|| "N/A".to_string());

                        let namespace = ev.involved_object.namespace.clone().unwrap_or_else(|| "default".to_string());

                        initial.push(super::EventItem {
                            message,
                            reason,
                            involved_object: involved,
                            event_type: type_,
                            timestamp,
                            namespace,
                        });
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = events_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;
                }
                watcher::Event::Apply(ev) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(message) = ev.message.clone() {
                        let reason = ev.reason.clone().unwrap_or_else(|| "Unknown".to_string());
                        let involved = format!(
                            "{}/{}",
                            ev.involved_object.kind.clone().unwrap_or_else(|| "Unknown".to_string()),
                            ev.involved_object.name.clone().unwrap_or_else(|| "Unknown".to_string())
                        );
                        let type_ = ev.type_.clone().unwrap_or_else(|| "Normal".to_string());
                        let timestamp = ev.event_time
                            .as_ref()
                            .map(|t| t.0.to_rfc3339())
                            .or_else(|| ev.last_timestamp.as_ref().map(|t| t.0.to_rfc3339()))
                            .unwrap_or_else(|| "N/A".to_string());

                        let mut list = events_list.lock().unwrap();
                        // check and clear length before adding
                        let list_len = list.len();
                        if list_len >= 500 {
                            list.drain(0..list_len - 499);
                        }
                        let namespace = ev.involved_object.namespace.clone().unwrap_or_else(|| "default".to_string());

                        list.push(super::EventItem {
                            message,
                            reason,
                            involved_object: involved,
                            event_type: type_,
                            timestamp,
                            namespace,
                        });
                    }
                }
                watcher::Event::Delete(_) => {
                    // we can ignore delete events
                }
            },
            Err(e) => {
                eprintln!("Event watch error: {:?}", e);
            }
        }
    }
}

pub async fn watch_namespaces(ns_list: Arc<Mutex<Vec<super::NamespaceItem>>>) {
    let client = Client::try_default().await.unwrap();
    let api: Api<Namespace> = Api::all(client);
    let mut ns_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = ns_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(ns) => {
                    if let Some(name) = ns.metadata.name {
                        initial.push(super::NamespaceItem { name });
                    }
                }
                watcher::Event::InitDone => {
                    let mut ns_vec = ns_list.lock().unwrap();
                    *ns_vec = initial.clone();
                    initialized = true;
                }
                watcher::Event::Apply(ns) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(name) = ns.metadata.name {
                        let mut ns_vec = ns_list.lock().unwrap();
                        if !ns_vec.iter().any(|n| n.name == name) {
                            ns_vec.push(super::NamespaceItem { name });
                        }
                    }
                }
                watcher::Event::Delete(ns) => {
                    if let Some(name) = ns.metadata.name {
                        let mut ns_vec = ns_list.lock().unwrap();
                        ns_vec.retain(|n| n.name != name);
                    }
                }
            },
            Err(e) => {
                eprintln!("Namespace watch error: {:?}", e);
            }
        }
    }
}

pub async fn watch_pods(pods_list: Arc<Mutex<Vec<super::PodItem>>>, selected_ns: String) {
    let client = Client::try_default().await.unwrap();
    let api: Api<Pod> = Api::namespaced(client, &selected_ns);
    let mut pod_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = pod_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(pod) => {
                    if let Some(name) = pod.metadata.name {
                        initial.push(super::PodItem { name });
                    }
                }
                watcher::Event::InitDone => {
                    let mut pods_vec = pods_list.lock().unwrap();
                    *pods_vec = initial.clone();
                    initialized = true;
                }
                watcher::Event::Apply(pod) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(name) = pod.metadata.name {
                        let mut pods_vec = pods_list.lock().unwrap();
                        if !pods_vec.iter().any(|p| p.name == name) {
                            pods_vec.push(super::PodItem { name });
                        }
                    }
                }
                watcher::Event::Delete(pod) => {
                    if let Some(name) = pod.metadata.name {
                        let mut ns_vec = pods_list.lock().unwrap();
                        ns_vec.retain(|p| p.name != name);
                    }
                }
            },
            Err(e) => {
                eprintln!("Pods watch error: {:?}", e);
            }
        }
    }
}
