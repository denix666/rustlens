use std::sync::{Arc, Mutex};
use egui::{Color32, Context};

use crate::functions::item_color;


pub struct PodDetailsWindow {
    pub show: bool,
}

impl PodDetailsWindow {
    pub fn new() -> Self {
        Self {
            show: false,
        }
    }
}

// Define items colors
const DETAIL_COLOR: Color32 = Color32::LIGHT_YELLOW;
const SECOND_DETAIL_COLOR: Color32 = Color32::LIGHT_BLUE;
const ROW_NAME_COLOR: Color32 = Color32::WHITE;

pub fn show_pod_details_window(
        ctx: &Context,
        pod_details_window: &mut PodDetailsWindow,
        details: Arc<Mutex<crate::PodDetails>>,
        pods: Arc<Mutex<Vec<crate::PodItem>>>)
{
    let guard_details = details.lock().unwrap(); // More detailed info
    let guard_pods = pods.lock().unwrap(); // Pods with base details already we have

    if guard_details.name.is_none() {
        return;
    }

    let pod_item = guard_pods.iter().find(|item| item.name == guard_details.name.clone().unwrap());

    egui::Window::new("Pod details").min_width(800.0).collapsible(false).resizable(true).show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
            egui::Grid::new("pod_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                ui.label(egui::RichText::new("Pod name:").color(ROW_NAME_COLOR));
                ui.label(egui::RichText::new(guard_details.name.clone().unwrap()).color(DETAIL_COLOR));
                ui.end_row();

                if let Some(item) = pod_item {
                    if let Some(creation_timestamp) = &item.creation_timestamp {
                        ui.label(egui::RichText::new("Creation time:").color(ROW_NAME_COLOR));
                        let timestamp_text = format!("{}, {} ago", creation_timestamp.0, crate::format_age(creation_timestamp));
                        ui.label(egui::RichText::new(timestamp_text).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if let Some(namespace) = &item.namespace {
                        ui.label(egui::RichText::new("Namespace:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(namespace).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if let Some(node_name) = &item.node_name {
                        ui.label(egui::RichText::new("Running on:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(node_name).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if let Some(phase) = &item.phase {
                        ui.label(egui::RichText::new("Status:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(phase).color(item_color(phase)));
                        ui.end_row();
                    }

                    if let Some(qos_class) = &item.qos_class {
                        ui.label(egui::RichText::new("QoS:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(qos_class).color(item_color(qos_class)));
                        ui.end_row();
                    }
                }

                if let Some(i) = guard_details.uid.clone() {
                    ui.label(egui::RichText::new("UID:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.service_account.clone() {
                    ui.label(egui::RichText::new("Service account:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.pod_ip.clone() {
                    ui.label(egui::RichText::new("Pod IP:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.host_ip.clone() {
                    ui.label(egui::RichText::new("Host IP:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.labels.clone() {
                    ui.label(egui::RichText::new("Labels:").color(ROW_NAME_COLOR));
                    egui::Grid::new("pod_details_labels_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
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
                    egui::Grid::new("pod_details_annotations_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for (j, y) in i.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            });
        });
        ui.separator();
        if ui.button(egui::RichText::new("🗙 Close").size(16.0).color(egui::Color32::WHITE)).clicked() {
            pod_details_window.show = false;
        }
    });
}
