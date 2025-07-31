use std::sync::{Arc, Mutex};

use egui::{Color32, Context};

const DETAIL_COLOR: Color32 = Color32::LIGHT_YELLOW;
const SECOND_DETAIL_COLOR: Color32 = Color32::LIGHT_BLUE;
const ROW_NAME_COLOR: Color32 = Color32::WHITE;

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

pub fn show_node_details_window(ctx: &Context, node_details_window: &mut NodeDetailsWindow, details: Arc<Mutex<crate::NodeDetails>>, nodes: Arc<Mutex<Vec<crate::NodeItem>>>, pods: Arc<Mutex<Vec<crate::PodItem>>>) {
    let guard_details = details.lock().unwrap(); // More detailed info
    let guard_nodes = nodes.lock().unwrap(); // Nodes with base details already we have
    let guard_pods = pods.lock().unwrap(); // Pods with base details already we have

    if guard_details.name.is_none() {
        return;
    }

    let node_item = guard_nodes.iter().find(|item| item.name == guard_details.name.clone().unwrap());

    let pods: Vec<_> = guard_pods.iter().filter(|pod| pod.node_name.as_deref() == Some(guard_details.name.clone().unwrap().as_str())).cloned().collect();

    egui::Window::new("Node details").min_width(800.0).collapsible(false).resizable(true).show(ctx, |ui| {
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
                        ui.label(egui::RichText::new(creation_timestamp.0.to_string()).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if let Some(storage_total) = &item.storage_total {
                        ui.label(egui::RichText::new("Storage total:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(storage_total.to_string()).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if let Some(mem_total) = &item.mem_total {
                        ui.label(egui::RichText::new("Memory total:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(mem_total.to_string()).color(DETAIL_COLOR));
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
                    egui::Grid::new("node_details_annotations_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
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
                        ui.label("Pod name");
                        ui.label("Namespace");
                        ui.label("Status");
                        ui.end_row();
                        for j in pods.iter() {
                            ui.label(&j.name.to_string());
                            ui.label(&j.namespace.as_ref().unwrap().to_string());
                            ui.label(&j.phase.as_ref().unwrap().to_string());
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            });
        });

        ui.separator();
        if ui.button(egui::RichText::new("🗙 Close").size(16.0).color(egui::Color32::WHITE)).clicked() {
            node_details_window.show = false;
        }
    });
}
