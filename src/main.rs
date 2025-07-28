use eframe::egui::{CursorIcon, Direction, Ui};
use eframe::*;
use egui::{Color32, Context, FontId, ScrollArea, TextStyle};
use std::f32;
use std::sync::{Arc, Mutex};
use kube::{Client};
use std::sync::atomic::{AtomicBool, Ordering};

mod functions;
use functions::*;

mod templates;
use templates::*;

mod items;
use items::*;

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");
const GREEN_BUTTON: Color32 = Color32::from_rgb(0x4C, 0xAF, 0x50); // green
const RED_BUTTON: Color32 = Color32::from_rgb(0xF4, 0x43, 0x36); // red
const ORANGE_BUTTON: Color32 = Color32::ORANGE; // orange
const BLUE_BUTTON: Color32 = Color32::LIGHT_BLUE; // blue
const MENU_BUTTON: Color32 = Color32::from_rgb(147, 38, 245);
const ACTIONS_MENU_BUTTON_SIZE: f32 = 10.0;
const MAX_LOG_LINES: usize = 7; // DEBUG

const ACTIONS_MENU_LABEL: &str = "🔻";

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
    Secret,
    ExternalSecret,
}

struct LogWindow {
    pod_name: String,
    containers: Vec<ContainerStatusItem>,
    show: bool,
    namespace: String,
    selected_container: String,
    buffer: Arc<Mutex<String>>,
    last_container: Option<String>,
}

impl LogWindow {
    fn new() -> Self {
        Self {
            pod_name: String::new(),
            containers: Vec::new(),
            selected_container: String::new(),
            namespace: String::new(),
            show: false,
            buffer: Arc::new(Mutex::new(String::new())),
            last_container: None,
        }
    }
}

struct YamlEditorWindow {
    content: String,
    show: bool,
    search_query: String,
}

impl YamlEditorWindow {
    fn new() -> Self {
        Self {
            content: String::new(),
            show: false,
            search_query: String::new(),
        }
    }
}

struct ScaleWindow {
    name: Option<String>,
    namespace: Option<String>,
    cur_replicas: i32,
    desired_replicas: i32,
    show: bool,
    resource_kind: Option<ScaleTarget>,
}

impl ScaleWindow {
    fn new() -> Self {
        Self {
            name: None,
            namespace: None,
            cur_replicas: 0,
            desired_replicas: 0,
            show: false,
            resource_kind: None,
        }
    }
}

struct NewResourceWindow {
    resource_type: ResourceType,
    content: String,
    show: bool,
}

impl NewResourceWindow {
    fn new() -> Self {
        Self {
            resource_type: ResourceType::Blank,
            content: String::new(),
            show: false,
        }
    }
}

fn show_loading(ui: &mut Ui) {
    ui.with_layout(
        egui::Layout::centered_and_justified(Direction::TopDown),
        |ui| {
            ui.spinner();
            //ui.label(egui::RichText::new("⏳ Loading...").heading());
        },
    );
}

