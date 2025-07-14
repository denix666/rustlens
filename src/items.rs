use std::collections::BTreeMap;
use k8s_openapi::apimachinery::pkg::{apis::meta::v1::Time};

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

#[derive(Debug, Clone)]
pub struct EndpointItem {
    pub name: String,
    pub addresses: String,
    pub ports: String,
    pub creation_timestamp: Option<Time>,
}

#[derive(Debug, Clone)]
pub struct NetworkPolicyItem {
    pub name: String,
    pub pod_selector: String,
    pub policy_types: String,
    pub creation_timestamp: Option<Time>,
}

#[derive(Debug, Clone)]
pub struct CRDItem {
    pub name: String,
    pub group: String,
    pub version: String,
    pub scope: String,
    pub kind: String,
    pub creation_timestamp: Option<Time>,
}

#[derive(Clone)]
pub struct ContainerStatusItem {
    pub name: String,
    pub state: Option<String>, // e.g. "Running", "Terminated", "Waiting"
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IngressItem {
    pub name: String,
    pub host: String,
    pub paths: String,
    pub service: String,
    pub tls: String,
    pub creation_timestamp: Option<Time>,
}

#[derive(Debug, Clone)]
pub struct CronJobItem {
    pub name: String,
    pub schedule: String,
    pub suspend: String,
    pub active: usize,
    pub last_schedule: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
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
}

#[derive(Debug, Clone)]
pub struct CSIDriverItem {
    pub name: String,
    pub attach_required: String,
    pub pod_info_on_mount: String,
    pub storage_capacity: String,
    pub fs_group_policy: String,
    pub creation_timestamp: Option<Time>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct JobItem {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub completions: i32,
    pub condition: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
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

#[derive(Debug, Clone)]
pub struct ServiceItem {
    pub name: String,
    pub svc_type: String,
    pub cluster_ip: String,
    pub ports: String,
    pub external_ip: String,
    pub selector: String,
    pub creation_timestamp: Option<Time>,
    pub status: String,
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
    pub namespace: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SecretItem {
    pub name: String,
    pub labels: String,
    pub keys: String,
    pub type_: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigMapItem {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub keys: Vec<String>,
    pub type_: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
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

#[derive(Debug, Clone)]
pub struct DaemonSetItem {
    pub name: String,
    pub desired: i32,
    pub current: i32,
    pub ready: i32,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

#[derive(Clone, PartialEq)]
pub struct NamespaceItem {
    pub name: String,
    pub creation_timestamp: Option<Time>,
    pub phase: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
}

#[derive(Clone)]
pub struct DeploymentItem {
    pub name: String,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub updated_replicas: i32,
    pub replicas: i32,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}
