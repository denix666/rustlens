use std::sync::{Arc, Mutex};
use egui::{Context, CursorIcon, Key};
use kube::Client;
use crate::{functions::item_color, get_details::get_pod_details, theme::*, ui::PodDetailsWindow};

pub struct NodeDetailsWindow {
    pub show: bool,
}

impl NodeDetailsWindow {
    pub fn new() -> Self {
        Self {
            show: false,
        }
    }
}

pub fn show_node_details_window(
        ctx: &Context,
        node_details_window: &mut NodeDetailsWindow,
        details: Arc<Mutex<crate::NodeDetails>>,
        nodes: Arc<Mutex<Vec<crate::NodeItem>>>,
        pods: Arc<Mutex<Vec<crate::PodItem>>>,
        pod_details_window: &mut PodDetailsWindow,
        pod_details: Arc<Mutex<crate::PodDetails>>,
        client: Arc<Client>
){
    let guard_details = details.lock().unwrap(); // More detailed info
    let guard_nodes = nodes.lock().unwrap(); // Nodes with base details already we have
    let guard_pods = pods.lock().unwrap(); // Pods with base details already we have

    if guard_details.name.is_none() {
        return;
    }

    let node_item = guard_nodes.iter().find(|item| item.name == guard_details.name.clone().unwrap());

    let pods: Vec<_> = guard_pods.iter().filter(|pod| pod.node_name.as_deref() == Some(guard_details.name.clone().unwrap().as_str())).cloned().collect();

    let response = egui::Window::new("Node details").min_width(800.0).collapsible(false).resizable(true).open(&mut node_details_window.show).show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
            egui::Grid::new("node_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                ui.label(egui::RichText::new("Node name:").color(ROW_NAME_COLOR));
                ui.label(egui::RichText::new(guard_details.name.clone().unwrap()).color(DETAIL_COLOR));
                ui.end_row();

                if let Some(item) = node_item {
                    ui.label(egui::RichText::new("Status:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(&item.status).color(DETAIL_COLOR));
                    ui.end_row();

                    if let Some(creation_timestamp) = &item.creation_timestamp {
                        ui.label(egui::RichText::new("Creation time:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(format!("{}, {} ago", creation_timestamp.0.to_string(), crate::format_age(&creation_timestamp))).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if let Some(storage_total) = &item.storage_total {
                        ui.label(egui::RichText::new("Storage total:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(format!("{}Gb", storage_total.round() as i64)).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if let Some(mem_total) = &item.mem_total {
                        ui.label(egui::RichText::new("Memory total:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(format!("{}Gb", mem_total.round() as i64)).color(DETAIL_COLOR));
                        ui.end_row();
                    }
                }

                if let Some(i) = guard_details.os.clone() {
                    ui.label(egui::RichText::new("OS Type:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.os_image.clone() {
                    ui.label(egui::RichText::new("OS Image:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.kernel_version.clone() {
                    ui.label(egui::RichText::new("Kernel version:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.container_runtime.clone() {
                    ui.label(egui::RichText::new("Container runtime:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.kubelet_version.clone() {
                    ui.label(egui::RichText::new("Kubelet version:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if guard_details.addresses.clone().len() > 0 {
                    ui.label(egui::RichText::new("Adresses:").color(ROW_NAME_COLOR));
                    egui::Grid::new("node_details_addresses_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for (j, y) in guard_details.addresses.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(i) = guard_details.labels.clone() {
                    ui.label(egui::RichText::new("Labels:").color(ROW_NAME_COLOR));
                    egui::Grid::new("node_details_labels_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for (j, y) in i.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(i) = guard_details.annotations.clone() {
                    ui.label(egui::RichText::new("Annotations:").color(ROW_NAME_COLOR));
                    egui::Grid::new("node_details_annotations_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                        for (j, y) in i.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if guard_details.taints.clone().len() > 0 {
                    ui.label(egui::RichText::new("Taints:").color(ROW_NAME_COLOR));
                    egui::Grid::new("node_details_taints_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for i in guard_details.taints.iter() {
                            ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if pods.len() > 0 {
                    ui.label(egui::RichText::new("Pods:").color(ROW_NAME_COLOR));
                    egui::Grid::new("pods_on_node_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        ui.label(egui::RichText::new("Pod name").color(PODS_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Namespace").color(PODS_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Status").color(PODS_HEAD_GRID_COLOR));
                        ui.end_row();
                        for j in pods.iter() {
                            if ui.label(egui::RichText::new(&j.name).color(POD_NAME_COLUMN_COLOR)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                let ns = j.namespace.clone();
                                let name = j.name.clone();
                                let client_clone = Arc::clone(&client);
                                let details = Arc::clone(&pod_details);
                                pod_details_window.show = true;
                                tokio::spawn({
                                    async move {
                                        if let Err(e) = get_pod_details(client_clone, &name, ns, details).await {
                                            eprintln!("Details fetch failed: {:?}", e);
                                        }
                                    }
                                });
                            }
                            ui.label(egui::RichText::new(&j.namespace.as_ref().unwrap().to_string()).color(NAMESPACE_COLUMN_COLOR));
                            ui.label(egui::RichText::new(&j.phase.as_ref().unwrap().to_string()).color(item_color(&j.phase.as_ref().unwrap().to_string())));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            });
        });
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            node_details_window.show = false;
        }
    }
}
