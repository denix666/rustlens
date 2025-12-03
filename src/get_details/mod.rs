pub mod get_node_details;
pub use get_node_details::*;

pub mod get_pod_details;
pub use get_pod_details::*;

pub mod get_deployment_details;
pub use get_deployment_details::*;

pub mod get_daemonset_details;
pub use get_daemonset_details::*;

pub mod get_pvc_details;
pub use get_pvc_details::*;

pub mod get_crd_details;
pub use get_crd_details::*;

pub mod get_rb_details;
pub use get_rb_details::*;

pub mod get_cluster_rb_details;
pub use get_cluster_rb_details::*;

pub mod get_pv_details;
pub use get_pv_details::*;

pub mod get_statefulset_details;
pub use get_statefulset_details::*;

pub mod get_role_details;
pub use get_role_details::*;

pub mod get_leases_details;
pub use get_leases_details::*;

pub mod get_cluster_role_details;
pub use get_cluster_role_details::*;

pub mod get_service_account_details;
pub use get_service_account_details::*;

pub mod get_replicaset_details;
pub use get_replicaset_details::*;

pub mod get_cronjob_details;
pub use get_cronjob_details::*;

pub mod get_job_details;
pub use get_job_details::*;

pub mod get_ingress_details;
pub use get_ingress_details::*;

pub mod get_confirmap_details;
pub use get_confirmap_details::*;

pub mod get_secret_details;
pub use get_secret_details::*;

pub mod get_service_details;
pub use get_service_details::*;

pub mod get_endpoint_details;
pub use get_endpoint_details::*;

pub mod get_cr_instances;
pub use get_cr_instances::*;

pub mod get_k8s_released_version;
pub use get_k8s_released_version::*;
