use std::sync::{Arc, Mutex};
use egui::{Context, Key};
use crate::theme::*;

pub struct CrdDetailsWindow {
    pub show: bool,
}

impl CrdDetailsWindow {
    pub fn new() -> Self {
        Self {
            show: false,
        }
    }
}

pub fn show_crd_details_window(
        ctx: &Context,
        crd_details_window: &mut CrdDetailsWindow,
        details: Arc<Mutex<crate::CrdDetails>>,
        crds: Arc<Mutex<Vec<crate::CRDItem>>>,
        delete_confirm: &mut super::DeleteConfirmation,
        client: Arc<crate::Client>,
        yaml_editor_window: Arc<Mutex<super::YamlEditorWindow>>,
) {
    let guard_details = details.lock().unwrap(); // More detailed info
    let guard_crds = crds.lock().unwrap(); // Crds with base details already we have
    if guard_details.name.is_none() {
        return;
    }
    let crd_item = guard_crds.iter().find(|item| item.name == guard_details.name.clone().unwrap());
    if crd_item.is_none() {
        return;
    }

    let response = egui::Window::new("Crd details").min_width(800.0).collapsible(false).resizable(true).open(&mut crd_details_window.show).show(ctx, |ui| {
        if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(crate::GREEN_BUTTON)).clicked() {
            crate::edit_cluster_yaml_for::<k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition>(
                guard_details.name.clone().unwrap(),
                Arc::clone(&yaml_editor_window),
                Arc::clone(&client),
            );
        }

        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(crate::RED_BUTTON)).clicked() {
                let name = guard_details.name.clone().unwrap();

                delete_confirm.request(name.clone(), None, move || {
                    tokio::spawn(async move {
                        if let Err(err) = crate::delete_cluster_crd(Arc::clone(&client), &name).await {
                            log::error!("Failed to delete crd: {}", err);
                        }
                    });
                });
            }
        });
        egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
            egui::Grid::new("crd_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                ui.label(egui::RichText::new("Crd name:").color(ROW_NAME_COLOR));
                ui.label(egui::RichText::new(guard_details.name.clone().unwrap()).color(DETAIL_COLOR));
                ui.end_row();

                if let Some(item) = crd_item {
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
                    egui::Grid::new("crd_details_labels_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
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
                    egui::Grid::new("crd_details_annotations_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                        for (j, y) in annotations.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            });
        });
    });
    crate::show_delete_confirmation(ctx, delete_confirm);

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            crd_details_window.show = false;
        }
    }
}
