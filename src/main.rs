mod ui;
use ui::*;

mod watchers;
use watchers::*;

mod theme;
use theme::*;

mod functions;
use functions::*;

mod get_details;
use get_details::*;

use eframe::egui::{CursorIcon};
use eframe::*;
use egui::{Color32, Context, FontId, TextStyle};
use std::collections::BTreeMap;
use std::f32;
use std::sync::{Arc, Mutex};
use kube::{Client};
use std::sync::atomic::{AtomicBool, Ordering};

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");
const ACTIONS_MENU_BUTTON_SIZE: f32 = 10.0;
const ACTIONS_MENU_LABEL: &str = "🔻";
const MAX_LOG_LINES: usize = 600;

#[derive(PartialEq)]
enum SortBy {
    Name,
    Age,
}

#[derive(PartialEq, Clone)]
enum Category {
    ClusterOverview,
    Nodes,
    Secrets,
    Namespaces,
    Pods,
    Deployments,
    Events,
    ConfigMaps,
    StatefulSets,
    ReplicaSets,
    Jobs,
    CronJobs,
    Services,
    Endpoints,
    Ingresses,
    PersistentVolumeClaims,
    PersistentVolumes,
    StorageClasses,
    CSIDrivers,
    DaemonSets,
    PodDisruptionBudgets,
    NetworkPolicies,
    CustomResourcesDefinitions,
    HelmReleases,
    About,
    Roles,
    SeriviceAccounts,
    ClusterRoles,
    ClusterRoleBindings,
    RoleBindings,
}

#[derive(Clone)]
struct ClusterInfo {
    name: String,
}

#[derive(Clone, PartialEq)]
enum ResourceType {
    Blank,
    NameSpace,
    PersistenceVolumeClaim,
    Pod,
    PodWithPvc,
    Secret,
    ExternalSecret,
    ServiceAccount,
    Role,
    ClusterRole,
    ConfigMap,
    DaemonSet,
    ReplicaSet,
    Ingress,
    Service,
    Deployment,
}

