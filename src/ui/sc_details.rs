use std::sync::{Arc, Mutex};
use egui::{Context, Key};
use crate::functions::item_color;
use crate::{ui::YamlEditorWindow};
use crate::theme::*;

pub struct ScDetailsWindow {
    pub show: bool,
}

impl ScDetailsWindow {
    pub fn new() -> Self {
        Self {
            show: false,
        }
    }
}

pub fn show_sc_details_window(
        ctx: &Context,
        sc_details_window: &mut ScDetailsWindow,
        details: Arc<Mutex<crate::ScDetails>>,
        storage_classes: Arc<Mutex<Vec<crate::StorageClassItem>>>,
        yaml_editor_window: Arc<Mutex<YamlEditorWindow>>,
        client: Arc<crate::Client>,
        delete_confirm: &mut super::DeleteConfirmation,
) {
    let guard_details = details.lock().unwrap();
    let guard_scs = storage_classes.lock().unwrap();
    if guard_details.name.is_none() {
        return;
    }
    let sc_item = guard_scs.iter().find(|item| item.name == guard_details.name.clone().unwrap());
    if sc_item.is_none() {
        return;
    }

    let response = egui::Window::new("StorageClass details").min_width(800.0).collapsible(false).resizable(true).open(&mut sc_details_window.show).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(crate::GREEN_BUTTON)).clicked() {
                crate::edit_cluster_yaml_for::<k8s_openapi::api::storage::v1::StorageClass>(
                    guard_details.name.clone().unwrap(),
                    Arc::clone(&yaml_editor_window),
                    Arc::clone(&client),
                );
            }

            if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(crate::RED_BUTTON)).clicked() {
                let name = guard_details.name.clone().unwrap();

                delete_confirm.request(name.clone(), None, move || {
                    tokio::spawn(async move {
                        if let Err(err) = crate::delete_storage_class(Arc::clone(&client), &name).await {
                            log::error!("Failed to delete storage class: {}", err);
                        }
                    });
                });
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink(false).max_height(600.0).show(ui, |ui| {
            egui::Grid::new("sc_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                ui.label(egui::RichText::new("Name:").color(ROW_NAME_COLOR));
                ui.label(egui::RichText::new(guard_details.name.clone().unwrap()).color(DETAIL_COLOR));
                ui.end_row();

                if let Some(item) = sc_item {
                    if let Some(creation_timestamp) = &item.creation_timestamp {
                        ui.label(egui::RichText::new("Creation time:").color(ROW_NAME_COLOR));
                        let timestamp_text = format!("{}, {} ago", creation_timestamp.0, crate::format_age(creation_timestamp));
                        ui.label(egui::RichText::new(timestamp_text).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    ui.label(egui::RichText::new("Provisioner:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(&item.provisioner).color(DETAIL_COLOR));
                    ui.end_row();

                    ui.label(egui::RichText::new("Reclaim policy:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(&item.reclaim_policy).color(item_color(&item.reclaim_policy)));
                    ui.end_row();

                    ui.label(egui::RichText::new("Volume binding mode:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(&item.volume_binding_mode).color(DETAIL_COLOR));
                    ui.end_row();

                    ui.label(egui::RichText::new("Default class:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(&item.is_default).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(mount_options) = guard_details.mount_options.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Mount options:").color(ROW_NAME_COLOR));
                    egui::Grid::new("sc_details_mount_options_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                        for item in mount_options.iter() {
                            ui.label(egui::RichText::new(item).color(DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(parameters) = guard_details.parameters.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Parameters:").color(ROW_NAME_COLOR));
                    egui::Grid::new("sc_details_parameters_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                        for (j, y) in parameters.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(finalizers) = guard_details.finalizers.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Finalizers:").color(ROW_NAME_COLOR));
                    egui::Grid::new("sc_details_finalizers_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                        for item in finalizers.iter() {
                            ui.label(egui::RichText::new(item).color(DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(labels) = guard_details.labels.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Labels:").color(ROW_NAME_COLOR));
                    egui::Grid::new("sc_details_labels_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
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
                    egui::Grid::new("sc_details_annotations_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
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
                egui::Grid::new("sc_details_events_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
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

    if let Some(inner_response) = response
        && inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            sc_details_window.show = false;
        }
}