fn show_empty(ui: &mut Ui) {
    ui.with_layout(
        egui::Layout::centered_and_justified(Direction::TopDown),
        |ui| {
            ui.label(egui::RichText::new("😟 Empty").heading());
        },
    );
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

    let mut new_resource_window = NewResourceWindow::new();
    let mut scale_window = ScaleWindow::new();
    let mut log_window = LogWindow::new();
    let yaml_editor_window = Arc::new(Mutex::new(YamlEditorWindow::new()));

    //####################################################//
    let mut sort_by = SortBy::Age;
    let mut sort_asc = false;

    let ctx_info = get_current_context_info().unwrap();
    let cluster_name = ctx_info.context.unwrap().cluster;
    let user_name = ctx_info.name;

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
    let mut filter_statefulsets = String::new();
    let mut filter_jobs = String::new();
    let mut filter_pvcs = String::new();
    let mut filter_pvs = String::new();
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
    let endpoints_loading = Arc::new(AtomicBool::new(true));
    spawn_watcher(
        Arc::clone(&client),
        Arc::clone(&endpoints),
        Arc::clone(&endpoints_loading),
        |c, s, l| {
        Box::pin(watch_endpoints(c, s, l))
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

    // CRONJOBS
    let cronjobs = Arc::new(Mutex::new(Vec::<CronJobItem>::new()));
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
            (TextStyle::Small, FontId::new(12.0, egui::FontFamily::Proportional)),
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

                egui::CollapsingHeader::new("🛠 Config").default_open(true).show(ui, |ui| {
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

                egui::CollapsingHeader::new("🖧 Network").default_open(true).show(ui, |ui| {
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

                egui::CollapsingHeader::new("🖴 Storage").default_open(true).show(ui, |ui| {
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

                egui::CollapsingHeader::new("🖥 Administration").default_open(true).show(ui, |ui| {
                    if ui.selectable_label(current == Category::CustomResourcesDefinitions, "📢 Custom Resources Definitions").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::CustomResourcesDefinitions;
                    }
                });

                egui::CollapsingHeader::new("⎈ Helm").default_open(true).show(ui, |ui| {
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
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match *selected_category_ui.lock().unwrap() {
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
                                        if item.namespace.is_some() {
                                            ui.label(format!("{}", item.namespace.as_ref().unwrap()));
                                        } else {
                                            ui.label("");
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
                                ui.label("Group");
                                ui.label("Version");
                                ui.label("Scope");
                                ui.label("Kind");
                                ui.label("Age");
                                ui.end_row();
                                for item in crds_list.iter().rev().take(200) {
                                    let cur_item_object = &item.name;
                                    if filter_crds.is_empty() || cur_item_object.contains(&filter_crds) {
                                        ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                        ui.label(format!("{}", &item.group));
                                        ui.label(format!("{}", &item.version));
                                        ui.label(format!("{}", &item.scope));
                                        ui.label(format!("{}", &item.kind));
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
                                    ui.label("Pod selecter");
                                    ui.label("Types");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_network_policies.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_network_policies.is_empty() || cur_item_object.contains(&filter_network_policies) {
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(&item.pod_selector);
                                            ui.label(&item.policy_types);
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::networking::v1::NetworkPolicy>(client,&ns, &name).await {
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
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(&item.min_available.clone().unwrap_or_else(|| "-".to_string()));
                                            ui.label(&item.max_unavailable.clone().unwrap_or_else(|| "-".to_string()));
                                            ui.label(format!("{} / {}", &item.current_healthy, &item.desired_healthy));
                                            ui.label(format!("{}", &item.allowed_disruptions));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::policy::v1::PodDisruptionBudget>(client,&ns, &name).await {
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
                                    ui.label("Desired");
                                    ui.label("Current");
                                    ui.label("Ready");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_daemonsets.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_daemonsets.is_empty() || cur_item_object.contains(&filter_daemonsets) {
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}", &item.desired));
                                            ui.label(format!("{}", &item.current));
                                            ui.label(format!("{}", &item.ready));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::apps::v1::DaemonSet>(client,&ns, &name).await {
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
                Category::ClusterOverview => {
                    ui.heading("Cluster Overview");
                    ui.separator();
                    let cluster = cluster_info_ui.lock().unwrap().clone();
                    ui.vertical(|ui| {
                        ui.label(format!("Connected to: {}", cluster.name));
                        ui.label(format!("Cluster name: {}", cluster_name));
                        ui.label(format!("User name: {}", user_name));
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
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
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
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::apps::v1::ReplicaSet>(client,&ns, &name).await {
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
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}", &item.host));
                                            ui.label(format!("{}", &item.paths));
                                            ui.label(format!("{}", &item.service));
                                            ui.label(format!("{}", &item.tls));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::networking::v1::Ingress>(client,&ns, &name).await {
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
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
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
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}", &item.storage_class));
                                            ui.label(format!("{}", &item.volume_name));
                                            ui.label(format!("{}", &item.size));
                                            ui.label(egui::RichText::new(&item.status).color(item_color(&item.status)));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::core::v1::PersistentVolumeClaim>(client,&ns, &name).await {
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
                                    ui.label("Addresses");
                                    ui.label("Ports");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_endpoints.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_endpoints.is_empty() || cur_item_object.contains(&filter_endpoints) {
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}", &item.addresses));
                                            ui.label(format!("{:?}", &item.ports));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::core::v1::Endpoints>(client,&ns, &name).await {
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
                                    ui.label("Completions");
                                    ui.label("Conditions");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_jobs.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_jobs.is_empty() || cur_item_object.contains(&filter_jobs) {
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}", &item.completions));
                                            ui.label(egui::RichText::new(&item.condition).color(item_color(&item.condition)));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::batch::v1::Job>(client,&ns, &name).await {
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
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
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
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::core::v1::Service>(client,&ns, &name).await {
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
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}", &item.schedule));
                                            ui.label(format!("{}", &item.suspend));
                                            ui.label(format!("{}", &item.active));
                                            ui.label(format!("{}", &item.last_schedule));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::batch::v1::CronJob>(client,&ns, &name).await {
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
                                    ui.label("Ready");
                                    ui.label("Service name");
                                    ui.label("Age");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_statefulsets.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_statefulsets.is_empty() || cur_item_object.contains(&filter_statefulsets) {
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
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
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::apps::v1::StatefulSet>(client,&ns, &name).await {
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
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
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
                                        // let running_on_node = item.node_name.as_ref().unwrap();
                                        // if filter_pods.is_empty() || cur_item_name.contains(&filter_pods) || running_on_node.contains(&filter_pods) {
                                        if filter_pods.is_empty() || cur_item_name.contains(&filter_pods) {
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
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
                                                    ready_color = Color32::from_rgb(100, 255, 100); // green
                                                },
                                                "Terminating" => {
                                                    status = "🗑 Terminating".to_string();
                                                    ready_color = Color32::from_rgb(128, 128, 128); // gray
                                                },
                                                "Pending" => {
                                                    status = "⏳ Pending".to_string();
                                                    ready_color = Color32::from_rgb(255, 165, 0); // orange
                                                },
                                                "Succeeded" => {
                                                    status = "✅ Completed".to_string();
                                                    ready_color = Color32::from_rgb(0, 255, 176); // green
                                                },
                                                "Failed" => {
                                                    status = "❌ Failed".to_string();
                                                    ready_color = Color32::from_rgb(255, 0, 0); // red
                                                },
                                                "CrashLoopBackOff" => {
                                                    status = "💥 CrashLoop".to_string();
                                                    ready_color = Color32::from_rgb(255, 0, 0); // red
                                                },
                                                "Cancelled" => {
                                                    status = "🚫 Cancelled".to_string();
                                                    ready_color = Color32::from_rgb(128, 128, 128); // gray
                                                },
                                                _ => {
                                                    status = "❓ Unknown".to_string();
                                                    ready_color = Color32::GRAY;
                                                },
                                            };

                                            ui.label(egui::RichText::new(status).color(ready_color));
                                            let ready = item.ready_containers;
                                            let total = item.total_containers;

                                            ready_color = if ready == total {
                                                Color32::from_rgb(100, 255, 100) // green
                                            } else if ready == 0 {
                                                Color32::from_rgb(255, 100, 100) // red
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
                                            ui.label(item.node_name.clone().unwrap_or("-".into()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(RED_BUTTON)).clicked() {
                                                    let cur_pod = item.name.clone();
                                                    let cur_ns = selected_ns.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    tokio::spawn(async move {
                                                        if let Err(err) = delete_pod(client_clone, &cur_pod.clone(), cur_ns.as_deref(), true).await {
                                                            eprintln!("Failed to delete pod: {}", err);
                                                        }
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::core::v1::Pod>(client, &ns, &name).await {
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
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button("📃 Logs").clicked() {
                                                    let cur_pod = item.name.clone();
                                                    log_window.pod_name = item.name.clone();

                                                    let cur_ns = selected_ns.clone();
                                                    log_window.namespace = selected_ns.clone().unwrap();

                                                    let cur_container = item.containers[0].name.clone();
                                                    log_window.selected_container = item.containers[0].name.clone();
                                                    log_window.last_container = None;

                                                    log_window.containers = item.containers.clone();

                                                    log_window.buffer = Arc::new(Mutex::new(String::new()));
                                                    let buf_clone = Arc::clone(&log_window.buffer);
                                                    log_window.show = true;
                                                    let client_clone = Arc::clone(&client);
                                                    tokio::spawn(async move {
                                                        fetch_logs(client_clone,
                                                        cur_ns.unwrap().as_str(),
                                                        cur_pod.as_str(),
                                                        cur_container.as_str(), buf_clone).await;
                                                    });
                                                    ui.close_kind(egui::UiKind::Menu);
                                                }
                                                if ui.button(egui::RichText::new("🖵 Shell").size(16.0).color(ORANGE_BUTTON)).clicked() {
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
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}/{}", &item.ready_replicas, &item.replicas));
                                            ui.label(format!("{}", &item.replicas));
                                            ui.label(format!("{}", &item.updated_replicas));
                                            ui.label(format!("{}", &item.available_replicas));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::apps::v1::Deployment>(client, &ns, &name).await {
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
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = selected_ns.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    tokio::spawn(async move {
                                                        if let Err(err) = delete_secret(client_clone, &cur_item.clone(), cur_ns.as_deref()).await {
                                                            eprintln!("Failed to delete deployment: {}", err);
                                                        }
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
                                    ui.label("Type");
                                    ui.label("Age");
                                    ui.label("Labels"); // Limit width!
                                    ui.label("Keys");
                                    ui.label("Actions");
                                    ui.end_row();
                                    for item in visible_secrets.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_secrets.is_empty() || cur_item_object.contains(&filter_secrets) {
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}", &item.type_));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.label(format!("{}", &item.labels));
                                            ui.label(format!("{}", &item.keys));
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::core::v1::Secret>(client, &ns, &name).await {
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
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = selected_ns.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    tokio::spawn(async move {
                                                        if let Err(err) = delete_secret(client_clone, &cur_item.clone(), cur_ns.as_deref()).await {
                                                            eprintln!("Failed to delete secret: {}", err);
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
                                    ui.label("Type");
                                    ui.label("Age");
                                    ui.label("Labels");
                                    ui.label("Keys");
                                    ui.label("Actions");
                                    ui.end_row();

                                    for item in visible_configmaps.iter().rev().take(200) {
                                        let cur_item_object = &item.name;
                                        if filter_configmaps.is_empty() || cur_item_object.contains(&filter_configmaps) {
                                            ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                            ui.label(format!("{}", &item.type_));
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.label(format!("{:?}", &item.labels));
                                            ui.label(format!("{}", &item.keys.join(", ")));

                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);

                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let ns = item.namespace.clone().unwrap_or_else(|| "default".to_string());
                                                    let name = item.name.clone();
                                                    let client = client.clone();
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::core::v1::ConfigMap>(client, &ns, &name).await {
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
                                                    let cur_item = item.name.clone();
                                                    let cur_ns = selected_ns.clone();
                                                    let client_clone = Arc::clone(&client);
                                                    tokio::spawn(async move {
                                                        if let Err(err) = delete_configmap(client_clone, &cur_item.clone(), cur_ns.as_deref()).await {
                                                            eprintln!("Failed to delete configmap: {}", err);
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
                                            ui.label(
                                                egui::RichText::new(&item.event_type).color(item_color(&item.event_type)),
                                            );
                                            ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                            ui.label(&item.namespace);
                                            ui.label(&item.reason);
                                            ui.label(&item.involved_object);
                                            ui.label(&item.message);
                                            ui.menu_button(egui::RichText::new(ACTIONS_MENU_LABEL).size(ACTIONS_MENU_BUTTON_SIZE).color(MENU_BUTTON), |ui| {
                                                ui.set_width(200.0);
                                                if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(GREEN_BUTTON)).clicked() {
                                                    let name = item.involved_object.clone();
                                                    let ns = item.namespace.clone();
                                                    let client = client.clone();
                                                    let yaml_editor_window = Arc::clone(&yaml_editor_window);
                                                    tokio::spawn(async move {
                                                        match get_yaml_namespaced::<k8s_openapi::api::core::v1::Event>(client, &ns, &name).await {
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
            }
        });

        if let Ok(mut editor) = yaml_editor_window.lock() {
            if editor.show {
                egui::Window::new("Edit resource").max_width(1200.0).max_height(600.0).collapsible(false).resizable(true).show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("🔍");
                        ui.add(egui::TextEdit::singleline(&mut editor.search_query)
                            .hint_text("Search...")
                            .desired_width(200.0),
                        );
                        if ui.button("×").clicked() {
                            editor.search_query.clear();
                        }
                    });
                    ui.separator();

                    let search = editor.search_query.clone();
                    egui::ScrollArea::vertical().hscroll(true).show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(&mut editor.content)
                            .font(TextStyle::Monospace)
                            .code_editor()
                            .layouter(&mut search_layouter(search)),
                        );
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(egui::RichText::new("✅ Save").size(16.0).color(egui::Color32::GREEN)).clicked() {
                            let content = editor.content.clone();

                            match serde_yaml::from_str::<serde_yaml::Value>(&content) {
                                Ok(_) => {
                                    // YAML is valid!
                                    let client_clone = Arc::clone(&client);
                                    tokio::spawn(async move {
                                        if let Err(e) = patch_resource(client_clone, content.as_str()).await {
                                            println!("Error applying YAML: {:?}", e);
                                        }
                                    });
                                    editor.show = false;
                                }
                                Err(e) => {
                                    eprintln!("YAML Error: {}", e);
                                }
                            }
                        }
                        if ui.button(egui::RichText::new("🗙 Cancel").size(16.0).color(egui::Color32::RED)).clicked() {
                            editor.show = false;
                        }
                    });
                });
            }
        }

        // New resource creation window
        if new_resource_window.show {
            egui::Window::new("Create New Resource").collapsible(false).resizable(true).show(ctx, |ui| {
                if new_resource_window.content.is_empty() {
                    new_resource_window.content = match new_resource_window.resource_type {
                        ResourceType::NameSpace => NAMESPACE_TEMPLATE.to_string(),
                        ResourceType::Pod => POD_TEMPLATE.to_string(),
                        ResourceType::Secret => SECRET_TEMPLATE.to_string(),
                        ResourceType::ExternalSecret => EXTERNAL_SECRET_TEMPLATE.to_string(),
                        ResourceType::PersistenceVolumeClaim => PVC_TEMPLATE.to_string(),
                        ResourceType::Blank => "".to_string(),
                    };
                }

                ui.horizontal(|ui| {
                    ui.label("YAML Template:");
                    egui::ComboBox::from_id_salt("templates_combo").width(150.0)
                        .selected_text(match new_resource_window.resource_type {
                            ResourceType::NameSpace => "NameSpace",
                            ResourceType::Secret => "Secret",
                            ResourceType::ExternalSecret => "External secret",
                            ResourceType::Pod => "Pod",
                            ResourceType::PersistenceVolumeClaim => "PersistenceVolumeClaim",
                            ResourceType::Blank => "Blank",
                        }).show_ui(ui, |ui| {
                            if ui.selectable_value(&mut new_resource_window.resource_type, ResourceType::NameSpace, "NameSpace",).clicked() {
                                new_resource_window.content = NAMESPACE_TEMPLATE.to_string();
                            };
                            if ui.selectable_value(&mut new_resource_window.resource_type, ResourceType::Secret, "Secret",).clicked() {
                                new_resource_window.content = SECRET_TEMPLATE.to_string();
                            };
                            if ui.selectable_value(&mut new_resource_window.resource_type, ResourceType::Pod, "Pod",).clicked() {
                                new_resource_window.content = POD_TEMPLATE.to_string();
                            };
                            if ui.selectable_value(&mut new_resource_window.resource_type, ResourceType::ExternalSecret, "External secret",).clicked() {
                                new_resource_window.content = EXTERNAL_SECRET_TEMPLATE.to_string();
                            };
                            if ui.selectable_value(&mut new_resource_window.resource_type, ResourceType::PersistenceVolumeClaim,"PersistenceVolumeClaim",).clicked() {
                                new_resource_window.content = PVC_TEMPLATE.to_string();
                            };
                            if ui.selectable_value(&mut new_resource_window.resource_type, ResourceType::Blank,"Blank",).clicked() {
                                new_resource_window.content = "".to_string();
                            };
                        });
                });
                ui.separator();
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut new_resource_window.content)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .text_color(egui::Color32::LIGHT_YELLOW)
                        .desired_rows(25)
                        .lock_focus(true)
                        .desired_width(f32::INFINITY),
                    );
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new("✔ Apply").size(16.0).color(egui::Color32::GREEN)).clicked() {
                        let yaml = new_resource_window.content.clone();
                        let client_clone = Arc::clone(&client);
                        let resource_type = new_resource_window.resource_type.clone();
                        tokio::spawn(async move {
                            if let Err(e) = apply_yaml(client_clone, &yaml, resource_type).await {
                                println!("Error applying YAML: {:?}", e);
                            }
                        });
                        new_resource_window.show = false;
                    }

                    if ui.button(egui::RichText::new("🗙 Cancel").size(16.0).color(egui::Color32::RED)).clicked() {
                        new_resource_window.show = false;
                    }
                });
            });
        }

        if log_window.show {
            egui::Window::new("Logs")
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Container:");
                        egui::ComboBox::from_id_salt("containers_combo")
                            .selected_text(&log_window.selected_container)
                            .width(150.0)
                            .show_ui(ui, |ui| {
                                for container in &log_window.containers {
                                    ui.selectable_value(
                                        &mut log_window.selected_container,
                                        container.name.clone(),
                                        &container.name,
                                    );
                                }
                            });
                    });

                    if log_window.last_container.as_ref() != Some(&log_window.selected_container) {
                        log_window.last_container = Some(log_window.selected_container.clone());
                        let buf_clone = Arc::clone(&log_window.buffer);
                        let cur_ns = log_window.namespace.clone();
                        let cur_pod = log_window.pod_name.clone();
                        let cur_container = log_window.selected_container.clone();
                        let client_clone = Arc::clone(&client);
                        tokio::spawn(async move {
                            fetch_logs(client_clone,
                            &cur_ns,
                             &cur_pod,
                            &cur_container, buf_clone).await;
                        });
                    }

                    ScrollArea::vertical().show(ui, |ui| {
                        if let Ok(logs) = log_window.buffer.lock() {
                            ui.add(
                                egui::TextEdit::multiline(&mut logs.clone())
                                    .font(TextStyle::Monospace)
                                    .desired_rows(MAX_LOG_LINES)
                                    .desired_width(f32::INFINITY)
                                    .code_editor()
                            );
                        }
                    });

                    ui.separator();
                    if ui.button(egui::RichText::new("🗙 Close logs window").size(16.0).color(egui::Color32::WHITE)).clicked() {
                        log_window.show = false;
                    }
            });
        }

        if scale_window.show {
            let title = format!("Scale {}", scale_window.name.as_ref().unwrap());
            egui::Window::new(title).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Current replicas scale: {}", scale_window.cur_replicas));
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Desired number of replicas:");
                    ui.add(egui::Slider::new(&mut scale_window.desired_replicas, 0..=10));
                });
                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new("🗙 Cancel").size(16.0).color(egui::Color32::WHITE)).clicked() {
                        scale_window.show = false;
                    }
                    if ui.button(egui::RichText::new("↕ Scale").size(16.0).color(egui::Color32::ORANGE)).clicked() {
                        eprintln!("Scaling {} to {} replicas", scale_window.name.as_ref().unwrap(), scale_window.desired_replicas);
                        let client_clone = Arc::clone(&client);
                        let name = scale_window.name.as_ref().unwrap().clone();
                        let namespace = scale_window.namespace.as_ref().unwrap().clone();
                        let replicas = scale_window.desired_replicas;
                        let kind = scale_window.resource_kind.as_ref().unwrap().clone();
                        tokio::spawn(async move {
                            if let Err(e) = scale_workload(client_clone, &name, &namespace, replicas, kind).await {
                                eprintln!("Scale failed: {:?}", e);
                            }
                        });
                        scale_window.show = false;
                    }
                });
            });
        }

        ctx.request_repaint();
    })
    .unwrap();
}
