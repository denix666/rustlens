use eframe::egui::CursorIcon;
use eframe::*;
use egui::{Context, Style, TextStyle, FontId};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

mod functions;
use functions::*;

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");

#[derive(PartialEq)]
enum SortBy {
    Name,
    Age,
}

#[derive(PartialEq, Clone)]
enum Category {
    ClusterOverview,
    Nodes,
    Namespaces,
    Pods,
    Deployments,
    Events,
}

#[derive(Clone)]
struct EventItem {
    message: String,
    reason: String,
    involved_object: String,
    event_type: String,
    timestamp: String,
    namespace: String,
    creation_timestamp: Option<Time>,
}

#[derive(Clone)]
struct NodeItem {
    name: String,
    status: String, // "Ready", "NotReady", "Unknown"
    roles: Vec<String>,
    scheduling_disabled: bool,
    taints: Option<Vec<k8s_openapi::api::core::v1::Taint>>,
    cpu_percent: f32,
    mem_percent: f32,
    storage: Option<String>,
    creation_timestamp: Option<Time>,
}

#[derive(Clone)]
struct NamespaceItem {
    name: String,
    creation_timestamp: Option<Time>,
}

#[derive(Clone)]
struct ClusterInfo {
    name: String,
}


#[derive(Clone)]
struct PodItem {
    name: String,
    creation_timestamp: Option<Time>,
}

#[tokio::main]
async fn main() {
    let mut title = String::from("RustLens v");
    title.push_str(env!("CARGO_PKG_VERSION"));
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 600.0])
            .with_maximize_button(false),
        ..Default::default()
    };

    if let Ok(icon) = load_embedded_icon() {
        options.viewport = options.viewport.with_icon(icon);
    }

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

    let events = Arc::new(Mutex::new(Vec::<EventItem>::new()));
    let events_clone = Arc::clone(&events);
    tokio::spawn(async move {
        watch_events(events_clone).await;
    });

    let nodes = Arc::new(Mutex::new(Vec::<NodeItem>::new()));
    let node_clone = Arc::clone(&nodes);
    tokio::spawn(async move {
        watch_nodes(node_clone).await;
    });

    let namespaces = Arc::new(Mutex::new(Vec::<NamespaceItem>::new()));
    let ns_clone = Arc::clone(&namespaces);
    tokio::spawn(async move {
        watch_namespaces(ns_clone).await;
    });

    let pods = Arc::new(Mutex::new(Vec::<PodItem>::new()));
    let pod_watcher_ns = Arc::clone(&selected_namespace);
    let pod_watcher_list = Arc::clone(&pods);
    tokio::spawn(async move {
        let mut last_ns = String::new();

        loop {
            // get current namespace or "default"
            let ns = pod_watcher_ns
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "default".to_string());

            if ns != last_ns {
                // clear old pods
                pod_watcher_list.lock().unwrap().clear();

                // run new watcher
                let pod_list_clone = Arc::clone(&pod_watcher_list);
                let ns_clone = ns.clone();

                tokio::spawn(async move {
                    watch_pods(pod_list_clone, ns_clone).await;
                });

                last_ns = ns;
            }

            sleep(Duration::from_secs(1)).await;
        }
    });

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
            });

            egui::CollapsingHeader::new("🛠 Config").default_open(true).show(ui, |ui| {
                ui.label("🗺 ConfigMaps");
                ui.label("🕵 Secrets");
            });

            egui::CollapsingHeader::new("🖧 Network").default_open(true).show(ui, |ui| {
                ui.label("💢 Services");
                ui.label("⛺ Endpoints");
                ui.label("⤵ Ingresses");
            });

            egui::CollapsingHeader::new("🖴 Storage").default_open(true).show(ui, |ui| {
                ui.label("📃 PersistentVolumeClaims");
                ui.label("🗄 PersistentVolumes");
                ui.label("⛭ StorageClasses");
            });

            egui::CollapsingHeader::new("⎈ Helm").default_open(true).show(ui, |ui| {
                ui.label("📰 Charts");
                ui.label("📥 Releases");
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
                        // if ui.button("🔄 Reconnect").clicked() {
                        //     let cluster_info_clone = Arc::clone(&cluster_info_ui);
                        //     tokio::spawn(async move {
                        //         if let Ok(name) = get_cluster_name().await {
                        //             *cluster_info_clone.lock().unwrap() = ClusterInfo { name };
                        //         }
                        //     });
                        // }
                    });
                },
                Category::Nodes => {
                    ui.horizontal(|ui| {
                        ui.heading(format!("Nodes - {}    ", nodes.lock().unwrap().len()));
                        ui.add(egui::TextEdit::singleline(&mut filter_nodes).hint_text("Filter nodes...").desired_width(200.0));
                        filter_nodes = filter_nodes.to_lowercase();
                        if ui.button("ｘ").clicked() {
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
                    ui.horizontal(|ui| {
                        ui.heading(format!("Namespaces - {}   ", namespaces.lock().unwrap().len()));
                        ui.add(egui::TextEdit::singleline(&mut filter_namespaces).hint_text("Filter namespaces...").desired_width(200.0));
                        filter_namespaces = filter_namespaces.to_lowercase();
                        if ui.button("ｘ").clicked() {
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
                                        let at = a.creation_timestamp.as_ref();
                                        let bt = b.creation_timestamp.as_ref();
                                        at.cmp(&bt)
                                    }
                                };
                                if sort_asc { ord } else { ord.reverse() }
                            });
                            for item in sorted_ns.iter() {
                                let cur_item_name = &item.name;
                                if filter_namespaces.is_empty() || cur_item_name.contains(&filter_namespaces) {
                                    if ui.label(&item.name).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        *selected_namespace_clone.lock().unwrap() = Some(item.name.clone());
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
                        ui.heading(format!("Pods - {}     |     Namespace - ", pods.lock().unwrap().len()));
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
                        ui.add(egui::TextEdit::singleline(&mut filter_pods).hint_text("Filter pods...").desired_width(200.0));
                        filter_pods = filter_pods.to_lowercase();
                        if ui.button("ｘ").clicked() {
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
                                if filter_pods.is_empty() || cur_item_name.contains(&filter_pods) {
                                    let pod_name = item.name.clone();
                                    ui.label(&item.name);
                                    ui.label(format_age(&item.creation_timestamp.as_ref().unwrap()));
                                    ui.menu_button("⚙", |ui| {
                                        ui.set_width(200.0);
                                        if ui.button("🗑 Delete").clicked() {
                                            let cur_ns = selected_ns.clone();
                                            tokio::spawn(async move {
                                                if let Err(err) = delete_pod(&pod_name, cur_ns.as_deref(), true).await {
                                                    eprintln!("Failed to delete pod: {}", err);
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
                Category::Deployments => {
                    ui.heading("Deployments (TODO)");
                },
                Category::Events => {
                    ui.horizontal(|ui| {
                        ui.heading(format!("Events - {}", events.lock().unwrap().len()));
                        ui.add(egui::TextEdit::singleline(&mut filter_events).hint_text("Filter events...").desired_width(200.0));
                        filter_events = filter_events.to_lowercase();
                        if ui.button("ｘ").clicked() {
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

        ctx.request_repaint();
    })
    .unwrap();
}