#[tokio::main]
async fn main() {
    let mut title = String::from("RustLens v");
    title.push_str(env!("CARGO_PKG_VERSION"));
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 800.0])
            .with_maximized(true),
        ..Default::default()
    };

    if let Ok(icon) = load_embedded_icon() {
        options.viewport = options.viewport.with_icon(icon);
    }

    let mut new_resource_window = ui::new_resource::NewResourceWindow::new();
    let mut scale_window = ui::scale::ScaleWindow::new();
    let mut node_details_window = ui::node_details::NodeDetailsWindow::new();
    let mut pod_details_window = ui::pod_details::PodDetailsWindow::new();
    let mut deployment_details_window = ui::deployment_details::DeploymentDetailsWindow::new();
    let mut daemonset_details_window = ui::daemonset_details::DaemonSetDetailsWindow::new();
    let mut replicaset_details_window = ui::replicaset_details::ReplicaSetDetailsWindow::new();
    let mut statefulset_details_window = ui::statefulset_details::StatefulSetDetailsWindow::new();
    let mut configmap_details_window = ui::configmap_details::ConfigMapDetailsWindow::new();
    let mut job_details_window = ui::job_details::JobDetailsWindow::new();
    let mut pvc_details_window = ui::pvc_details::PvcDetailsWindow::new();
    let mut pv_details_window = ui::pv_details::PvDetailsWindow::new();
    let mut cronjob_details_window = ui::cronjob_details::CronJobDetailsWindow::new();
    let mut service_details_window = ui::service_details::ServiceDetailsWindow::new();
    let mut service_account_details_window = ui::service_account_details::ServiceAccountDetailsWindow::new();
    let mut role_details_window = ui::role_details::RoleDetailsWindow::new();
    //let mut crd_details_window = ui::crd_details::CrdDetailsWindow::new();
    let mut cluster_role_details_window = ui::cluster_role_details::ClusterRoleDetailsWindow::new();
    let mut ingress_details_window = ui::ingress_details::IngressDetailsWindow::new();
    let mut endpoint_details_window = ui::endpoint_details::EndpointDetailsWindow::new();
    let mut secret_details_window = ui::secret_details::SecretDetailsWindow::new();
    let log_window = Arc::new(Mutex::new(ui::logs::LogWindow::new()));
    let yaml_editor_window = Arc::new(Mutex::new(ui::yaml_editor::YamlEditorWindow::new()));
    let mut decoder_window = ui::decoder::DecoderWindow::new();
    // let cr_groups_list: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
    // let cr_items_list: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));

    let cr_grouped_list = Arc::new(Mutex::new(BTreeMap::<String, Vec<String>>::new()));

    //####################################################//
    let mut sort_by = SortBy::Age;
    let mut sort_asc = false;

    let ctx_info = get_current_context_info().unwrap();
    let cluster_name = ctx_info.context.unwrap().cluster;
    let user_name = ctx_info.name;

    let mut confirmation_dialog = DeleteConfirmation::new();

    let selected_category = Arc::new(Mutex::new(Category::ClusterOverview));
    let selected_category_ui = Arc::clone(&selected_category);

    let selected_namespace = Arc::new(Mutex::new(None::<String>));
    let selected_namespace_clone = Arc::clone(&selected_namespace);

    let mut filter_namespaces = String::new();
    let mut filter_nodes = String::new();
    let mut filter_pods = String::new();
    let mut filter_events = String::new();
    let mut filter_deployments = String::new();
    let mut filter_replicasets = String::new();
    let mut filter_secrets = String::new();
    let mut filter_roles = String::new();
    let mut filter_cluster_roles = String::new();
    let mut filter_statefulsets = String::new();
    let mut filter_jobs = String::new();
    let mut filter_pvcs = String::new();
    let mut filter_pvs = String::new();
    let mut filter_service_accounts = String::new();
    let mut filter_scs = String::new();
    let mut filter_csi_drivers = String::new();
    let mut filter_services = String::new();
    let mut filter_configmaps = String::new();
    let mut filter_endpoints = String::new();
    let mut filter_ingresses = String::new();
    let mut filter_cronjobs = String::new();
    let mut filter_daemonsets = String::new();
    let mut filter_pdbs = String::new();
    let mut filter_network_policies = String::new();
    let mut filter_crds = String::new();
    let mut filter_helm_releases = String::new();


    // Client connection
    let client = Arc::new(Client::try_default().await.unwrap());

    // CLUSTER INFO - (rework)
    let cluster_info = Arc::new(Mutex::new(ClusterInfo {
        name: "unknown".to_string(),
    }));
    let cluster_info_ui = Arc::clone(&cluster_info);
    let cluster_info_bg = Arc::clone(&cluster_info);
    tokio::spawn(async move {
        if let Ok(name) = get_cluster_name().await {
            *cluster_info_bg.lock().unwrap() = ClusterInfo { name };
        }
    });

    // PODS
    let pods = Arc::new(Mutex::new(Vec::<PodItem>::new()));
    let pod_details = Arc::new(Mutex::new(PodDetails::default()));
    let pods_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&pods),
        Arc::clone(&pods_loading),
        |c, s, l| {
        Box::pin(watch_pods(c, s, l))
    });

    // ENDPOINTS
    let endpoints = Arc::new(Mutex::new(Vec::<EndpointItem>::new()));
    let endpoint_details = Arc::new(Mutex::new(EndpointDetails::default()));
    let endpoints_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&endpoints),
        Arc::clone(&endpoints_loading),
        |c, s, l| {
        Box::pin(watch_endpoints(c, s, l))
    });

    // ROLES
    let roles = Arc::new(Mutex::new(Vec::<RoleItem>::new()));
    let role_details = Arc::new(Mutex::new(RoleDetails::default()));
    let roles_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&roles),
        Arc::clone(&roles_loading),
        |c, s, l| {
        Box::pin(watch_roles(c, s, l))
    });

    // CLUSTER ROLES
    let cluster_roles = Arc::new(Mutex::new(Vec::<ClusterRoleItem>::new()));
    let cluster_role_details = Arc::new(Mutex::new(ClusterRoleDetails::default()));
    let cluster_roles_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&cluster_roles),
        Arc::clone(&cluster_roles_loading),
        |c, s, l| {
        Box::pin(watch_cluster_roles(c, s, l))
    });

    // POD DISRUPTION BUDGET
    let pdbs = Arc::new(Mutex::new(Vec::<PodDisruptionBudgetItem>::new()));
    let pdbs_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&pdbs),
        Arc::clone(&pdbs_loading),
        |c, s, l| {
        Box::pin(watch_pod_disruption_budgets(c, s, l))
    });

    // SERVICE ACCOUNT
    let service_accounts = Arc::new(Mutex::new(Vec::<ServiceAccountItem>::new()));
    let service_account_details = Arc::new(Mutex::new(ServiceAccountDetails::default()));
    let service_accounts_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&service_accounts),
        Arc::clone(&service_accounts_loading),
        |c, s, l| {
        Box::pin(watch_service_accounts(c, s, l))
    });

    // CRONJOBS
    let cronjobs = Arc::new(Mutex::new(Vec::<CronJobItem>::new()));
    let cronjob_details = Arc::new(Mutex::new(CronJobDetails::default()));
    let cronjobs_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&cronjobs),
        Arc::clone(&cronjobs_loading),
        |c, s, l| {
        Box::pin(watch_cronjobs(c, s, l))
    });

    // NETWORK POLICIES
    let network_policies = Arc::new(Mutex::new(Vec::<NetworkPolicyItem>::new()));
    let network_policies_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&network_policies),
        Arc::clone(&network_policies_loading),
        |c, s, l| {
        Box::pin(watch_network_policies(c, s, l))
    });

    // SERVICES
    let services = Arc::new(Mutex::new(Vec::<ServiceItem>::new()));
    let service_details = Arc::new(Mutex::new(ServiceDetails::default()));
    let services_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&services),
        Arc::clone(&services_loading),
        |c, s, l| {
        Box::pin(watch_services(c, s, l))
    });

    // INGRESSES
    let ingresses = Arc::new(Mutex::new(Vec::<IngressItem>::new()));
    let ingress_details = Arc::new(Mutex::new(IngressDetails::default()));
    let ingresses_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&ingresses),
        Arc::clone(&ingresses_loading),
        |c, s, l| {
        Box::pin(watch_ingresses(c, s, l))
    });

    // CRDS
    let crds = Arc::new(Mutex::new(Vec::<CRDItem>::new()));
    //let crd_details = Arc::new(Mutex::new(CrdDetails::default()));
    let crds_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&crds),
        Arc::clone(&crds_loading),
        |c, s, l| {
        Box::pin(watch_crds(c, s, l))
    });

    // CSI DRIVERS
    let csi_drivers = Arc::new(Mutex::new(Vec::<CSIDriverItem>::new()));
    let csi_drivers_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&csi_drivers),
        Arc::clone(&csi_drivers_loading),
        |c, s, l| {
        Box::pin(watch_csi_drivers(c, s, l))
    });

    // PVC
    let pvcs = Arc::new(Mutex::new(Vec::<PvcItem>::new()));
    let pvc_details = Arc::new(Mutex::new(PvcDetails::default()));
    let pvcs_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&pvcs),
        Arc::clone(&pvcs_loading),
        |c, s, l| {
        Box::pin(watch_pvcs(c, s, l))
    });

    // DAEMONSETS
    let daemonsets = Arc::new(Mutex::new(Vec::<DaemonSetItem>::new()));
    let daemonset_details = Arc::new(Mutex::new(DaemonSetDetails::default()));
    let daemonsets_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&daemonsets),
        Arc::clone(&daemonsets_loading),
        |c, s, l| {
        Box::pin(watch_daemonsets(c, s, l))
    });

    // JOBS
    let jobs = Arc::new(Mutex::new(Vec::<JobItem>::new()));
    let job_details = Arc::new(Mutex::new(JobDetails::default()));
    let jobs_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&jobs),
        Arc::clone(&jobs_loading),
        |c, s, l| {
        Box::pin(watch_jobs(c, s, l))
    });

    // PV
    let pvs = Arc::new(Mutex::new(Vec::<PvItem>::new()));
    let pv_details = Arc::new(Mutex::new(PvDetails::default()));
    let pvs_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&pvs),
        Arc::clone(&pvs_loading),
        |c, s, l| {
        Box::pin(watch_pvs(c, s, l))
    });

    // SC
    let storage_classes = Arc::new(Mutex::new(Vec::<StorageClassItem>::new()));
    let storage_classes_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&storage_classes),
        Arc::clone(&storage_classes_loading),
        |c, s, l| {
        Box::pin(watch_storage_classes(c, s, l))
    });

    // EVENTS
    let events = Arc::new(Mutex::new(Vec::<EventItem>::new()));
    let events_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&events),
        Arc::clone(&events_loading),
        |c, s, l| {
        Box::pin(watch_events(c, s, l))
    });

    // STATEFULSETS
    let statefulsets = Arc::new(Mutex::new(Vec::<StatefulSetItem>::new()));
    let statefulset_details = Arc::new(Mutex::new(StatefulSetDetails::default()));
    let statefulsets_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&statefulsets),
        Arc::clone(&statefulsets_loading),
        |c, s, l| {
        Box::pin(watch_statefulsets(c, s, l))
    });

    // REPLICASETS
    let replicasets = Arc::new(Mutex::new(Vec::<ReplicaSetItem>::new()));
    let replicaset_details = Arc::new(Mutex::new(ReplicaSetDetails::default()));
    let replicasets_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&replicasets),
        Arc::clone(&replicasets_loading),
        |c, s, l| {
        Box::pin(watch_replicasets(c, s, l))
    });

    // DEPLOYMENTS
    let deployments = Arc::new(Mutex::new(Vec::<DeploymentItem>::new()));
    let deployment_details = Arc::new(Mutex::new(DeploymentDetails::default()));
    let deployments_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&deployments),
        Arc::clone(&deployments_loading),
        |c, s, l| {
        Box::pin(watch_deployments(c, s, l))
    });

    // SECRETS
    let secrets = Arc::new(Mutex::new(Vec::<SecretItem>::new()));
    let secret_details = Arc::new(Mutex::new(SecretDetails::default()));
    let secrets_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&secrets),
        Arc::clone(&secrets_loading),
        |c, s, l| {
        Box::pin(watch_secrets(c, s, l))
    });

    // CONFIGMAPS
    let configmaps = Arc::new(Mutex::new(Vec::<ConfigMapItem>::new()));
    let configmap_details = Arc::new(Mutex::new(ConfigMapDetails::default()));
    let configmaps_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&configmaps),
        Arc::clone(&configmaps_loading),
        |c, s, l| {
        Box::pin(watch_configmaps(c, s, l))
    });

    // NODES
    let nodes = Arc::new(Mutex::new(Vec::<NodeItem>::new()));
    let node_details = Arc::new(Mutex::new(NodeDetails::new()));
    let nodes_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&nodes),
        Arc::clone(&nodes_loading),
        |c, s, l| {
        Box::pin(watch_nodes(c, s, l))
    });

    // NAMESPACES
    let namespaces = Arc::new(Mutex::new(Vec::<NamespaceItem>::new()));
    let namespaces_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&namespaces),
        Arc::clone(&namespaces_loading),
        |c, s, l| {
        Box::pin(watch_namespaces(c, s, l))
    });

    // HELM RELEASES
    let helm_releases = Arc::new(Mutex::new(Vec::<HelmReleaseItem>::new()));
    let helm_releases_loading = Arc::new(AtomicBool::new(true));

    eframe::run_simple_native(&title, options, move |ctx: &Context, _frame| {
        // Setup style
        let mut style: egui::Style = (*ctx.style()).clone();

        // Increase font size for different TextStyle
        style.text_styles = [
            (TextStyle::Heading, FontId::new(20.0, egui::FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, egui::FontFamily::Monospace)),
            (TextStyle::Monospace, FontId::new(14.0, egui::FontFamily::Monospace)),
            (TextStyle::Button, FontId::new(16.0, egui::FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(13.0, egui::FontFamily::Proportional)),
        ]
        .into();

        ctx.set_style(style);

        egui::SidePanel::left("tasks panel").resizable(false).exact_width(290.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("menu_scroll").show(ui, |ui| {
                let current = selected_category_ui.lock().unwrap().clone();

                egui::CollapsingHeader::new("☸ Cluster").default_open(true).show(ui, |ui| {
                    if ui.selectable_label(*selected_category_ui.lock().unwrap() == Category::ClusterOverview,"🗠 Overview",).clicked() {
                        *selected_category_ui.lock().unwrap() = Category::ClusterOverview;
                    }

                    if ui.selectable_label(*selected_category_ui.lock().unwrap() == Category::Nodes,"💻 Nodes",).clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Nodes;
                    }

                    if ui.selectable_label(*selected_category_ui.lock().unwrap() == Category::Namespaces,"☰ Namespaces",).clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Namespaces;
                    }

                    if ui.selectable_label(*selected_category_ui.lock().unwrap() == Category::Events,"🕓 Events",).clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Events;
                    }
                });

                egui::CollapsingHeader::new("📦 Workloads").default_open(true).show(ui, |ui| {
                    if ui.selectable_label(current == Category::Pods, "📚 Pods").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Pods;
                    }

                    if ui.selectable_label(current == Category::Deployments, "📃 Deployments").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Deployments;
                    }

                    if ui.selectable_label(current == Category::StatefulSets, "📚 StatefulSets").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::StatefulSets;
                    }

                    if ui.selectable_label(current == Category::DaemonSets, "📰 DaemonSets").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::DaemonSets;
                    }

                    if ui.selectable_label(current == Category::ReplicaSets, "📜 ReplicaSets").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::ReplicaSets;
                    }

                    if ui.selectable_label(current == Category::Jobs, "💼 Jobs").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Jobs;
                    }

                    if ui.selectable_label(current == Category::CronJobs, "📅 CronJobs").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::CronJobs;
                    }
                });

                egui::CollapsingHeader::new("🛠 Config").default_open(false).show(ui, |ui| {
                    if ui.selectable_label(current == Category::ConfigMaps, "🗺 ConfigMaps").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::ConfigMaps;
                    }

                    if ui.selectable_label(current == Category::Secrets, "🕵 Secrets").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Secrets;
                    }

                    if ui.selectable_label(current == Category::PodDisruptionBudgets, "📌 Pod Disruption Budgets").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::PodDisruptionBudgets;
                    }
                });

                egui::CollapsingHeader::new("🖧 Network").default_open(false).show(ui, |ui| {
                    if ui.selectable_label(current == Category::Services, "🔗 Services").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Services;
                    }

                    if ui.selectable_label(current == Category::Endpoints, "⛺ Endpoints").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Endpoints;
                    }

                    if ui.selectable_label(current == Category::Ingresses, "⤵ Ingresses").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Ingresses;
                    }

                    if ui.selectable_label(current == Category::NetworkPolicies, "📋 Network Policies").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::NetworkPolicies;
                    }
                });

                egui::CollapsingHeader::new("🖴 Storage").default_open(false).show(ui, |ui| {
                    if ui.selectable_label(current == Category::PersistentVolumeClaims, "⛃ PersistentVolumeClaims").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::PersistentVolumeClaims;
                    }

                    if ui.selectable_label(current == Category::PersistentVolumes, "🗄 PersistentVolumes").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::PersistentVolumes;
                    }

                    if ui.selectable_label(current == Category::StorageClasses, "⛭ StorageClasses").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::StorageClasses;
                    }

                    if ui.selectable_label(current == Category::CSIDrivers, "🔌 CSI Drivers").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::CSIDrivers;
                    }
                });

                egui::CollapsingHeader::new("🛡 Access control").default_open(false).show(ui, |ui| {
                    if ui.selectable_label(current == Category::SeriviceAccounts, "👤 Service accounts").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::SeriviceAccounts;
                    }

                    if ui.selectable_label(current == Category::Roles, "📜 Roles").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Roles;
                    }

                    if ui.selectable_label(current == Category::ClusterRoles, "📁 Cluster roles").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::ClusterRoles;
                    }

                    if ui.selectable_label(current == Category::RoleBindings, "🔒 Role bindings").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::RoleBindings;
                    }

                    if ui.selectable_label(current == Category::ClusterRoleBindings, "🔐 Cluster role bindings").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::ClusterRoleBindings;
                    }
                });

                egui::CollapsingHeader::new("🖥 Custom Resources").default_open(false).show(ui, |ui| {
                    if ui.selectable_label(current == Category::CustomResourcesDefinitions, "📢 Definitions").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::CustomResourcesDefinitions;

                        let crds_clone = Arc::clone(&crds);
                        let result = get_crs_list(crds_clone);
                        *cr_grouped_list.lock().unwrap() = result;

                    }

                    for (group, items) in cr_grouped_list.lock().unwrap().iter() {
                        egui::CollapsingHeader::new(group).default_open(false).show(ui, |ui| {
                            for item_name in items {
                                if ui.label(egui::RichText::new(item_name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    println!("{}", item_name);
                                }
                            }
                        });
                    }
                });

                egui::CollapsingHeader::new("⎈ Helm").default_open(false).show(ui, |ui| {
                    if ui.selectable_label(current == Category::HelmReleases, "📥 Releases").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::HelmReleases;
                        tokio::spawn({
                            let client = Arc::clone(&client);
                            let list = Arc::clone(&helm_releases);
                            let helm_releases_loading = Arc::clone(&helm_releases_loading);

                            async move {
                                if let Err(e) = get_helm_releases(client.clone(), list.clone(), helm_releases_loading).await {
                                    eprintln!("Helm release fetch failed: {:?}", e);
                                }
                            }
                        });
                    }
                });

                egui::CollapsingHeader::new("🏷 About").default_open(false).show(ui, |ui| {
                    if ui.selectable_label(current == Category::About, "🐧 About author").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::About;
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match *selected_category_ui.lock().unwrap() {
                Category::About => {
                    show_about_info(ui);
                },
                Category::SeriviceAccounts => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_service_accounts: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        service_accounts.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        service_accounts.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("Service accounts - {}", visible_service_accounts.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::ServiceAccount;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_service_accounts).hint_text("Filter service accounts...").desired_width(200.0));
                        filter_service_accounts = filter_service_accounts.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_service_accounts.clear();
                        }
                    });
                    ui.separator();
                    if service_accounts_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_service_accounts.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("sservice_accounts_scroll").show(ui, |ui| {
                                egui::Grid::new("service_accounts_grid").striped(true).min_col_width(20.0).max_col_width(430.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_service_accounts.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_service_accounts.is_empty() || cur_item_object.contains(&filter_service_accounts) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&service_account_details);
                                                let ns = item.namespace.clone();
                                                service_account_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_service_account_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::core::v1::ServiceAccount>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::core::v1::ServiceAccount>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete ServiceAccount: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Roles => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_roles: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        roles.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        roles.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("Roles - {}", visible_roles.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::Role;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_roles).hint_text("Filter roles...").desired_width(200.0));
                        filter_roles = filter_roles.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_roles.clear();
                        }
                    });
                    ui.separator();
                    if roles_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_roles.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("roles_scroll").show(ui, |ui| {
                                egui::Grid::new("roles_grid").striped(true).min_col_width(20.0).max_col_width(430.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_roles.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_roles.is_empty() || cur_item_object.contains(&filter_roles) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&role_details);
                                                let ns = item.namespace.clone();
                                                role_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_role_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::rbac::v1::Role>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::rbac::v1::Role>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete Role: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::ClusterRoles => {
                    let visible_cluster_roles = cluster_roles.lock().unwrap();
                    ui.horizontal(|ui| {
                        ui.heading(format!("Cluster roles - {}", visible_cluster_roles.len()));
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::ClusterRole;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_cluster_roles).hint_text("Filter cluster roles...").desired_width(200.0));
                        filter_cluster_roles = filter_cluster_roles.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_cluster_roles.clear();
                        }
                    });
                    ui.separator();
                    if cluster_roles_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_cluster_roles.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("cluster_roles_scroll").show(ui, |ui| {
                                egui::Grid::new("cluster_roles_grid").striped(true).min_col_width(20.0).max_col_width(430.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_cluster_roles.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_cluster_roles.is_empty() || cur_item_object.contains(&filter_cluster_roles) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&cluster_role_details);
                                                cluster_role_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_cluster_role_details(client_clone, &name, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_cluster_yaml_for::<k8s_openapi::api::rbac::v1::ClusterRole>(
                                                        item.name.clone(),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    tokio::spawn(async move {
                                                        if let Err(err) = delete_cluster_role(client_clone, &cur_item.clone()).await {
                                                            eprintln!("Failed to delete cluster role: {}", err);
                                                        }
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::RoleBindings => {

                },
                Category::ClusterRoleBindings => {

                },
                Category::HelmReleases => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_helm_releases: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        helm_releases.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        helm_releases.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("Helm releases - {}", helm_releases.lock().unwrap().len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.add(egui::TextEdit::singleline(&mut filter_helm_releases).hint_text("Filter releases...").desired_width(200.0));
                        filter_helm_releases = filter_helm_releases.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_helm_releases.clear();
                        }
                    });
                    ui.separator();
                    if helm_releases_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        egui::ScrollArea::vertical().id_salt("helm_releases_scroll").show(ui, |ui| {
                            egui::Grid::new("helm_releases_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                ui.label("Name");
                                ui.label("Chart name");
                                ui.label("Version");
                                ui.label("Namespace");
                                ui.label("Age");
                                ui.end_row();
                                for item in visible_helm_releases.iter().rev().take(200) {
                                    let cur_item_object = &item.name;
                                    if filter_helm_releases.is_empty() || cur_item_object.contains(&filter_helm_releases) {
                                        ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                        if item.chart_name.is_some() {
                                            ui.label(format!("{}", item.chart_name.as_ref().unwrap()));
                                        } else {
                                            ui.label("");
                                        }
                                        if item.version.is_some() {
                                            ui.label(format!("{}", item.version.as_ref().unwrap()));
                                        } else {
                                            ui.label("");
                                        }
                                        if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                            *selected_ns = item.namespace.clone();
                                        }
                                        if item.creation_timestamp.is_some() {
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                        } else {
                                            ui.label("");
                                        }

                                        ui.end_row();
                                    }
                                }
                            });
                        });
                    }
                },
                Category::CustomResourcesDefinitions => {
                    ui.horizontal(|ui| {
                        ui.heading(format!("Custom Resources Definitions - {}", crds.lock().unwrap().len()));
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_crds).hint_text("Filter crds...").desired_width(200.0));
                        filter_crds = filter_crds.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_crds.clear();
                        }
                    });
                    ui.separator();
                    if crds_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        let crds_list = crds.lock().unwrap();
                        egui::ScrollArea::vertical().id_salt("crds_scroll").show(ui, |ui| {
                            egui::Grid::new("crds_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                ui.label("Name");
                                ui.label("Plural");
                                ui.label("Group");
                                ui.label("Version");
                                ui.label("Scope");
                                ui.label("Kind");
                                ui.label("Namespace");
                                ui.label("Age");
                                ui.end_row();
                                for item in crds_list.iter().rev().take(200) {
                                    let cur_item_object = &item.name;
                                    if filter_crds.is_empty() || cur_item_object.contains(&filter_crds) {
                                        if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                            // let name = cur_item_object.clone();
                                            // let version = item.version.clone();
                                            // let group = item.group.clone();
                                            // let plural = item.plural.clone();
                                            // let kind = item.kind.clone();
                                            // let ns = item.namespace.clone();
                                            // let scope = item.scope.clone();
                                            // let client_clone = Arc::clone(&client);
                                            // let details = Arc::clone(&crd_details);
                                            // crd_details_window.show = true;
                                            // tokio::spawn({
                                            //     async move {
                                            //         if let Err(e) = get_cr_details(client_clone, &name, &plural, &kind, &version, &group, &scope, ns, details).await {
                                            //             eprintln!("Details fetch failed: {:?}", e);
                                            //         }
                                            //     }
                                            // });
                                        }
                                        ui.label(format!("{}", &item.plural));
                                        ui.label(format!("{}", &item.group));
                                        ui.label(format!("{}", &item.version));
                                        ui.label(format!("{}", &item.scope));
                                        ui.label(format!("{}", &item.kind));
                                        if let Some(crd_namespace) = &item.namespace {
                                            ui.label(format!("{}", crd_namespace));
                                        } else {
                                            ui.label("-");
                                        }
                                        ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                        ui.end_row();
                                    }
                                }
                            });
                        });
                    }
                },
                Category::NetworkPolicies => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_network_policies: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        network_policies.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        network_policies.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("PodDisruptionBudgets - {}", visible_network_policies.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.add(egui::TextEdit::singleline(&mut filter_network_policies).hint_text("Filter policies...").desired_width(200.0));
                        filter_network_policies = filter_network_policies.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_network_policies.clear();
                        }
                    });
                    ui.separator();
                    if network_policies_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_network_policies.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("network_policies_scroll").show(ui, |ui| {
                                egui::Grid::new("network_policies_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Pod selecter");
                                    ui.label("Types");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_network_policies.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_network_policies.is_empty() || cur_item_object.contains(&filter_network_policies) {
                                            ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR));
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(&item.pod_selector);
                                            ui.label(&item.policy_types);
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::networking::v1::NetworkPolicy>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::networking::v1::NetworkPolicy>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete NetworkPolicy: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::PodDisruptionBudgets => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_pdbs: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        pdbs.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        pdbs.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("PodDisruptionBudgets - {}", visible_pdbs.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.add(egui::TextEdit::singleline(&mut filter_pdbs).hint_text("Filter pdbs...").desired_width(200.0));
                        filter_pdbs = filter_pdbs.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_pdbs.clear();
                        }
                    });
                    ui.separator();
                    if pdbs_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_pdbs.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("pdbs_scroll").show(ui, |ui| {
                                egui::Grid::new("pdbs_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Min available");
                                    ui.label("Max unavailable");
                                    ui.label("Current/Desired healthy");
                                    ui.label("Allowed disruptions");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_pdbs.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_pdbs.is_empty() || cur_item_object.contains(&filter_pdbs) {
                                            ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR));
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(&item.min_available.clone().unwrap_or_else(|| "-".to_string()));
                                            ui.label(&item.max_unavailable.clone().unwrap_or_else(|| "-".to_string()));
                                            ui.label(format!("{} / {}", &item.current_healthy, &item.desired_healthy));
                                            ui.label(format!("{}", &item.allowed_disruptions));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::policy::v1::PodDisruptionBudget>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::policy::v1::PodDisruptionBudget>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete pdb: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::DaemonSets => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_daemonsets: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        daemonsets.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        daemonsets.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("DaemonSets - {}", visible_daemonsets.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::DaemonSet;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_daemonsets).hint_text("Filter daemonsets...").desired_width(200.0));
                        filter_daemonsets = filter_daemonsets.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_daemonsets.clear();
                        }
                    });
                    ui.separator();
                    if daemonsets_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_daemonsets.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("daemonsets_scroll").show(ui, |ui| {
                                egui::Grid::new("daemonsets_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Desired");
                                    ui.label("Current");
                                    ui.label("Ready");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_daemonsets.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_daemonsets.is_empty() || cur_item_object.contains(&filter_daemonsets) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&daemonset_details);
                                                let ns = item.namespace.clone();
                                                daemonset_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_daemonset_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}", &item.desired));
                                            ui.label(format!("{}", &item.current));
                                            ui.label(format!("{}", &item.ready));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::apps::v1::DaemonSet>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::apps::v1::DaemonSet>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete daemonSet: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::ClusterOverview => {
                    //let mut pending_nav: Option<(String, Category)> = None;

                    ui.heading("Cluster Overview");
                    ui.separator();
                    let cluster = cluster_info_ui.lock().unwrap().clone();
                    ui.vertical(|ui| {
                        ui.label(format!("Connected to: {}", cluster.name));
                        ui.label(format!("Cluster name: {}", cluster_name));
                        ui.label(format!("User name: {}", user_name));
                    });

                    ui.add_space(20.0);

                    let stats = compute_overview_stats(
                        &pods.lock().unwrap(),
                        &deployments.lock().unwrap(),
                        &daemonsets.lock().unwrap(),
                        &statefulsets.lock().unwrap(),
                        &replicasets.lock().unwrap(),
                    );
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        show_overview(ui, &stats);

                        if stats.namespaces_with_pending_items.len() > 0 {
                            ui.add_space(50.0);
                            ui.heading("List of namespaces with pending items:");
                            egui::ScrollArea::vertical().id_salt("ns_with_pending_items_scroll").show(ui, |ui| {
                                ui.separator();
                                egui::Grid::new("ns_with_pending_items_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("");
                                    ui.label("Namespace");
                                    ui.label("Pending items");
                                    ui.end_row();
                                    for i in &stats.namespaces_with_pending_items {
                                        if selected_namespace_clone.lock().unwrap().is_some() && selected_namespace_clone.lock().unwrap().as_ref().unwrap() == i.0 {
                                            ui.colored_label(Color32::LIGHT_BLUE,"⏵");
                                        } else {
                                            ui.label("");
                                        };
                                        if ui.colored_label(Color32::WHITE,i.0).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                            *selected_namespace_clone.lock().unwrap() = Some(i.0.clone());
                                            // TODO (stack when enabled)
                                            //*selected_category_clone.lock().unwrap() = Category::Pods;
                                        }
                                        ui.label(i.1.to_string());
                                        ui.end_row();
                                    }
                                });
                            });
                        }
                    });
                },
                Category::ReplicaSets => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_replicasets: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        replicasets.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        replicasets.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("ReplicaSets - {}", visible_replicasets.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::ReplicaSet;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_replicasets).hint_text("Filter replicasets...").desired_width(200.0));
                        filter_replicasets = filter_replicasets.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_replicasets.clear();
                        }
                    });
                    ui.separator();
                    if replicasets_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_replicasets.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("replicasets_scroll").show(ui, |ui| {
                                egui::Grid::new("replicasets_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Desired");
                                    ui.label("Current");
                                    ui.label("Ready");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_replicasets.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        let status = if item.ready == 0 {
                                            if item.current > item.ready {
                                                "NotReady"
                                            } else {
                                                "Pending"
                                            }
                                        } else {
                                            "Ready"
                                        };
                                        if filter_replicasets.is_empty() || cur_item_object.contains(&filter_replicasets) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&replicaset_details);
                                                let ns = item.namespace.clone();
                                                replicaset_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_replicaset_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(egui::RichText::new(format!("{}", &item.desired)).color(item_color(status)));
                                            ui.label(egui::RichText::new(format!("{}", &item.current)).color(item_color(status)));
                                            ui.label(egui::RichText::new(format!("{}", &item.ready)).color(item_color(status)));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("⬍ Scale").size(16.0).color(ORANGE_BUTTON)).clicked() {
                                                    scale_window.show = true;
                                                    scale_window.name = Some(item.name.clone());
                                                    scale_window.namespace = selected_ns.clone();
                                                    scale_window.cur_replicas = item.current;
                                                    scale_window.desired_replicas = item.desired;
                                                    scale_window.resource_kind = Some(ScaleTarget::ReplicaSet);
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::apps::v1::ReplicaSet>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::apps::v1::ReplicaSet>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete replicaSet: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Ingresses => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_ingresses: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        ingresses.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        ingresses.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("Ingresses - {}", visible_ingresses.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::Ingress;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_ingresses).hint_text("Filter ingresses...").desired_width(200.0));
                        filter_ingresses = filter_ingresses.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_ingresses.clear();
                        }
                    });
                    ui.separator();
                    if ingresses_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_ingresses.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("ingresses_scroll").show(ui, |ui| {
                                egui::Grid::new("ingresses_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Host");
                                    ui.label("Paths");
                                    ui.label("Service");
                                    ui.label("Tls");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_ingresses.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_ingresses.is_empty() || cur_item_object.contains(&filter_ingresses) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&ingress_details);
                                                let ns = item.namespace.clone();
                                                ingress_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_ingress_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}", &item.host));
                                            ui.label(format!("{}", &item.paths));
                                            ui.label(format!("{}", &item.service));
                                            ui.label(format!("{}", &item.tls));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::networking::v1::Ingress>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::networking::v1::Ingress>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete ingress: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::CSIDrivers => {
                    ui.horizontal(|ui| {
                        ui.heading(format!("CSI Drivers - {}", csi_drivers.lock().unwrap().len()));
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_csi_drivers).hint_text("Filter csi drivers...").desired_width(200.0));
                        filter_csi_drivers = filter_csi_drivers.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_csi_drivers.clear();
                        }
                    });
                    ui.separator();
                    if csi_drivers_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if csi_drivers.lock().unwrap().len() == 0 {
                            show_empty(ui);
                        } else {
                            let csi_drivers_list = csi_drivers.lock().unwrap();
                            egui::ScrollArea::vertical().id_salt("csi_drivers_scroll").show(ui, |ui| {
                                egui::Grid::new("csi_drivers_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Attach Required");
                                    ui.label("Pod Info On Mount");
                                    ui.label("Storage Capacity");
                                    ui.label("FSGroupPolicy");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in csi_drivers_list.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_csi_drivers.is_empty() || cur_item_object.contains(&filter_csi_drivers) {
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}", &item.attach_required));
                                            ui.label(format!("{}", &item.pod_info_on_mount));
                                            ui.label(format!("{}", &item.storage_capacity));
                                            ui.label(format!("{}", &item.fs_group_policy));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_global::<k8s_openapi::api::storage::v1::CSIDriver>(client, &name).await {
                                                            Ok(yaml) => {
                                                                let mut editor = yaml_editor_window.lock().unwrap();
                                                                editor.content = yaml;
                                                                editor.show = true;
                                                            }
                                                            Err(e) => {
                                                                eprintln!("Failed to get YAML: {}", e);
                                                            }
                                                        }
                                                    });
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::StorageClasses => {
                    ui.horizontal(|ui| {
                        ui.heading(format!("StorageClasses - {}", pvs.lock().unwrap().len()));
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_scs).hint_text("Filter scs...").desired_width(200.0));
                        filter_scs = filter_scs.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_scs.clear();
                        }
                    });
                    ui.separator();
                    if storage_classes_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if storage_classes.lock().unwrap().len() == 0 {
                            show_empty(ui);
                        } else {
                            let scs_list = storage_classes.lock().unwrap();
                            egui::ScrollArea::vertical().id_salt("scs_scroll").show(ui, |ui| {
                                egui::Grid::new("scs_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Provisioner");
                                    ui.label("Reclaim policy");
                                    ui.label("Volume binding mode");
                                    ui.label("Default class");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in scs_list.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_scs.is_empty() || cur_item_object.contains(&filter_scs) {
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}", &item.provisioner));
                                            ui.label(format!("{}", &item.reclaim_policy));
                                            ui.label(format!("{}", &item.volume_binding_mode));
                                            ui.label(format!("{}", &item.is_default));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_global::<k8s_openapi::api::storage::v1::StorageClass>(client, &name).await {
                                                            Ok(yaml) => {
                                                                let mut editor = yaml_editor_window.lock().unwrap();
                                                                editor.content = yaml;
                                                                editor.show = true;
                                                            }
                                                            Err(e) => {
                                                                eprintln!("Failed to get YAML: {}", e);
                                                            }
                                                        }
                                                    });
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::PersistentVolumes => {
                    ui.horizontal(|ui| {
                        ui.heading(format!("PersistentVolumes - {}", pvs.lock().unwrap().len()));
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_pvs).hint_text("Filter pvs...").desired_width(200.0));
                        filter_pvs = filter_pvs.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_pvs.clear();
                        }
                    });
                    ui.separator();
                    if pvs_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if pvs.lock().unwrap().len() == 0 {
                            show_empty(ui);
                        } else {
                            let pvs_list = pvs.lock().unwrap();
                            egui::ScrollArea::vertical().id_salt("pvs_scroll").show(ui, |ui| {
                                egui::Grid::new("pvs_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Storage class");
                                    ui.label("Capacity");
                                    ui.label("Claim");
                                    ui.label("Status");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in pvs_list.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        let cur_item_claim = &item.claim;
                                        if filter_pvs.is_empty() || cur_item_object.contains(&filter_pvs) || cur_item_claim.contains(&filter_pvs) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&pv_details);
                                                pv_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_pv_details(client_clone, &name, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            ui.label(format!("{}", &item.storage_class));
                                            ui.label(format!("{}", &item.capacity));
                                            ui.label(format!("{}", &item.claim));
                                            ui.label(egui::RichText::new(&item.status).color(item_color(&item.status)));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_global::<k8s_openapi::api::core::v1::PersistentVolume>(client, &name).await {
                                                            Ok(yaml) => {
                                                                let mut editor = yaml_editor_window.lock().unwrap();
                                                                editor.content = yaml;
                                                                editor.show = true;
                                                            }
                                                            Err(e) => {
                                                                eprintln!("Failed to get YAML: {}", e);
                                                            }
                                                        }
                                                    });
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    // TODO
                                                    ui.close_kind(egui::UiKind::Menu)
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::PersistentVolumeClaims => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_pvcs: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        pvcs.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        pvcs.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("PersistentVolumeClaims - {}", visible_pvcs.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::PersistenceVolumeClaim;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_pvcs).hint_text("Filter pvcs...").desired_width(200.0));
                        filter_pvcs = filter_pvcs.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_pvcs.clear();
                        }
                    });
                    ui.separator();
                    if pvcs_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_pvcs.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("pvcs_scroll").show(ui, |ui| {
                                egui::Grid::new("pvcs_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("StorageClass");
                                    ui.label("Volume");
                                    ui.label("Size");
                                    ui.label("Status");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_pvcs.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_pvcs.is_empty() || cur_item_object.contains(&filter_pvcs) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&pvc_details);
                                                let ns = item.namespace.clone();
                                                pvc_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_pvc_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}", &item.storage_class));
                                            ui.label(format!("{}", &item.volume_name));
                                            ui.label(format!("{}", &item.size));
                                            ui.label(egui::RichText::new(&item.status).color(item_color(&item.status)));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::core::v1::PersistentVolumeClaim>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::core::v1::PersistentVolumeClaim>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete pvc: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Endpoints => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_endpoints: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        endpoints.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        endpoints.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("Endpoints - {}", visible_endpoints.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.add(egui::TextEdit::singleline(&mut filter_endpoints).hint_text("Filter jobs...").desired_width(200.0));
                        filter_endpoints = filter_endpoints.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_endpoints.clear();
                        }
                    });
                    ui.separator();
                    if endpoints_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_endpoints.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("endpoints_scroll").show(ui, |ui| {
                                egui::Grid::new("endpoints_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Addresses");
                                    ui.label("Ports");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_endpoints.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_endpoints.is_empty() || cur_item_object.contains(&filter_endpoints) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&endpoint_details);
                                                let ns = item.namespace.clone();
                                                endpoint_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_endpoint_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}", &item.addresses));
                                            ui.label(format!("{:?}", &item.ports));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::core::v1::Endpoints>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::core::v1::Endpoints>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete endpoints: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Jobs => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_jobs: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        jobs.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        jobs.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("Jobs - {}", visible_jobs.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.add(egui::TextEdit::singleline(&mut filter_jobs).hint_text("Filter jobs...").desired_width(200.0));
                        filter_jobs = filter_jobs.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_jobs.clear();
                        }
                    });
                    ui.separator();
                    if jobs_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_jobs.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("jobs_scroll").show(ui, |ui| {
                                egui::Grid::new("jobs_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Completions");
                                    ui.label("Conditions");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_jobs.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_jobs.is_empty() || cur_item_object.contains(&filter_jobs) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&job_details);
                                                let ns = item.namespace.clone();
                                                job_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_job_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}", &item.completions));
                                            ui.label(egui::RichText::new(&item.condition).color(item_color(&item.condition)));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::batch::v1::Job>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::batch::v1::Job>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete job: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Services => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_services: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        services.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        services.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("Services - {}", visible_services.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::Service;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_services).hint_text("Filter services...").desired_width(200.0));
                        filter_services = filter_services.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_services.clear();
                        }
                    });
                    ui.separator();
                    if services_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_services.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("services_scroll").show(ui, |ui| {
                                egui::Grid::new("services_grid").striped(true).min_col_width(20.0).max_col_width(400.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Type");
                                    ui.label("Cluster IP");
                                    ui.label("External IP");
                                    ui.label("Status");
                                    ui.label("Age");
                                    ui.label("Ports");
                                    ui.label("Selector");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_services.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_services.is_empty() || cur_item_object.contains(&filter_services) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&service_details);
                                                let ns = item.namespace.clone();
                                                service_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_service_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}", &item.svc_type));
                                            ui.label(format!("{:?}", &item.cluster_ip));
                                            ui.label(format!("{:?}", &item.external_ip));
                                            ui.label(format!("{:?}", &item.status));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.label(egui::RichText::new(&item.ports).color(egui::Color32::LIGHT_YELLOW));
                                            ui.label(format!("{:?}", &item.selector));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::core::v1::Service>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::core::v1::Service>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete service: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::CronJobs => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_cronjobs: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        cronjobs.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        cronjobs.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("CronJobs - {}", visible_cronjobs.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.add(egui::TextEdit::singleline(&mut filter_cronjobs).hint_text("Filter cronjobs...").desired_width(200.0));
                        filter_cronjobs = filter_cronjobs.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_cronjobs.clear();
                        }
                    });
                    ui.separator();
                    if cronjobs_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_cronjobs.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("cronjobs_scroll").show(ui, |ui| {
                                egui::Grid::new("cronjobs_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Schedule");
                                    ui.label("Suspend");
                                    ui.label("Active");
                                    ui.label("Last schedule");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_cronjobs.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_cronjobs.is_empty() || cur_item_object.contains(&filter_cronjobs) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&cronjob_details);
                                                let ns = item.namespace.clone();
                                                cronjob_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_cronjob_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}", &item.schedule));
                                            ui.label(format!("{}", &item.suspend));
                                            ui.label(format!("{}", &item.active));
                                            ui.label(format!("{}", &item.last_schedule));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::batch::v1::CronJob>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::batch::v1::CronJob>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete cronJob: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::StatefulSets => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_statefulsets: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        statefulsets.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        statefulsets.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("StatefulSets - {}", visible_statefulsets.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.add(egui::TextEdit::singleline(&mut filter_statefulsets).hint_text("Filter statefulsets...").desired_width(200.0));
                        filter_statefulsets = filter_statefulsets.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_statefulsets.clear();
                        }
                    });
                    ui.separator();
                    if statefulsets_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_statefulsets.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("statefulsets_scroll").show(ui, |ui| {
                                egui::Grid::new("statefulsets_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Ready");
                                    ui.label("Service name");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_statefulsets.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_statefulsets.is_empty() || cur_item_object.contains(&filter_statefulsets) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&statefulset_details);
                                                let ns = item.namespace.clone();
                                                statefulset_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_statefulset_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}/{}", &item.ready_replicas, &item.replicas));
                                            ui.label(egui::RichText::new(&item.service_name).italics().color(egui::Color32::CYAN));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("⬍ Scale").size(16.0).color(ORANGE_BUTTON)).clicked() {
                                                    scale_window.show = true;
                                                    scale_window.name = Some(item.name.clone());
                                                    scale_window.namespace = selected_ns.clone();
                                                    scale_window.cur_replicas = item.ready_replicas;
                                                    scale_window.desired_replicas = item.ready_replicas;
                                                    scale_window.resource_kind = Some(ScaleTarget::StatefulSet);
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::apps::v1::StatefulSet>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::apps::v1::StatefulSet>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete statefulSet: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Nodes => {
                    ui.horizontal(|ui| {
                        ui.heading(format!("Nodes - {}", nodes.lock().unwrap().len()));
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_nodes).hint_text("Filter nodes...").desired_width(200.0));
                        filter_nodes = filter_nodes.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_nodes.clear();
                        }
                    });
                    ui.separator();
                    if nodes_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if nodes.lock().unwrap().len() == 0 {
                            show_empty(ui);
                        } else {
                            let nodes = nodes.lock().unwrap();
                            egui::ScrollArea::vertical().id_salt("node_scroll").hscroll(true).show(ui, |ui| {
                                egui::Grid::new("node_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    if ui.label("Name").on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        if sort_by == SortBy::Name {
                                            sort_asc = !sort_asc;
                                        } else {
                                            sort_by = SortBy::Name;
                                            sort_asc = true;
                                        }
                                    }
                                    ui.label("CPU");
                                    ui.label("Memory");
                                    ui.label("Storage");
                                    ui.label("Taints");
                                    ui.label("Version");
                                    ui.label("Role");
                                    if ui.label("Age").on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        if sort_by == SortBy::Age {
                                            sort_asc = !sort_asc;
                                        } else {
                                            sort_by = SortBy::Age;
                                            sort_asc = true;
                                        }
                                    }
                                    ui.label("Status");
                                    ui.label("");
                                    ui.label("Actions");
                                    ui.end_row();
                                    let mut sorted_nodes = nodes.clone();
                                    sorted_nodes.sort_by(|a, b| {
                                        let ord = match sort_by {
                                            SortBy::Name => a.name.cmp(&b.name),
                                            SortBy::Age => {
                                                let at = a.creation_timestamp.as_ref();
                                                let bt = b.creation_timestamp.as_ref();
                                                at.cmp(&bt)
                                            }
                                        };
                                        if sort_asc { ord } else { ord.reverse() }
                                    });
                                    for item in sorted_nodes.iter() {
                                        let cur_item_name = &item.name;
                                        if filter_nodes.is_empty() || cur_item_name.contains(&filter_nodes) {
                                            if ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_name.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&node_details);
                                                node_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_node_details(client_clone, &name, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if let Some(p) = &item.cpu_percent {
                                                let hover_text = format!("Used: {} / Total: {}", item.cpu_used.unwrap_or(0.0), item.cpu_total.unwrap_or(0.0));
                                                ui.add(egui::ProgressBar::new(p / 100.0).show_percentage()).on_hover_text(hover_text);
                                            } else {
                                                ui.add(egui::ProgressBar::new(0.0).show_percentage()).on_hover_text("Loading...");
                                            }
                                            if let Some(p) = &item.mem_percent {
                                                let hover_text = format!("Used: {} Gb / Total: {} Gb", item.mem_used.unwrap_or(0.0), item.mem_total.unwrap_or(0.0));
                                                ui.add(egui::ProgressBar::new(p / 100.0).show_percentage()).on_hover_text(hover_text);
                                            } else {
                                                ui.add(egui::ProgressBar::new(0.0).show_percentage()).on_hover_text("Loading...");
                                            }
                                            if let Some(p) = &item.storage_percent {
                                                let hover_text = format!("Used: {} Gb / Total: {} Gb", item.storage_used.unwrap_or(0.0), item.storage_total.unwrap_or(0.0));
                                                ui.add(egui::ProgressBar::new(p / 100.0).show_percentage()).on_hover_text(hover_text);
                                            } else {
                                                ui.add(egui::ProgressBar::new(0.0).show_percentage()).on_hover_text("Loading...");
                                            }
                                            if let Some(taints) = &item.taints {
                                                ui.label(taints.len().to_string())
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text(format!("{:?}", taints));
                                            } else {
                                                ui.label("0");
                                            }
                                            if let Some(version) = &item.version {
                                                ui.label(egui::RichText::new(version).color(egui::Color32::LIGHT_YELLOW));
                                            } else {
                                                ui.label("unknown");
                                            }
                                            ui.label(format!("{}", item.roles.join(", ")));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));

                                            let node_status = egui::RichText::new(&item.status).color(item_color(&item.status));
                                            let scheduling_status = match item.scheduling_disabled {
                                                true => egui::RichText::new("SchedulingDisabled").color(egui::Color32::ORANGE),
                                                false => egui::RichText::new(""),
                                            };

                                            ui.label( node_status);
                                            ui.label( scheduling_status);

                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                let node_name = item.name.clone();
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_global::<k8s_openapi::api::core::v1::Node>(client, &name).await {
                                                            Ok(yaml) => {
                                                                let mut editor = yaml_editor_window.lock().unwrap();
                                                                editor.content = yaml;
                                                                editor.show = true;
                                                            }
                                                            Err(e) => {
                                                                eprintln!("Failed to get YAML: {}", e);
                                                            }
                                                        }
                                                    });
                                                }
                                                if item.scheduling_disabled {
                                                    if ui.button("▶ Uncordon").clicked() {
                                                        let client_clone = Arc::clone(&client);
                                                        tokio::spawn(async move {
                                                            if let Err(err) = cordon_node(client_clone, &node_name, false).await {
                                                                eprintln!("Failed to uncordon node: {}", err);
                                                            }
                                                        });
                                                        ui.close_kind(egui::UiKind::Menu);
                                                    }
                                                } else {
                                                    if ui.button(egui::RichText::new("⏸ Cordon").size(16.0).color(ORANGE_BUTTON)).clicked() {
                                                        let client_clone = Arc::clone(&client);
                                                        tokio::spawn(async move {
                                                            if let Err(err) = cordon_node(client_clone, &node_name, true).await {
                                                                eprintln!("Failed to cordon node: {}", err);
                                                            }
                                                        });
                                                        ui.close_kind(egui::UiKind::Menu);
                                                    }
                                                }
                                                if ui.button(egui::RichText::new("♻ Drain").size(16.0).color(BLUE_BUTTON)).clicked() {
                                                    let node_name = item.name.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    tokio::spawn(async move {
                                                        if let Err(err) = drain_node(client_clone, &node_name).await {
                                                            eprintln!("Failed to drain node: {}", err);
                                                        }
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let node_name = item.name.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    tokio::spawn(async move {
                                                        if let Err(err) = delete_node(client_clone, &node_name).await {
                                                            eprintln!("Failed to delete node: {}", err);
                                                        }
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Namespaces => {
                    ui.horizontal(|ui| {
                        ui.heading(format!("Namespaces - {}", namespaces.lock().unwrap().len()));
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::NameSpace;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_namespaces).hint_text("Filter namespaces...").desired_width(200.0));
                        filter_namespaces = filter_namespaces.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_namespaces.clear();
                        }
                    });
                    ui.separator();
                    if namespaces_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if namespaces.lock().unwrap().len() == 0 {
                            show_empty(ui);
                        } else {
                            let ns = namespaces.lock().unwrap();
                            egui::ScrollArea::vertical().id_salt("namespace_scroll").show(ui, |ui| {
                                egui::Grid::new("namespace_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("");
                                    if ui.label("Name").on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        if sort_by == SortBy::Name {
                                            sort_asc = !sort_asc;
                                        } else {
                                            sort_by = SortBy::Name;
                                            sort_asc = true;
                                        }
                                    }
                                    ui.label("Phase");
                                    ui.label("Labels");
                                    if ui.label("Age").on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        if sort_by == SortBy::Age {
                                            sort_asc = !sort_asc;
                                        } else {
                                            sort_by = SortBy::Age;
                                            sort_asc = true;
                                        }
                                    }
                                    ui.label("Actions");
                                    ui.end_row();
                                    let mut sorted_ns = ns.clone();
                                    sorted_ns.sort_by(|a, b| {
                                        let ord = match sort_by {
                                            SortBy::Name => a.name.cmp(&b.name),
                                            SortBy::Age => {
                                                let at = &a.creation_timestamp;
                                                let bt = &b.creation_timestamp;
                                                at.cmp(&bt)
                                            }
                                        };
                                        if sort_asc { ord } else { ord.reverse() }
                                    });

                                    for item in sorted_ns.iter_mut() {
                                        let cur_item_name = &item.name;
                                        if filter_namespaces.is_empty() || cur_item_name.contains(&filter_namespaces) {
                                            if selected_namespace_clone.lock().unwrap().is_some() && selected_namespace_clone.lock().unwrap().as_ref().unwrap() == &item.name {
                                                ui.colored_label(Color32::LIGHT_BLUE,"⏵");
                                            } else {
                                                ui.label("");
                                            };

                                            if ui.colored_label(Color32::WHITE,&item.name).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_namespace_clone.lock().unwrap() = Some(item.name.clone());
                                            }
                                            if let Some(phase) = &item.phase {
                                                ui.colored_label(item_color(phase), phase);
                                            } else {
                                                ui.colored_label(Color32::LIGHT_GRAY, "-");
                                            }

                                            if let Some(labels) = &item.labels {
                                                let label_str = labels.iter()
                                                    .map(|(k, v)| format!("{}={}", k, v))
                                                    .collect::<Vec<_>>()
                                                    .join(", ");
                                                ui.label(label_str);
                                            } else {
                                                ui.label("-");
                                            }

                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);

                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_global::<k8s_openapi::api::core::v1::Namespace>(client, &name).await {
                                                            Ok(yaml) => {
                                                                let mut editor = yaml_editor_window.lock().unwrap();
                                                                editor.content = yaml;
                                                                editor.show = true;
                                                            }
                                                            Err(e) => {
                                                                eprintln!("Failed to get YAML: {}", e);
                                                            }
                                                        }
                                                    });
                                                }

                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    // TODO (confirm dialog)
                                                    let cur_item = item.name.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    tokio::spawn(async move {
                                                        if let Err(err) = delete_namespace(client_clone, &cur_item.clone()).await {
                                                            eprintln!("Failed to delete namespace: {}", err);
                                                        }
                                                    });
                                                    *selected_namespace_clone.lock().unwrap() = None;
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Pods => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_pods: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        pods.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        pods.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("Pods - {}", visible_pods.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::Pod;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_pods).hint_text("Filter pods...").desired_width(200.0));
                        filter_pods = filter_pods.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_pods.clear();
                        }
                    });
                    ui.separator();
                    if pods_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_pods.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("pods_scroll").show(ui, |ui| {
                                egui::Grid::new("pods_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    if ui.label("Name").on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        if sort_by == SortBy::Name {
                                            sort_asc = !sort_asc;
                                        } else {
                                            sort_by = SortBy::Name;
                                            sort_asc = true;
                                        }
                                    }
                                    ui.label("Status");
                                    ui.label("Containers");
                                    if ui.label("Age").on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        if sort_by == SortBy::Age {
                                            sort_asc = !sort_asc;
                                        } else {
                                            sort_by = SortBy::Age;
                                            sort_asc = true;
                                        }
                                    }
                                    ui.label("Restarts");
                                    ui.label("Controlled by");
                                    ui.label("QoS");
                                    ui.label("Node");
                                    ui.label("Actions");
                                    ui.end_row();
                                    let mut sorted_pods = visible_pods.clone();
                                    sorted_pods.sort_by(|a, b| {
                                        let ord = match sort_by {
                                            SortBy::Name => a.name.cmp(&b.name),
                                            SortBy::Age => {
                                                let at = a.creation_timestamp.as_ref();
                                                let bt = b.creation_timestamp.as_ref();
                                                at.cmp(&bt)
                                            }
                                        };
                                        if sort_asc { ord } else { ord.reverse() }
                                    });
                                    for item in sorted_pods.iter() {
                                        let cur_item_name = &item.name;
                                        // TODO:
                                        // check what bad with nodes filter
                                        // let running_on_node = item.node_name.as_ref().unwrap();
                                        // if filter_pods.is_empty() || cur_item_name.contains(&filter_pods) || running_on_node.contains(&filter_pods) {
                                        if filter_pods.is_empty() || cur_item_name.contains(&filter_pods) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_name.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&pod_details);
                                                let ns = item.namespace.clone();

                                                pod_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_pod_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            let status;
                                            let mut ready_color: Color32;
                                            let cur_phase: &str;
                                            if item.pod_has_crashloop {
                                                cur_phase = "CrashLoopBackOff";
                                            } else {
                                                if item.terminating {
                                                    cur_phase = "Terminating";
                                                } else {
                                                    cur_phase = item.phase.as_ref().unwrap();
                                                }
                                            }
                                            match cur_phase {
                                                "Running" => {
                                                    status = "✅ Running".to_string();
                                                    ready_color = item_color("Running");
                                                },
                                                "Terminating" => {
                                                    status = "🗑 Terminating".to_string();
                                                    ready_color = item_color("Terminating");
                                                },
                                                "Pending" => {
                                                    status = "⏳ Pending".to_string();
                                                    ready_color = item_color("Pending");
                                                },
                                                "Succeeded" => {
                                                    status = "✅ Completed".to_string();
                                                    ready_color = item_color("Completed");
                                                },
                                                "Failed" => {
                                                    status = "❌ Failed".to_string();
                                                    ready_color = item_color("Failed");
                                                },
                                                "CrashLoopBackOff" => {
                                                    status = "💥 CrashLoop".to_string();
                                                    ready_color = item_color("CrashLoop");
                                                },
                                                "Cancelled" => {
                                                    status = "🚫 Cancelled".to_string();
                                                    ready_color = item_color("Cancelled");
                                                },
                                                _ => {
                                                    status = "❓ Unknown".to_string();
                                                    ready_color =  item_color("Unknown");
                                                },
                                            };
                                            ui.label(egui::RichText::new(status).color(ready_color));

                                            let ready = item.ready_containers;
                                            let total = item.total_containers;

                                            ready_color = if ready == total {
                                                Color32::from_rgb(100, 255, 100) // green
                                            } else if ready == 0 {
                                                if cur_phase != "Succeeded" {
                                                    Color32::from_rgb(255, 100, 100) // red
                                                } else {
                                                    Color32::GRAY
                                                }
                                            } else {
                                                Color32::from_rgb(255, 165, 0) // orange
                                            };

                                            ui.colored_label(ready_color, format!("{}/{}", ready, total)).on_hover_cursor(CursorIcon::PointingHand).on_hover_ui(|ui| {
                                                for container in &item.containers {
                                                    let icon = match container.state.as_deref() {
                                                        Some("Running") => "✅",
                                                        Some("Waiting") => "⏳",
                                                        Some("Terminated") => "❌",
                                                        _ => "❔",
                                                    };

                                                    let state_str = container.state.as_deref().unwrap_or("Unknown");

                                                    ui.horizontal(|ui| {
                                                        ui.label(format!("{} {}", icon, container.name));
                                                        ui.label(egui::RichText::new(state_str).color(item_color(state_str)));
                                                    });

                                                    if let Some(msg) = &container.message {
                                                        ui.label(egui::RichText::new(format!("💬 {}", msg)).italics().color(egui::Color32::GRAY));
                                                    }
                                                }
                                            });
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            if item.restart_count > 0 {
                                                ui.label(egui::RichText::new(format!("{}", item.restart_count)).color(egui::Color32::ORANGE));
                                            } else {
                                                ui.label(egui::RichText::new(format!("Never")).color(egui::Color32::GRAY));
                                            }
                                            if item.controller.is_some().clone() {
                                                ui.label(item.controller.as_ref().unwrap());
                                            } else {
                                                ui.label("");
                                            }
                                            ui.label(egui::RichText::new(item.qos_class.clone().unwrap_or("-".into())).color(item_color(&item.qos_class.clone().unwrap_or("-".to_string()))));
                                            ui.label(item.node_name.clone().unwrap_or("-".into()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = delete_pod(client_clone, cur_item.clone(), cur_ns.as_deref(), true).await {
                                                                eprintln!("Failed to delete pod: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    edit_yaml_for::<k8s_openapi::api::core::v1::Pod>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap(),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client),
                                                    );
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button("📃 Logs").clicked() {
                                                    open_logs_for_pod(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap(),
                                                        item.containers.clone(),
                                                        Arc::clone(&log_window),
                                                        Arc::clone(&client),
                                                    );
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("🖵 Shell").size(16.0).color(ORANGE_BUTTON)).clicked() {
                                                    // TODO
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }

                                                if ui.button(egui::RichText::new("🔍 Details").size(16.0).color(BLUE_BUTTON)).clicked() {
                                                    let name = cur_item_name.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    let details = Arc::clone(&pod_details);
                                                    let ns = item.namespace.clone();

                                                    pod_details_window.show = true;
                                                    tokio::spawn({
                                                        async move {
                                                            if let Err(e) = get_pod_details(client_clone, &name, ns, details).await {
                                                                eprintln!("Details fetch failed: {:?}", e);
                                                            }
                                                        }
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Deployments => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_deployments: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        deployments.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        deployments.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("Deployments - {}", visible_deployments.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::Deployment;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_deployments).hint_text("Filter deployments...").desired_width(200.0));
                        filter_deployments = filter_deployments.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_deployments.clear();
                        }
                    });
                    ui.separator();
                    if deployments_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_deployments.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("deployments_scroll").show(ui, |ui| {
                                egui::Grid::new("deployments_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Ready");
                                    ui.label("Desired");
                                    ui.label("Up-to-date");
                                    ui.label("Available");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_deployments.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_deployments.is_empty() || cur_item_object.contains(&filter_deployments) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&deployment_details);
                                                let ns = item.namespace.clone();
                                                deployment_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_deployment_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}/{}", &item.ready_replicas, &item.replicas));
                                            ui.label(format!("{}", &item.replicas));
                                            ui.label(format!("{}", &item.updated_replicas));
                                            ui.label(format!("{}", &item.available_replicas));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::apps::v1::Deployment>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::apps::v1::Deployment>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete deployment: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("⬍ Scale").size(16.0).color(ORANGE_BUTTON)).clicked() {
                                                    scale_window.show = true;
                                                    scale_window.name = Some(item.name.clone());
                                                    scale_window.namespace = selected_ns.clone();
                                                    scale_window.cur_replicas = item.replicas;
                                                    scale_window.desired_replicas = item.replicas;
                                                    scale_window.resource_kind = Some(ScaleTarget::Deployment);
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Secrets => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_secrets: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        secrets.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        secrets.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("Secrets - {}", visible_secrets.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::Secret;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_secrets).hint_text("Filter secrets...").desired_width(200.0));
                        filter_secrets = filter_secrets.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_secrets.clear();
                        }
                    });
                    ui.separator();
                    if secrets_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_secrets.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("secrets_scroll").show(ui, |ui| {
                                egui::Grid::new("secrets_grid").striped(true).min_col_width(20.0).max_col_width(430.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Type");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_secrets.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_secrets.is_empty() || cur_item_object.contains(&filter_secrets) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&secret_details);
                                                let ns = item.namespace.clone();
                                                secret_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_secret_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}", &item.secret_type));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::core::v1::Secret>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::core::v1::Secret>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete secret: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::ConfigMaps => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    let visible_configmaps: Vec<_> = if let Some(ns) = selected_ns.as_ref() {
                        configmaps.lock().unwrap()
                            .iter()
                            .filter(|p| p.namespace.as_deref() == Some(ns))
                            .cloned()
                            .collect()
                    } else {
                        configmaps.lock().unwrap().iter().cloned().collect()
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("ConfigMaps - {}", visible_configmaps.len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("all")).width(150.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut *selected_ns, None, "all");
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Add new").size(16.0).color(GREEN_BUTTON)).clicked() {
                            new_resource_window.resource_type = ResourceType::ConfigMap;
                            new_resource_window.content.clear();
                            new_resource_window.show = true;
                        }
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_configmaps).hint_text("Filter configmaps...").desired_width(200.0));
                        filter_configmaps = filter_configmaps.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_configmaps.clear();
                        }
                    });
                    ui.separator();
                    if configmaps_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if visible_configmaps.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("configmaps_scroll").show(ui, |ui| {
                                egui::Grid::new("configmaps_grid").striped(true).min_col_width(20.0).max_col_width(430.0).show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Namespace");
                                    ui.label("Type");
                                    ui.label("Age");
                                    ui.label("Keys");
                                    ui.label("Actions");
                                    ui.end_row();

                                    for item in visible_configmaps.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_configmaps.is_empty() || cur_item_object.contains(&filter_configmaps) {
                                            if ui.label(egui::RichText::new(&item.name).color(ITEM_NAME_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                let name = cur_item_object.clone();
                                                let client_clone = Arc::clone(&client);
                                                let details = Arc::clone(&configmap_details);
                                                let ns = item.namespace.clone();
                                                configmap_details_window.show = true;
                                                tokio::spawn({
                                                    async move {
                                                        if let Err(e) = get_configmap_details(client_clone, &name, ns, details).await {
                                                            eprintln!("Details fetch failed: {:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                            if ui.label(egui::RichText::new(&item.namespace.clone().unwrap_or("".to_string())).color(NAMESPACE_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                *selected_ns = item.namespace.clone();
                                            }
                                            ui.label(format!("{}", &item.type_));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.label(format!("{}", &item.keys.join(", ")));

                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);

                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::core::v1::ConfigMap>(
                                                        item.name.clone(),
                                                        item.namespace.clone().unwrap_or_else(|| "default".to_string()),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }

                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = item.namespace.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    confirmation_dialog.request(cur_item.clone(), cur_ns.clone(), move || {
                                                        tokio::spawn(async move {
                                                            if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::core::v1::ConfigMap>(
                                                                cur_item.clone(),
                                                                cur_ns.as_deref(),
                                                                client_clone,
                                                            ).await {
                                                                eprintln!("Failed to delete configmap: {}", err);
                                                            }
                                                        });
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
                Category::Events => {
                    ui.horizontal(|ui| {
                        ui.heading(format!("Events - {}", events.lock().unwrap().len()));
                        ui.separator();
                        ui.add(egui::TextEdit::singleline(&mut filter_events).hint_text("Filter events...").desired_width(200.0));
                        filter_events = filter_events.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_events.clear();
                        }
                    });
                    ui.separator();
                    let events_list = events.lock().unwrap();
                    if events_loading.load(Ordering::Relaxed) {
                        show_loading(ui);
                    } else {
                        if events_list.len() == 0 {
                            show_empty(ui);
                        } else {
                            egui::ScrollArea::vertical().id_salt("events_scroll").show(ui, |ui| {
                                egui::Grid::new("events_grid").striped(true).min_col_width(20.0).max_col_width(430.0).show(ui, |ui| {
                                    ui.label("Time");
                                    ui.label("Type");
                                    ui.label("Age");
                                    ui.label("Namespace");
                                    ui.label("Reason");
                                    ui.label("Object");
                                    ui.label("Message");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in events_list.iter().rev().take(200) {
                                        let cur_item_object = &item.involved_object;
                                        if filter_events.is_empty() || cur_item_object.contains(&filter_events) {
                                            ui.label(&item.timestamp);
                                            ui.label(egui::RichText::new(&item.event_type).color(item_color(&item.event_type)));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.label(&item.namespace);
                                            ui.label(&item.reason);
                                            ui.label(&item.involved_object);
                                            ui.label(&item.message);
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    crate::edit_yaml_for::<k8s_openapi::api::core::v1::Event>(
                                                        item.involved_object.clone(),
                                                        item.namespace.clone(),
                                                        Arc::clone(&yaml_editor_window),
                                                        Arc::clone(&client)
                                                    );
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        }
                    }
                },
            }
        });

        // YAML editor
        if let Ok(mut editor) = yaml_editor_window.lock() {
            if editor.show {
                let client_clone = Arc::clone(&client);
                show_yaml_editor(ctx, &mut editor, &mut decoder_window, client_clone);
            }
        }

        // New resource creation window
        if new_resource_window.show {
            let client_clone = Arc::clone(&client);
            show_new_resource_window(ctx, &mut new_resource_window, client_clone);
        }

        // Decoder window
        if decoder_window.show {
            show_decoder_window(ctx, &mut decoder_window);
        }

        // Logs window
        if let Ok(mut logs) = log_window.lock() {
            if logs.show {
                let client_clone = Arc::clone(&client);
                show_log_window(ctx, &mut logs, client_clone);
            }
        }

        // Node details window
        if node_details_window.show {
            let node_details_clone = Arc::clone(&node_details);
            let nodes_clone = Arc::clone(&nodes);
            let pods_clone = Arc::clone(&pods);
            show_node_details_window(ctx, &mut node_details_window, node_details_clone, nodes_clone, pods_clone);
        }

        // Pod details window
        if pod_details_window.show {
            let pod_details_clone = Arc::clone(&pod_details);
            let pods_clone = Arc::clone(&pods);
            let log_window_clone = Arc::clone(&log_window);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_pod_details_window(ctx, &mut pod_details_window, pod_details_clone, pods_clone, log_window_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Deployment details window
        if deployment_details_window.show {
            let deployment_details_clone = Arc::clone(&deployment_details);
            let deployments_clone = Arc::clone(&deployments);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_deployment_details_window(ctx, &mut deployment_details_window, deployment_details_clone, deployments_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Pvc details window
        if pvc_details_window.show {
            let pvc_details_clone = Arc::clone(&pvc_details);
            let pvcs_clone = Arc::clone(&pvcs);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_pvc_details_window(ctx, &mut pvc_details_window, pvc_details_clone, pvcs_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Pv details window
        if pv_details_window.show {
            let pv_details_clone = Arc::clone(&pv_details);
            let pvs_clone = Arc::clone(&pvs);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_pv_details_window(ctx, &mut pv_details_window, pv_details_clone, pvs_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Service details window
        if service_details_window.show {
            let service_details_clone = Arc::clone(&service_details);
            let services_clone = Arc::clone(&services);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_service_details_window(ctx, &mut service_details_window, service_details_clone, services_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Endpoint details window
        if endpoint_details_window.show {
            let endpoint_details_clone = Arc::clone(&endpoint_details);
            let endpoints_clone = Arc::clone(&endpoints);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_endpoint_details_window(ctx, &mut endpoint_details_window, endpoint_details_clone, endpoints_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Ingress details window
        if ingress_details_window.show {
            let ingress_details_clone = Arc::clone(&ingress_details);
            let ingresses_clone = Arc::clone(&ingresses);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_ingress_details_window(ctx, &mut ingress_details_window, ingress_details_clone, ingresses_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Service account details window
        if service_account_details_window.show {
            let service_account_details_clone = Arc::clone(&service_account_details);
            let service_accounts_clone = Arc::clone(&service_accounts);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_service_account_details_window(ctx, &mut service_account_details_window, service_account_details_clone, service_accounts_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Role details window
        if role_details_window.show {
            let role_details_clone = Arc::clone(&role_details);
            let roles_clone = Arc::clone(&roles);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_role_details_window(ctx, &mut role_details_window, role_details_clone, roles_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Cluser role details window
        if cluster_role_details_window.show {
            let cluster_role_details_clone = Arc::clone(&cluster_role_details);
            let cluster_roles_clone = Arc::clone(&cluster_roles);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_cluster_role_details_window(ctx, &mut cluster_role_details_window, cluster_role_details_clone, cluster_roles_clone, yaml_editor_window_clone, client_clone);
        }

        // Secret details window
        if secret_details_window.show {
            let secret_details_clone = Arc::clone(&secret_details);
            let secrets_clone = Arc::clone(&secrets);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_secret_details_window(ctx, &mut secret_details_window, secret_details_clone, secrets_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // // CRD details window
        // if crd_details_window.show {
        //     let crd_details_clone = Arc::clone(&crd_details);
        //     let crds_clone = Arc::clone(&crds);
        //     //let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
        //     //let client_clone = Arc::clone(&client);
        //     show_crd_details_window(ctx, &mut crd_details_window, crd_details_clone, crds_clone,  &mut confirmation_dialog);
        // }

        // DaemonSet details window
        if daemonset_details_window.show {
            let daemonset_details_clone = Arc::clone(&daemonset_details);
            let daemonsets_clone = Arc::clone(&daemonsets);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_daemonset_details_window(ctx, &mut daemonset_details_window, daemonset_details_clone, daemonsets_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // ReplicaSet details window
        if replicaset_details_window.show {
            let replicaset_details_clone = Arc::clone(&replicaset_details);
            let replicasets_clone = Arc::clone(&replicasets);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_replicaset_details_window(ctx, &mut replicaset_details_window, replicaset_details_clone, replicasets_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Job details window
        if job_details_window.show {
            let job_details_clone = Arc::clone(&job_details);
            let jobs_clone = Arc::clone(&jobs);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_job_details_window(ctx, &mut job_details_window, job_details_clone, jobs_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // CronJob details window
        if cronjob_details_window.show {
            let cronjob_details_clone = Arc::clone(&cronjob_details);
            let cronjobs_clone = Arc::clone(&cronjobs);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_cronjob_details_window(ctx, &mut cronjob_details_window, cronjob_details_clone, cronjobs_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // StatefulSet details window
        if statefulset_details_window.show {
            let statefulset_details_clone = Arc::clone(&statefulset_details);
            let statefulsets_clone = Arc::clone(&statefulsets);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_statefulset_details_window(ctx, &mut statefulset_details_window, statefulset_details_clone, statefulsets_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // ConfigMap details window
        if configmap_details_window.show {
            let configmap_details_clone = Arc::clone(&configmap_details);
            let configmaps_clone = Arc::clone(&configmaps);
            let yaml_editor_window_clone = Arc::clone(&yaml_editor_window);
            let client_clone = Arc::clone(&client);
            show_configmap_details_window(ctx, &mut configmap_details_window, configmap_details_clone, configmaps_clone, yaml_editor_window_clone, client_clone, &mut confirmation_dialog);
        }

        // Scale window
        if scale_window.show {
            let client_clone = Arc::clone(&client);
            show_scale_window(ctx, &mut scale_window, client_clone);
        }

        // Confirmation dialog
        show_delete_confirmation(ctx, &mut confirmation_dialog);

        ctx.request_repaint();
    })
    .unwrap();
}
