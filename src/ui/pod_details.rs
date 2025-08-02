use std::sync::{Arc, Mutex};
use egui::{Color32, Context, Ui};
//use k8s_openapi::api::core::v1::{NodeAffinity, PodAffinity};
use crate::functions::item_color;
use k8s_openapi::api::core::v1::{
    NodeAffinity,
    PodAffinity,
    PodAntiAffinity,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{
    LabelSelectorRequirement,
};

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
const TOLERATIONS_HEAD_GRID_COLOR: Color32 = Color32::GRAY;
const TOLERATION_NAME_COLUMN_COLOR: Color32 = Color32::MAGENTA;

// --- Вспомогательные функции ---

/// Renders a grid for NodeSelectorRequirement (used in Node Affinity).
fn render_node_selector_requirements(ui: &mut Ui, exprs: &[k8s_openapi::api::core::v1::NodeSelectorRequirement], grid_id: &str) {
    if !exprs.is_empty() {
        egui::Grid::new(grid_id).show(ui, |ui| {
            for expr in exprs {
                let key = &expr.key;
                let operator = &expr.operator;
                let values = expr.values.as_ref()
                    .map(|v| v.join(", "))
                    .unwrap_or_else(|| "None".to_string());

                ui.label(key);
                ui.label(format!("{operator} [{values}]"));
                ui.end_row();
            }
        });
    }
}

/// Renders a grid for LabelSelectorRequirement (used in Pod Affinity).
fn render_label_selector_requirements(ui: &mut Ui, exprs: &[LabelSelectorRequirement], grid_id: &str) {
    if !exprs.is_empty() {
        egui::Grid::new(grid_id).show(ui, |ui| {
            for expr in exprs {
                let key = &expr.key;
                let operator = &expr.operator;
                let values = expr.values.as_ref()
                    .map(|v| v.join(", "))
                    .unwrap_or_else(|| "None".to_string());

                ui.label(key);
                ui.label(format!("{operator} [{values}]"));
                ui.end_row();
            }
        });
    }
}

/// Renders the UI for Node Affinity.
fn render_node_affinity(ui: &mut Ui, node_affinity: &NodeAffinity) {
    egui::CollapsingHeader::new("Node Affinity")
        .default_open(false)
        .show(ui, |ui| {
            // RequiredDuringScheduling
            if let Some(required) = &node_affinity.required_during_scheduling_ignored_during_execution {
                ui.label(egui::RichText::new("RequiredDuringScheduling:").strong());
                for (i, term) in required.node_selector_terms.iter().enumerate() {
                    if let Some(exprs) = &term.match_expressions {
                        let grid_id = format!("required_node_affinity_grid_{}", i);
                        render_node_selector_requirements(ui, exprs, &grid_id);
                    }
                }
            }

            // PreferredDuringScheduling
            if let Some(preferred) = &node_affinity.preferred_during_scheduling_ignored_during_execution {
                ui.label(egui::RichText::new("PreferredDuringScheduling:").strong());
                for (i, pref) in preferred.iter().enumerate() {
                    ui.label(format!("Weight: {}", pref.weight));
                    if let Some(exprs) = &pref.preference.match_expressions {
                        let grid_id = format!("preferred_node_affinity_grid_{}", i);
                        render_node_selector_requirements(ui, exprs, &grid_id);
                    }
                }
            }
        });
}

/// Renders the UI for Pod Affinity.
fn render_pod_affinity(ui: &mut Ui, pod_affinity: &PodAffinity) {
    egui::CollapsingHeader::new("Pod Affinity")
        .default_open(false)
        .show(ui, |ui| {
            if let Some(required) = &pod_affinity.required_during_scheduling_ignored_during_execution {
                ui.label(egui::RichText::new("RequiredDuringScheduling:").strong());
                for (i, term) in required.iter().enumerate() {
                    ui.label(format!("TopologyKey: {}", term.topology_key));
                    if let Some(selector) = &term.label_selector {
                        let grid_id = format!("required_pod_affinity_grid_{}", i);
                        // Используем правильную функцию для LabelSelectorRequirement
                        if let Some(exprs) = &selector.match_expressions {
                            render_label_selector_requirements(ui, exprs, &grid_id);
                        }
                    }
                }
            }
        });
}

/// Renders the UI for Pod Anti-Affinity.
fn render_pod_anti_affinity(ui: &mut Ui, pod_anti_affinity: &PodAntiAffinity) {
    egui::CollapsingHeader::new("Pod Anti-Affinity")
        .default_open(false)
        .show(ui, |ui| {
            if let Some(required) = &pod_anti_affinity.required_during_scheduling_ignored_during_execution {
                ui.label(egui::RichText::new("RequiredDuringScheduling:").strong());
                for (i, term) in required.iter().enumerate() {
                    ui.label(format!("TopologyKey: {}", term.topology_key));
                    if let Some(selector) = &term.label_selector {
                        let grid_id = format!("required_pod_anti_affinity_grid_{}", i);
                        // Используем правильную функцию для LabelSelectorRequirement
                        if let Some(exprs) = &selector.match_expressions {
                            render_label_selector_requirements(ui, exprs, &grid_id);
                        }
                    }
                }
            }
        });
}


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

                    if let Some(controller) = &item.controller {
                        ui.label(egui::RichText::new("Controlled by:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(controller).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    ui.label(egui::RichText::new("Restarts count:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(&item.restart_count.to_string()).color(DETAIL_COLOR));
                    ui.end_row();
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
                    ui.separator(); ui.separator(); ui.end_row();
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
                    ui.separator(); ui.separator(); ui.end_row();
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

                if guard_details.tolerations.len() > 0 {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Tolerations:").color(ROW_NAME_COLOR));
                    egui::Grid::new("pod_details_tolerations_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        ui.label(egui::RichText::new("Key").color(TOLERATIONS_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Operator").color(TOLERATIONS_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Value").color(TOLERATIONS_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Effect").color(TOLERATIONS_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Seconds").color(TOLERATIONS_HEAD_GRID_COLOR));
                        ui.end_row();
                        for j in guard_details.tolerations.iter() {
                            if let Some(item) = &j.key {
                                ui.label(egui::RichText::new(item).color(TOLERATION_NAME_COLUMN_COLOR));
                            } else {
                                ui.label("");
                            }
                            if let Some(item) = &j.operator {
                                ui.label(egui::RichText::new(item).color(TOLERATION_NAME_COLUMN_COLOR));
                            } else {
                                ui.label("");
                            }
                            if let Some(item) = &j.value {
                                ui.label(egui::RichText::new(item).color(TOLERATION_NAME_COLUMN_COLOR));
                            } else {
                                ui.label("");
                            }
                            if let Some(item) = &j.effect {
                                ui.label(egui::RichText::new(item).color(TOLERATION_NAME_COLUMN_COLOR));
                            } else {
                                ui.label("");
                            }
                            if let Some(item) = &j.toleration_seconds {
                                ui.label(egui::RichText::new(item.to_string()).color(TOLERATION_NAME_COLUMN_COLOR));
                            } else {
                                ui.label("");
                            }
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(affinity) = &guard_details.affinity {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Affinity").heading());

                    if let Some(node_affinity) = &affinity.node_affinity {
                        render_node_affinity(ui, node_affinity);
                    }

                    if let Some(pod_affinity) = &affinity.pod_affinity {
                        render_pod_affinity(ui, pod_affinity);
                    }

                    if let Some(pod_anti_affinity) = &affinity.pod_anti_affinity {
                        render_pod_anti_affinity(ui, pod_anti_affinity);
                    }
                }


                if let Some(item) = guard_details.node_selector.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Node selector:").color(ROW_NAME_COLOR));
                    egui::Grid::new("pod_details_node_selector_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for (j, y) in item.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if !guard_details.conditions.is_empty() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Conditions:").color(ROW_NAME_COLOR));
                    egui::Grid::new("pod_details_conditions_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for item in guard_details.conditions.iter() {
                            let cond_type = item.type_.clone();
                            let status = item.status.clone();
                            let reason = item.reason.clone().unwrap_or_default();
                            let message = item.message.clone().unwrap_or_default();

                            ui.label(egui::RichText::new(&cond_type).color(DETAIL_COLOR));

                            if !reason.is_empty() || !message.is_empty() {
                                let lbl_text = format!("{status} ({reason} - {message})");
                                ui.label(egui::RichText::new(lbl_text).color(DETAIL_COLOR));
                            } else {
                                ui.label(egui::RichText::new(status).color(DETAIL_COLOR));
                            }

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
