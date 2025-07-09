use std::collections::BTreeMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

#[derive(Clone)]
pub struct NodeItem {
    pub name: String,
    pub status: String, // "Ready", "NotReady", "Unknown"
    pub roles: Vec<String>,
    pub scheduling_disabled: bool,
    pub taints: Option<Vec<k8s_openapi::api::core::v1::Taint>>,
    pub cpu_percent: f32,
    pub mem_percent: f32,
    pub storage: Option<String>,
    pub creation_timestamp: Option<Time>,
}

#[derive(Clone)]
pub struct ContainerStatusItem {
    pub name: String,
    pub state: Option<String>, // e.g. "Running", "Terminated", "Waiting"
    pub message: Option<String>,
}

#[derive(Clone)]
pub struct PodItem {
    pub name: String,
    pub creation_timestamp: Option<Time>,
    pub phase: Option<String>,
    pub ready_containers: u32,
    pub total_containers: u32,
    pub containers: Vec<ContainerStatusItem>,
    pub restart_count: i32,
    pub node_name: Option<String>,
    pub pod_has_crashloop: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct JobItem {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub completions: i32,
    pub conditions: Vec<String>,
    pub creation_timestamp: Option<Time>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ReplicaSetItem {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub desired: i32,
    pub current: i32,
    pub ready: i32,
    pub creation_timestamp: Option<Time>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StorageClassItem {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub provisioner: String,
    pub reclaim_policy: String,
    pub volume_binding_mode: String,
    pub is_default: String,
    pub creation_timestamp: Option<Time>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PvcItem {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub storage_class: String,
    pub size: String,
    pub volume_name: String,
    pub status: String,
    pub creation_timestamp: Option<Time>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PvItem {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub storage_class: String,
    pub capacity: String,
    pub claim: String,
    pub status: String,
    pub creation_timestamp: Option<Time>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StatefulSetItem {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub replicas: i32,
    pub service_name: String,
    pub ready_replicas: i32,
    pub creation_timestamp: Option<Time>,
}

#[derive(Clone, Debug)]
pub struct SecretItem {
    pub name: String,
    pub labels: String,
    pub keys: String,
    pub type_: String,
    pub creation_timestamp: Option<Time>,
}

#[derive(Debug, Clone)]
pub struct ConfigMapItem {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub keys: Vec<String>,
    pub type_: String,
    pub creation_timestamp: Option<Time>,
}

#[derive(Clone)]
pub struct EventItem {
    pub message: String,
    pub reason: String,
    pub involved_object: String,
    pub event_type: String,
    pub timestamp: String,
    pub namespace: String,
    pub creation_timestamp: Option<Time>,
}

#[derive(Clone)]
pub struct NamespaceItem {
    pub name: String,
    pub creation_timestamp: Option<Time>,
    pub phase: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
}

#[derive(Clone)]
pub struct DeploymentItem {
    pub name: String,
    pub namespace: String,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub updated_replicas: i32,
    pub replicas: i32,
    pub creation_timestamp: Option<Time>,
}
