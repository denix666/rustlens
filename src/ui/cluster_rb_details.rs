use std::sync::{Arc, Mutex};
use egui::{Context, Key};
use crate::functions::item_color;
use crate::theme::*;
use crate::ui::YamlEditorWindow;

pub struct ClusterRoleBindingDetailsWindow {
    pub show: bool,
}

impl ClusterRoleBindingDetailsWindow {
    pub fn new() -> Self {
        Self {
            show: false,
        }
    }
}

pub fn show_cluster_rb_details_window(
        ctx: &Context,
        cluster_rb_details_window: &mut ClusterRoleBindingDetailsWindow,
        details: Arc<Mutex<crate::ClusterRoleBindingDetails>>,
        cluster_rbs: Arc<Mutex<Vec<crate::ClusterRoleBindingItem>>>,
        yaml_editor_window: Arc<Mutex<YamlEditorWindow>>,
        client: Arc<crate::Client>
    )
{
    let guard_details = details.lock().unwrap(); // More detailed info
    let guard_cluster_rbs = cluster_rbs.lock().unwrap(); // ClusterRoleBindings with base details already we have
    if guard_details.name.is_none() {
        return;
    }
    let cluster_rb_item = guard_cluster_rbs.iter().find(|item| item.name == guard_details.name.clone().unwrap());
    if cluster_rb_item.is_none() {
        return;
    }

    let response = egui::Window::new("Cluster role binding details").min_width(800.0).collapsible(false).resizable(true).open(&mut cluster_rb_details_window.show).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("📃 Logs").size(16.0).color(crate::GRAY_BUTTON)).clicked() {
                // TODO
                log::warn!("TODO! Not implemented yet");
            }

            if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(crate::GREEN_BUTTON)).clicked() {
                crate::edit_cluster_yaml_for::<k8s_openapi::api::rbac::v1::ClusterRoleBinding>(
                    guard_details.name.clone().unwrap(),
                    Arc::clone(&yaml_editor_window),
                    Arc::clone(&client),
                );
            }

            if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(crate::RED_BUTTON)).clicked() {
                // TODO
                log::warn!("TODO! Not implemented yet");
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
            egui::Grid::new("cluster_rb_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                ui.label(egui::RichText::new("ClusterRoleBinding name:").color(ROW_NAME_COLOR));
                ui.label(egui::RichText::new(guard_details.name.clone().unwrap()).color(DETAIL_COLOR));
                ui.end_row();

                if let Some(item) = cluster_rb_item {
                    if let Some(creation_timestamp) = &item.creation_timestamp {
                        ui.label(egui::RichText::new("Creation time:").color(ROW_NAME_COLOR));
                        let timestamp_text = format!("{}, {} ago", creation_timestamp.0, crate::format_age(creation_timestamp));
                        ui.label(egui::RichText::new(timestamp_text).color(DETAIL_COLOR));
                        ui.end_row();
                    }
                }

                if let Some(labels) = guard_details.labels.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Labels:").color(ROW_NAME_COLOR));
                    egui::Grid::new("cluster_rb_details_labels_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for (j, y) in labels.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(annotations) = guard_details.annotations.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Annotations:").color(ROW_NAME_COLOR));
                    egui::Grid::new("cluster_rb_details_annotations_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                        for (j, y) in annotations.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            });

            ui.separator();
            ui.heading(egui::RichText::new("Events:").color(ROW_NAME_COLOR));
            if !guard_details.events.is_empty() {
                egui::Grid::new("cluster_rb_details_events_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                    for event in &guard_details.events {
                        ui.label(egui::RichText::new(event.timestamp.clone().unwrap_or_default()).color(DETAIL_COLOR));
                        ui.label(egui::RichText::new(event.reason.clone().unwrap_or_default()).color(SECOND_DETAIL_COLOR));
                        ui.label(egui::RichText::new(event.message.clone().unwrap_or_default()).color(item_color(&event.event_type.clone().unwrap_or_default())));
                        ui.end_row();
                    }
                });
            }
        });
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            cluster_rb_details_window.show = false;
        }
    }
}
