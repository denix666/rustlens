use eframe::egui::{CursorIcon};
use eframe::*;
use egui::{Context, Style, TextStyle, FontId, Color32, ScrollArea};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use kube::Client;

mod functions;
use functions::*;

mod templates;
use templates::*;

mod items;
use items::*;

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");
const GREEN_BUTTON: Color32 = Color32::from_rgb(0x4C, 0xAF, 0x50);
const RED_BUTTON: Color32 = Color32::from_rgb(0xF4, 0x43, 0x36);
const MAX_LOG_LINES: usize = 7; // DEBUG

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
}

#[derive(Clone)]
struct ClusterInfo {
    name: String,
}

#[derive(PartialEq)]
enum ResourceType {
    Blank,
    NameSpace,
    PersistenceVolumeClaim,
    Pod,
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

#[tokio::main]
async fn main() {
    let mut title = String::from("RustLens v");
    title.push_str(env!("CARGO_PKG_VERSION"));
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 600.0])
            .with_maximize_button(false),
        ..Default::default()
    };

    if let Ok(icon) = load_embedded_icon() {
        options.viewport = options.viewport.with_icon(icon);
    }

    let mut new_resource_window = NewResourceWindow::new();
    let mut log_window = LogWindow::new();

    //####################################################//
    let mut sort_by = SortBy::Name;
    let mut sort_asc = true;

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

    // Client connection
    let client = Arc::new(Client::try_default().await.unwrap());

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

    // PVC
    let pvcs = Arc::new(Mutex::new(Vec::new()));
    spawn_namespace_watcher_loop(
        Arc::clone(&client),
        Arc::clone(&pvcs),
        Arc::clone(&selected_namespace),
        Arc::new(|client, pvcs, ns| {
            tokio::spawn(async move {
                watch_pvcs(client, pvcs, ns).await;
            });
        }),
        Duration::from_secs(1),
    );

    // JOBS
    let jobs = Arc::new(Mutex::new(Vec::new()));
    spawn_namespace_watcher_loop(
        Arc::clone(&client),
        Arc::clone(&jobs),
        Arc::clone(&selected_namespace),
        Arc::new(|client, jobs, ns| {
            tokio::spawn(async move {
                watch_jobs(client, jobs, ns).await;
            });
        }),
        Duration::from_secs(1),
    );

    // PV
    let pvs = Arc::new(Mutex::new(Vec::new()));
    let client_clone = Arc::clone(&client);
    let pvs_clone = Arc::clone(&pvs);
    tokio::spawn(async move {
        watch_pvs(client_clone, pvs_clone).await;
    });

    // SC
    let storage_classes = Arc::new(Mutex::new(Vec::new()));
    let client_clone = Arc::clone(&client);
    let sc_clone = Arc::clone(&storage_classes);
    tokio::spawn(async move {
        watch_storage_classes(client_clone, sc_clone).await;
    });

    // EVENTS
    let events = Arc::new(Mutex::new(Vec::<EventItem>::new()));
    let events_clone = Arc::clone(&events);
    let client_clone = Arc::clone(&client);
    tokio::spawn(async move {
        watch_events(client_clone, events_clone).await;
    });

    // STATEFULSETS
    let statefulsets = Arc::new(Mutex::new(Vec::new()));
    spawn_namespace_watcher_loop(
        Arc::clone(&client),
        Arc::clone(&statefulsets),
        Arc::clone(&selected_namespace),
        Arc::new(|client, ss, ns| {
            tokio::spawn(async move {
                watch_statefulsets(client, ss, ns).await;
            });
        }),
        Duration::from_secs(1),
    );

    // REPLICASETS
    let replicasets = Arc::new(Mutex::new(Vec::new()));
    spawn_namespace_watcher_loop(
        Arc::clone(&client),
        Arc::clone(&replicasets),
        Arc::clone(&selected_namespace),
        Arc::new(|client, rs, ns| {
            tokio::spawn(async move {
                watch_replicasets(client, rs, ns).await;
            });
        }),
        Duration::from_secs(1),
    );

    // DEPLOYMENTS
    let deployments = Arc::new(Mutex::new(Vec::new()));
    spawn_namespace_watcher_loop(
        Arc::clone(&client),
        Arc::clone(&deployments),
        Arc::clone(&selected_namespace),
        Arc::new(|client, deployments, ns| {
            tokio::spawn(async move {
                watch_deployments(client, deployments, ns).await;
            });
        }),
        Duration::from_secs(1),
    );

    // SECRETS
    let secrets = Arc::new(Mutex::new(Vec::new()));
    spawn_namespace_watcher_loop(
        Arc::clone(&client),
        Arc::clone(&secrets),
        Arc::clone(&selected_namespace),
        Arc::new(|client, secrets, ns| {
            tokio::spawn(async move {
                watch_secrets(client, secrets, ns).await;
            });
        }),
        Duration::from_secs(1),
    );

    // CONFIGMAPS
    let configmaps = Arc::new(Mutex::new(Vec::new()));
    spawn_namespace_watcher_loop(
        Arc::clone(&client),
        Arc::clone(&configmaps),
        Arc::clone(&selected_namespace),
        Arc::new(|client, configmaps, ns| {
            tokio::spawn(async move {
                watch_configmaps(client, configmaps, ns).await;
            });
        }),
        Duration::from_secs(1),
    );

    // NODES
    let nodes = Arc::new(Mutex::new(Vec::<NodeItem>::new()));
    let node_clone = Arc::clone(&nodes);
    let client_clone = Arc::clone(&client);
    tokio::spawn(async move {
        watch_nodes(client_clone, node_clone).await;
    });

    // NAMESPACES
    let namespaces = Arc::new(Mutex::new(Vec::<NamespaceItem>::new()));
    let ns_clone = Arc::clone(&namespaces);
    let client_clone = Arc::clone(&client);
    tokio::spawn(async move {
        watch_namespaces(client_clone, ns_clone).await;
    });


    // PODS
    let pods = Arc::new(Mutex::new(Vec::<PodItem>::new()));
    spawn_namespace_watcher_loop(
        Arc::clone(&client),
        Arc::clone(&pods),
        Arc::clone(&selected_namespace),
        Arc::new(|client, pods, ns| {
            tokio::spawn(async move {
                watch_pods(client, pods, ns).await;
            });
        }),
        Duration::from_secs(1),
    );

    eframe::run_simple_native(&title, options, move |ctx: &Context, _frame| {
        // Setup style
        let mut style: Style = (*ctx.style()).clone();

        // Increase font size for different TextStyle
        style.text_styles = [
            (TextStyle::Heading, FontId::new(24.0, egui::FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(18.0, egui::FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(16.0, egui::FontFamily::Monospace)),
            (TextStyle::Button, FontId::new(18.0, egui::FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(14.0, egui::FontFamily::Proportional)),
        ]
        .into();

        ctx.set_style(style);

        egui::SidePanel::left("tasks panel").resizable(false).exact_width(280.0).show(ctx, |ui| {
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
                });

                egui::CollapsingHeader::new("🖧 Network").default_open(true).show(ui, |ui| {
                    if ui.selectable_label(current == Category::Services, "💢 Services").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Services;
                    }

                    if ui.selectable_label(current == Category::Endpoints, "⛺ Endpoints").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Endpoints;
                    }

                    if ui.selectable_label(current == Category::Ingresses, "⤵ Ingresses").clicked() {
                        *selected_category_ui.lock().unwrap() = Category::Ingresses;
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
                });

                egui::CollapsingHeader::new("⎈ Helm").default_open(true).show(ui, |ui| {
                    ui.label("📰 Charts");
                    ui.label("📥 Releases");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match *selected_category_ui.lock().unwrap() {
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
                    ui.horizontal(|ui| {
                        ui.heading(format!("ReplicaSets - {}", replicasets.lock().unwrap().len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("default")).width(150.0).show_ui(ui, |ui| {
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
                    let replicasets_list = replicasets.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("replicasets_scroll").show(ui, |ui| {
                        egui::Grid::new("replicasets_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Desired");
                            ui.label("Current");
                            ui.label("Ready");
                            ui.label("Age");
                            ui.end_row();
                            for item in replicasets_list.iter().rev().take(200) {
                                let cur_item_object = &item.name;
                                if filter_replicasets.is_empty() || cur_item_object.contains(&filter_replicasets) {
                                    ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                    ui.label(format!("{}", &item.desired));
                                    ui.label(format!("{}", &item.current));
                                    ui.label(format!("{}", &item.ready));
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                    ui.end_row();
                                }
                            }
                        });
                    });
                },
                Category::Ingresses => {
                    ui.heading("Ingresses (TODO)");
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
                    let scs_list = storage_classes.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("scs_scroll").show(ui, |ui| {
                        egui::Grid::new("scs_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Provisioner");
                            ui.label("Reclaim policy");
                            ui.label("Volume binding mode");
                            ui.label("Default class");
                            ui.label("Age");
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
                                    ui.end_row();
                                }
                            }
                        });
                    });
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
                    let pvs_list = pvs.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("pvs_scroll").show(ui, |ui| {
                        egui::Grid::new("pvs_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Storage class");
                            ui.label("Capacity");
                            ui.label("Claim");
                            ui.label("Status");
                            ui.label("Age");
                            ui.end_row();
                            for item in pvs_list.iter().rev().take(200) {
                                let cur_item_object = &item.name;
                                let cur_item_claim = &item.claim;
                                if filter_pvs.is_empty() || cur_item_object.contains(&filter_pvs) || cur_item_claim.contains(&filter_pvs) {
                                    ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                    ui.label(format!("{}", &item.storage_class));
                                    ui.label(format!("{}", &item.capacity));
                                    ui.label(format!("{}", &item.claim));
                                    ui.label(format!("{}", &item.status));
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                    ui.end_row();
                                }
                            }
                        });
                    });
                },
                Category::PersistentVolumeClaims => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    ui.horizontal(|ui| {
                        ui.heading(format!("PersistentVolumeClaims - {}", pvcs.lock().unwrap().len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("default")).width(150.0).show_ui(ui, |ui| {
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
                    let pvcs_list = pvcs.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("pvcs_scroll").show(ui, |ui| {
                        egui::Grid::new("pvcs_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("StorageClass");
                            ui.label("Volume");
                            ui.label("Size");
                            ui.label("Status");
                            ui.label("Age");
                            ui.end_row();
                            for item in pvcs_list.iter().rev().take(200) {
                                let cur_item_object = &item.name;
                                if filter_pvcs.is_empty() || cur_item_object.contains(&filter_pvcs) {
                                    ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                    ui.label(format!("{}", &item.storage_class));
                                    ui.label(format!("{}", &item.volume_name));
                                    ui.label(format!("{}", &item.size));
                                    ui.label(format!("{}", &item.status));
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                    ui.end_row();
                                }
                            }
                        });
                    });
                },
                Category::Endpoints => {
                    ui.heading("Endpoints (TODO)");
                },
                Category::Jobs => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    ui.horizontal(|ui| {
                        ui.heading(format!("Jobs - {}", jobs.lock().unwrap().len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("default")).width(150.0).show_ui(ui, |ui| {
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
                    let jobs_list = jobs.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("jobs_scroll").show(ui, |ui| {
                        egui::Grid::new("jobs_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Completions");
                            ui.label("Conditions");
                            ui.label("Age");
                            ui.end_row();
                            for item in jobs_list.iter().rev().take(200) {
                                let cur_item_object = &item.name;
                                if filter_jobs.is_empty() || cur_item_object.contains(&filter_jobs) {
                                    ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                    ui.label(format!("{}", &item.completions));
                                    ui.label(format!("{:?}", &item.conditions));
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                    ui.end_row();
                                }
                            }
                        });
                    });
                },
                Category::Services => {
                    ui.heading("Services (TODO)");
                },
                Category::CronJobs => {
                    ui.heading("CronJobs (TODO)");
                },
                Category::StatefulSets => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    ui.horizontal(|ui| {
                        ui.heading(format!("StatefulSets - {}", statefulsets.lock().unwrap().len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("default")).width(150.0).show_ui(ui, |ui| {
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
                    let statefulsets_list = statefulsets.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("statefulsets_scroll").show(ui, |ui| {
                        egui::Grid::new("statefulsets_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Ready");
                            ui.label("Service name");
                            ui.label("Age");
                            ui.end_row();
                            for item in statefulsets_list.iter().rev().take(200) {
                                let cur_item_object = &item.name;
                                if filter_statefulsets.is_empty() || cur_item_object.contains(&filter_statefulsets) {
                                    ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                    ui.label(format!("{}/{}", &item.ready_replicas, &item.replicas));
                                    ui.label(egui::RichText::new(&item.service_name).italics().color(egui::Color32::CYAN));
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                    ui.end_row();
                                }
                            }
                        });
                    });
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
                                    ui.label(&item.name);
                                    ui.add(egui::ProgressBar::new(item.cpu_percent / 100.0).show_percentage());
                                    ui.add(egui::ProgressBar::new(item.mem_percent / 100.0).show_percentage());
                                    ui.label(&item.storage.as_ref().unwrap().to_string());

                                    if let Some(taints) = &item.taints {
                                        ui.label(taints.len().to_string())
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text(format!("{:?}", taints));
                                    } else {
                                        ui.label("0");
                                    }
                                    ui.label(format!("{}", item.roles.join(", ")));
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));

                                    let node_status = egui::RichText::new(&item.status).color(match item.status.as_str() {
                                        "Ready" => egui::Color32::GREEN,
                                        "NotReady" => egui::Color32::RED,
                                        _ => egui::Color32::YELLOW,
                                    });
                                    let scheduling_status = match item.scheduling_disabled {
                                        true => egui::RichText::new("SchedulingDisabled").color(egui::Color32::ORANGE),
                                        false => egui::RichText::new(""),
                                    };

                                    ui.label( node_status);
                                    ui.label( scheduling_status);

                                    ui.menu_button("⚙", |ui| {
                                        ui.set_width(200.0);
                                        let node_name = item.name.clone();
                                        if item.scheduling_disabled {
                                            if ui.button("▶ Uncordon").clicked() {
                                                tokio::spawn(async move {
                                                    if let Err(err) = cordon_node(&node_name, false).await {
                                                        eprintln!("Failed to uncordon node: {}", err);
                                                    }
                                                });
                                                ui.close_menu();
                                            }
                                        } else {
                                            if ui.button("⏸ Cordon").clicked() {
                                                tokio::spawn(async move {
                                                    if let Err(err) = cordon_node(&node_name, true).await {
                                                        eprintln!("Failed to cordon node: {}", err);
                                                    }
                                                });
                                                ui.close_menu();
                                            }
                                        }
                                        if ui.button("♻ Drain").clicked() {
                                            let node_name = item.name.clone();
                                            tokio::spawn(async move {
                                                if let Err(err) = drain_node(&node_name).await {
                                                    eprintln!("Failed to drain node: {}", err);
                                                }
                                            });
                                            ui.close_menu();
                                        }
                                    });
                                    ui.end_row();
                                }
                            }
                        });
                    });
                },
                Category::Namespaces => {
                    fn phase_color(phase: &str) -> Color32 {
                        match phase {
                            "Active" => Color32::GREEN,
                            "Terminating" => Color32::RED,
                            _ => Color32::from_rgb(0x90, 0xA4, 0xAE),          // gray (default)
                        }
                    }

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
                    let ns = namespaces.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("namespace_scroll").show(ui, |ui| {
                        egui::Grid::new("namespace_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
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
                            for item in sorted_ns.iter() {
                                let cur_item_name = &item.name;
                                if filter_namespaces.is_empty() || cur_item_name.contains(&filter_namespaces) {
                                    if ui.colored_label(Color32::WHITE,&item.name).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        *selected_namespace_clone.lock().unwrap() = Some(item.name.clone());
                                    }
                                    if let Some(phase) = &item.phase {
                                        ui.colored_label(phase_color(phase), phase);
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
                                    let _ = ui.button("⚙");
                                    ui.end_row();
                                }
                            }
                        });
                    });
                },
                Category::Pods => {
                    // Get the list of available namespaces
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    ui.horizontal(|ui| {
                        ui.heading(format!("Pods - {}", pods.lock().unwrap().len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("default")).width(150.0).show_ui(ui, |ui| {
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
                    let pod = pods.lock().unwrap();
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
                            ui.label("Node");
                            ui.label("Actions");
                            ui.end_row();
                            let mut sorted_pods = pod.clone();
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
                                let running_on_node = &item.node_name.as_ref().unwrap();
                                if filter_pods.is_empty() || cur_item_name.contains(&filter_pods) || running_on_node.contains(&filter_pods) {
                                    ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                    let status;
                                    let mut ready_color: Color32;
                                    let cur_phase: &str;
                                    if item.pod_has_crashloop {
                                        cur_phase = "CrashLoopBackOff";
                                    } else {
                                        cur_phase = item.phase.as_ref().unwrap();
                                    }
                                    match cur_phase {
                                        "Running" => {
                                            status = "✅ Running".to_string();
                                            ready_color = Color32::from_rgb(100, 255, 100); // green
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
                                                ui.label(egui::RichText::new(state_str).color(match state_str {
                                                    "Running" => egui::Color32::GREEN,
                                                    "Waiting" => egui::Color32::YELLOW,
                                                    "Terminated" => egui::Color32::RED,
                                                    _ => egui::Color32::LIGHT_GRAY,
                                                }));
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
                                    ui.label(item.node_name.clone().unwrap_or("-".into()));
                                    ui.menu_button("⚙", |ui| {
                                        ui.set_width(200.0);
                                        if ui.button("🗑 Delete").clicked() {
                                            let cur_pod = item.name.clone();
                                            let cur_ns = selected_ns.clone();

                                            tokio::spawn(async move {
                                                if let Err(err) = delete_pod(&cur_pod.clone(), cur_ns.as_deref(), true).await {
                                                    eprintln!("Failed to delete pod: {}", err);
                                                }
                                            });
                                            ui.close_menu();
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
                                            ui.close_menu();
                                        }
                                    });
                                    ui.end_row();
                                }
                            }
                        });
                    });
                },
                Category::Deployments => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    ui.horizontal(|ui| {
                        ui.heading(format!("Deployments - {}", deployments.lock().unwrap().len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("default")).width(150.0).show_ui(ui, |ui| {
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
                    let deployments_list = deployments.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("deployments_scroll").show(ui, |ui| {
                        egui::Grid::new("deployments_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("NameSpace");
                            ui.label("Ready");
                            ui.label("Desired");
                            ui.label("Up-to-date");
                            ui.label("Available");
                            ui.label("Age");
                            ui.end_row();
                            for item in deployments_list.iter().rev().take(200) {
                                let cur_item_object = &item.name;
                                if filter_deployments.is_empty() || cur_item_object.contains(&filter_deployments) {
                                    ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                    ui.label(format!("{}", &item.namespace));
                                    ui.label(format!("{}/{}", &item.ready_replicas, &item.replicas));
                                    ui.label(format!("{}", &item.replicas));
                                    ui.label(format!("{}", &item.updated_replicas));
                                    ui.label(format!("{}", &item.available_replicas));
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                    ui.end_row();
                                }
                            }
                        });
                    });
                },
                Category::Secrets => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    ui.horizontal(|ui| {
                        ui.heading(format!("Secrets - {}", secrets.lock().unwrap().len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("default")).width(150.0).show_ui(ui, |ui| {
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.add(egui::TextEdit::singleline(&mut filter_secrets).hint_text("Filter secrets...").desired_width(200.0));
                        filter_secrets = filter_secrets.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_secrets.clear();
                        }
                    });
                    ui.separator();
                    let secrets_list = secrets.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("secrets_scroll").show(ui, |ui| {
                        egui::Grid::new("secrets_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Type");
                            ui.label("Age");
                            ui.label("Labels");
                            ui.label("Keys");
                            ui.end_row();
                            for item in secrets_list.iter().rev().take(200) {
                                let cur_item_object = &item.name;
                                if filter_secrets.is_empty() || cur_item_object.contains(&filter_secrets) {
                                    ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                    ui.label(format!("{}", &item.type_));
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                    ui.label(format!("{}", &item.labels));
                                    ui.label(format!("{}", &item.keys));
                                    ui.end_row();
                                }
                            }
                        });
                    });
                },
                Category::ConfigMaps => {
                    let ns = namespaces.lock().unwrap();
                    let mut selected_ns = selected_namespace_clone.lock().unwrap();
                    ui.horizontal(|ui| {
                        ui.heading(format!("ConfigMaps - {}", configmaps.lock().unwrap().len()));
                        ui.separator();
                        ui.heading(format!("Namespace - "));
                        egui::ComboBox::from_id_salt("namespace_combo").selected_text(selected_ns.as_deref().unwrap_or("default")).width(150.0).show_ui(ui, |ui| {
                            for item in ns.iter() {
                                let ns_name = &item.name;
                                ui.selectable_value(
                                    &mut *selected_ns,
                                    Some(ns_name.clone()),
                                    ns_name,
                                );
                            }
                        });
                        ui.add(egui::TextEdit::singleline(&mut filter_secrets).hint_text("Filter configmaps...").desired_width(200.0));
                        filter_secrets = filter_secrets.to_lowercase();
                        if ui.button(egui::RichText::new("ｘ").size(16.0).color(RED_BUTTON)).clicked() {
                            filter_secrets.clear();
                        }
                    });
                    ui.separator();
                    let configmaps_list = configmaps.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("configmaps_scroll").show(ui, |ui| {
                        egui::Grid::new("configmaps_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Type");
                            ui.label("Age");
                            ui.label("Labels");
                            ui.label("Keys");
                            ui.end_row();
                            for item in configmaps_list.iter().rev().take(200) {
                                let cur_item_object = &item.name;
                                if filter_secrets.is_empty() || cur_item_object.contains(&filter_secrets) {
                                    ui.label(egui::RichText::new(&item.name).color(egui::Color32::WHITE));
                                    ui.label(format!("{}", &item.type_));
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                    ui.label(format!("{:?}", &item.labels));
                                    ui.label(format!("{}", &item.keys.join(", ")));
                                    ui.end_row();
                                }
                            }
                        });
                    });
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
                    egui::ScrollArea::vertical().id_salt("events_scroll").show(ui, |ui| {
                        egui::Grid::new("events_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Time");
                            ui.label("Type");
                            ui.label("Age");
                            ui.label("Namespace");
                            ui.label("Reason");
                            ui.label("Object");
                            ui.label("Message");
                            ui.end_row();
                            for item in events_list.iter().rev().take(200) {
                                let cur_item_object = &item.involved_object;
                                if filter_events.is_empty() || cur_item_object.contains(&filter_events) {
                                    ui.label(&item.timestamp);
                                    ui.label(
                                        egui::RichText::new(&item.event_type).color(match item.event_type.as_str() {
                                            "Warning" => egui::Color32::ORANGE,
                                            "Normal" => egui::Color32::GREEN,
                                            _ => egui::Color32::LIGHT_GRAY,
                                        }),
                                    );
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                    ui.label(&item.namespace);
                                    ui.label(&item.reason);
                                    ui.label(&item.involved_object);
                                    ui.label(&item.message);
                                    ui.end_row();
                                }
                            }
                        });
                    });
                },
            }
        });

        // New resource creation window
        if new_resource_window.show {
            egui::Window::new("Create New Resource").collapsible(false).resizable(true).show(ctx, |ui| {
                if new_resource_window.content.is_empty() {
                    new_resource_window.content = match new_resource_window.resource_type {
                        ResourceType::NameSpace => NAMESPACE_TEMPLATE.to_string(),
                        ResourceType::Pod => POD_TEMPLATE.to_string(),
                        ResourceType::PersistenceVolumeClaim => PVC_TEMPLATE.to_string(),
                        ResourceType::Blank => "".to_string(),
                    };
                }

                ui.horizontal(|ui| {
                    ui.label("YAML Template:");
                    egui::ComboBox::from_id_salt("templates_combo").width(150.0)
                        .selected_text(match new_resource_window.resource_type {
                            ResourceType::NameSpace => "NameSpace",
                            ResourceType::Pod => "Pod",
                            ResourceType::PersistenceVolumeClaim => "PersistenceVolumeClaim",
                            ResourceType::Blank => "Blank",
                        }).show_ui(ui, |ui| {
                            if ui.selectable_value(&mut new_resource_window.resource_type, ResourceType::NameSpace, "NameSpace",).clicked() {
                                new_resource_window.content = NAMESPACE_TEMPLATE.to_string();
                            };
                            if ui.selectable_value(&mut new_resource_window.resource_type, ResourceType::Pod, "Pod",).clicked() {
                                new_resource_window.content = POD_TEMPLATE.to_string();
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
                        tokio::spawn(async move {
                            if let Err(e) = apply_yaml(&yaml).await {
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

        ctx.request_repaint();
    })
    .unwrap();
}
