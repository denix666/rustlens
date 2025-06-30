use eframe::egui::CursorIcon;
use eframe::*;
use egui::{Context, Style, TextStyle, FontId};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

mod functions;
use functions::*;

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");

#[derive(PartialEq, Clone)]
enum Category {
    Cluster,
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
}

#[derive(Clone)]
struct NamespaceItem {
    name: String,
}

#[derive(Clone)]
struct PodItem {
    name: String,
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

    let nodes = Arc::new(Mutex::new(Vec::<NodeItem>::new()));
    let node_clone = Arc::clone(&nodes);

    let pods = Arc::new(Mutex::new(Vec::<PodItem>::new()));

    let namespaces = Arc::new(Mutex::new(Vec::<NamespaceItem>::new()));
    let ns_clone = Arc::clone(&namespaces);

    let selected_category = Arc::new(Mutex::new(Category::Cluster));
    let selected_category_ui = Arc::clone(&selected_category);

    let selected_namespace = Arc::new(Mutex::new(None::<String>));
    let selected_namespace_clone = Arc::clone(&selected_namespace);

    let pod_watcher_ns = Arc::clone(&selected_namespace);
    let pod_watcher_list = Arc::clone(&pods);

    let events = Arc::new(Mutex::new(Vec::<EventItem>::new()));
    let events_clone = Arc::clone(&events);

    tokio::spawn(async move {
        watch_events(events_clone).await;
    });

    tokio::spawn(async move {
        watch_nodes(node_clone).await;
    });

    tokio::spawn(async move {
        watch_namespaces(ns_clone).await;
    });

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

            if ui.selectable_label(*selected_category_ui.lock().unwrap() == Category::Cluster,"☸ Cluster",).clicked() {
                *selected_category_ui.lock().unwrap() = Category::Cluster;
            }

            if ui.selectable_label(*selected_category_ui.lock().unwrap() == Category::Nodes,"💻 Nodes",).clicked() {
                *selected_category_ui.lock().unwrap() = Category::Nodes;
            }

            if ui.selectable_label(*selected_category_ui.lock().unwrap() == Category::Namespaces,"☰ Namespaces",).clicked() {
                *selected_category_ui.lock().unwrap() = Category::Namespaces;
            }

            egui::CollapsingHeader::new("📦 Workloads").default_open(false).show(ui, |ui| {
                if ui.selectable_label(current == Category::Pods, "📚 Pods").clicked() {
                    *selected_category_ui.lock().unwrap() = Category::Pods;
                }

                if ui.selectable_label(current == Category::Deployments, "📃 Deployments").clicked() {
                    *selected_category_ui.lock().unwrap() = Category::Deployments;
                }
            });

            egui::CollapsingHeader::new("🛠 Config").default_open(false).show(ui, |ui| {
                ui.label("🗺 ConfigMaps");
                ui.label("🕵 Secrets");
            });

            egui::CollapsingHeader::new("🖧 Network").default_open(false).show(ui, |ui| {
                ui.label("💢 Services");
                ui.label("⛺ Endpoints");
                ui.label("⤵ Ingresses");
            });

            egui::CollapsingHeader::new("🖴 Storage").default_open(false).show(ui, |ui| {
                ui.label("📃 PersistentVolumeClaims");
                ui.label("🗄 PersistentVolumes");
                ui.label("⛭ StorageClasses");
            });

            egui::CollapsingHeader::new("⎈ Helm").default_open(false).show(ui, |ui| {
                ui.label("📰 Charts");
                ui.label("📥 Releases");
            });

            if ui.selectable_label(*selected_category_ui.lock().unwrap() == Category::Events,"🕓 Events",).clicked() {
                *selected_category_ui.lock().unwrap() = Category::Events;
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match *selected_category_ui.lock().unwrap() {
                Category::Cluster => {
                    ui.heading("Cluster (TODO)");
                },
                Category::Nodes => {
                    ui.heading(format!("Nodes - {}", nodes.lock().unwrap().len()));
                    ui.separator();

                    let nodes = nodes.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("node_scroll").hscroll(true).show(ui, |ui| {
                        egui::Grid::new("node_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("CPU");
                            ui.label("Memory");
                            ui.label("Storage");
                            ui.label("Taints");
                            ui.label("Role");
                            ui.label("Status");
                            ui.label("");
                            ui.label("Actions");
                            ui.end_row();
                            for item in nodes.iter() {
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
                        });
                    });
                },
                Category::Namespaces => {
                    ui.heading(format!("Namespaces - {}", namespaces.lock().unwrap().len()));
                    ui.separator();

                    let ns = namespaces.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("namespace_scroll").show(ui, |ui| {
                        egui::Grid::new("namespace_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Actions");
                            ui.end_row();
                            for item in ns.iter() {
                                if ui.label(&item.name).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    *selected_namespace_clone.lock().unwrap() = Some(item.name.clone());
                                }
                                let _ = ui.button("⚙");
                                ui.end_row();
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
                    });

                    ui.separator();
                    let pod = pods.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("pods_scroll").show(ui, |ui| {
                        egui::Grid::new("pods_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Actions");
                            ui.end_row();
                            for item in pod.iter() {
                                ui.label(&item.name);
                                let _ = ui.button("⚙");
                                ui.end_row();
                            }
                        });
                    });
                },
                Category::Deployments => {
                    ui.heading("Deployments (TODO)");
                },
                Category::Events => {
                    ui.heading(format!("Events - {}", events.lock().unwrap().len()));
                    ui.separator();

                    let events_list = events.lock().unwrap();
                    egui::ScrollArea::vertical().id_salt("events_scroll").show(ui, |ui| {
                        egui::Grid::new("events_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                            ui.label("Time");
                            ui.label("Namespace");
                            ui.label("Type");
                            ui.label("Reason");
                            ui.label("Object");
                            ui.label("Message");
                            ui.end_row();
                            for item in events_list.iter().rev().take(200) {
                                ui.label(&item.timestamp);
                                ui.label(
                                    egui::RichText::new(&item.event_type).color(match item.event_type.as_str() {
                                        "Warning" => egui::Color32::ORANGE,
                                        "Normal" => egui::Color32::GREEN,
                                        _ => egui::Color32::LIGHT_GRAY,
                                    }),
                                );
                                ui.label(&item.namespace);
                                ui.label(&item.reason);
                                ui.label(&item.involved_object);
                                ui.label(&item.message);
                                ui.end_row();
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
