use std::sync::{Arc, Mutex};
use egui::{Context, Key};
use crate::functions::item_color;
use crate::{ui::YamlEditorWindow};
use crate::theme::*;

pub struct ReplicaSetDetailsWindow {
    pub show: bool,
}

impl ReplicaSetDetailsWindow {
    pub fn new() -> Self {
        Self {
            show: false,
        }
    }
}

pub fn show_replicaset_details_window(
        ctx: &Context,
        replicaset_details_window: &mut ReplicaSetDetailsWindow,
        details: Arc<Mutex<crate::ReplicaSetDetails>>,
        replicasets: Arc<Mutex<Vec<crate::ReplicaSetItem>>>,
        yaml_editor_window: Arc<Mutex<YamlEditorWindow>>,
        client: Arc<crate::Client>,
        delete_confirm: &mut super::DeleteConfirmation,
) {
    let guard_details = details.lock().unwrap(); // More detailed info
    let guard_replicasets = replicasets.lock().unwrap(); // ReplicaSets with base details already we have
    if guard_details.name.is_none() {
        return;
    }
    let replicaset_item = guard_replicasets.iter().find(|item| item.name == guard_details.name.clone().unwrap() && item.namespace == guard_details.namespace.clone());
    if replicaset_item.is_none() {
        return;
    }
    let cur_ns = &replicaset_item.unwrap().namespace;

    let response = egui::Window::new("ReplicaSet details").min_width(800.0).collapsible(false).resizable(true).open(&mut replicaset_details_window.show).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("📃 Logs").size(16.0).color(crate::GRAY_BUTTON)).clicked() {
                // TODO
                println!("TODO");
            }

            if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(crate::GREEN_BUTTON)).clicked() {
                crate::edit_yaml_for::<k8s_openapi::api::apps::v1::ReplicaSet>(
                    guard_details.name.clone().unwrap(),
                    cur_ns.to_owned().unwrap(),
                    Arc::clone(&yaml_editor_window),
                    Arc::clone(&client),
                );
            }

            if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(crate::RED_BUTTON)).clicked() {
                let name = guard_details.name.clone().unwrap();
                let ns = cur_ns.clone();

                delete_confirm.request(name.clone(), ns.clone(), move || {
                    tokio::spawn(async move {
                        if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::apps::v1::ReplicaSet>(
                            name,
                            ns.as_deref(),
                            Arc::clone(&client),
                        ).await {
                            eprintln!("Failed to delete replicaset: {}", err);
                        }
                    });
                });
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
            egui::Grid::new("replicaset_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                ui.label(egui::RichText::new("ReplicaSet name:").color(ROW_NAME_COLOR));
                ui.label(egui::RichText::new(guard_details.name.clone().unwrap()).color(DETAIL_COLOR));
                ui.end_row();

                if let Some(item) = replicaset_item {
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

                    ui.label(egui::RichText::new("Replicas status:").color(ROW_NAME_COLOR));
                    let replicas_label = format!("{} desired, {} current, {} ready", &item.desired, &item.current, &item.ready);
                    ui.label(egui::RichText::new(replicas_label).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(labels) = guard_details.labels.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Labels:").color(ROW_NAME_COLOR));
                    egui::Grid::new("replicaset_details_labels_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
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
                    egui::Grid::new("replicaset_details_annotations_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
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
                egui::Grid::new("replicaset_details_events_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
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
    crate::show_delete_confirmation(ctx, delete_confirm);

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            replicaset_details_window.show = false;
        }
    }
}
