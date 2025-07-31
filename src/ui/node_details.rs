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

pub fn show_node_details_window(ctx: &Context, node_details_window: &mut NodeDetailsWindow, details: Arc<Mutex<crate::NodeDetails>>) {
    let guard = details.lock().unwrap();

    egui::Window::new("Node details").min_width(800.0).collapsible(false).resizable(true).show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height(800.0).show(ui, |ui| {
            egui::Grid::new("node_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                if let Some(i) = guard.name.clone() {
                    ui.label(egui::RichText::new("Node name:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard.os.clone() {
                    ui.label(egui::RichText::new("OS:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard.os_image.clone() {
                    ui.label(egui::RichText::new("OS Image:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard.kernel_version.clone() {
                    ui.label(egui::RichText::new("Kernel version:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard.container_runtime.clone() {
                    ui.label(egui::RichText::new("Container runtime:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard.kubelet_version.clone() {
                    ui.label(egui::RichText::new("Kubelet version:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if guard.addresses.clone().len() > 0 {
                    ui.label(egui::RichText::new("Adresses:").color(ROW_NAME_COLOR));
                    egui::Grid::new("node_details_addresses_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for (j, y) in guard.addresses.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(i) = guard.labels.clone() {
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

                if let Some(i) = guard.annotations.clone() {
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

                if guard.taints.clone().len() > 0 {
                    ui.label(egui::RichText::new("Taints:").color(ROW_NAME_COLOR));
                    egui::Grid::new("node_details_taints_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for i in guard.taints.iter() {
                            ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            });

                // if guard.pods.clone().len() > 0 {
                //     ui.label("Pods:");
                //     for j in guard.pods.clone().iter() {
                //         let label_text = format!("{} - {} - {}", &j.name, &j.namespace, &j.status);
                //         ui.label(label_text);
                //     }
                //     ui.separator();
                // }

        });

        ui.separator();
        if ui.button(egui::RichText::new("🗙 Close").size(16.0).color(egui::Color32::WHITE)).clicked() {
            node_details_window.show = false;
        }
    });
}
