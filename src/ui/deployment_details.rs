use std::sync::{Arc, Mutex};
use egui::{Context, Key};
use crate::{functions::item_color, theme::*, ui::YamlEditorWindow};

pub struct DeploymentDetailsWindow {
    pub show: bool,
}

impl DeploymentDetailsWindow {
    pub fn new() -> Self {
        Self {
            show: false,
        }
    }
}

pub fn show_deployment_details_window(
    ctx: &Context,
    deployment_details_window: &mut DeploymentDetailsWindow,
    details: Arc<Mutex<crate::DeploymentDetails>>,
    deployments: Arc<Mutex<Vec<crate::DeploymentItem>>>,
    yaml_editor_window: Arc<Mutex<YamlEditorWindow>>,
    client: Arc<crate::Client>,
    delete_confirm: &mut super::DeleteConfirmation,
) {
    let guard_details = details.lock().unwrap(); // More detailed info
    let guard_deployments = deployments.lock().unwrap(); // Deployments with base details already we have
    if guard_details.name.is_none() {
        return;
    }
    let deployment_item = guard_deployments.iter().find(|item| item.name == guard_details.name.clone().unwrap() && item.namespace == guard_details.namespace.clone());
    if deployment_item.is_none() {
        return;
    }
    let cur_ns = &deployment_item.unwrap().namespace;

    let response = egui::Window::new("Deployment details").min_width(800.0).collapsible(false).resizable(true).open(&mut deployment_details_window.show).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("📃 Logs").size(16.0).color(crate::GRAY_BUTTON)).clicked() {
                // TODO
            }

            if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(crate::GREEN_BUTTON)).clicked() {
                crate::edit_yaml_for::<k8s_openapi::api::apps::v1::Deployment>(
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
                        if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::apps::v1::Deployment>(
                            name,
                            ns.as_deref(),
                            Arc::clone(&client),
                        ).await {
                            log::error!("Failed to delete deployment: {}", err);
                        }
                    });
                });
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
            egui::Grid::new("deployment_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                ui.label(egui::RichText::new("Deployment name:").color(ROW_NAME_COLOR));
                ui.label(egui::RichText::new(guard_details.name.clone().unwrap()).color(DETAIL_COLOR));
                ui.end_row();

                if let Some(item) = deployment_item {
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
                    let replicas_label = format!("{} desired, {} updated, {} available, {} ready, {} unavailable", &item.replicas, &item.updated_replicas, &item.available_replicas, &item.ready_replicas, &item.unavailable_replicas);
                    ui.label(egui::RichText::new(replicas_label).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if guard_details.conditions.len() > 0 {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Conditions:").color(ROW_NAME_COLOR));
                    egui::Grid::new("deployment_details_conditions_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for item in guard_details.conditions.iter() {
                            ui.label(egui::RichText::new(&item.type_).color(item_color(&item.type_)));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(labels) = guard_details.labels.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Labels:").color(ROW_NAME_COLOR));
                    egui::Grid::new("deployment_details_labels_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
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
                    egui::Grid::new("deployment_details_annotations_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                        for (j, y) in annotations.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(strategy) = guard_details.strategy.clone() {
                    ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Stratedy type:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(strategy).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if guard_details.selector.len() > 0 {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Selector:").color(ROW_NAME_COLOR));
                    egui::Grid::new("deployment_details_selector_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        ui.label(egui::RichText::new("Key").color(SELECTOR_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Value").color(SELECTOR_HEAD_GRID_COLOR));
                        ui.end_row();
                        for (i, j) in guard_details.selector.iter() {
                            ui.label(egui::RichText::new(i).color(SELECTOR_NAME_COLUMN_COLOR));
                            ui.label(egui::RichText::new(j).color(SELECTOR_NAME_COLUMN_COLOR));
                            ui.end_row();
                        }
                        ui.end_row();
                    });
                }
            });

            ui.separator();
            ui.heading(egui::RichText::new("Events:").color(ROW_NAME_COLOR));
            if !guard_details.events.is_empty() {
                egui::Grid::new("deployment_details_events_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
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
            deployment_details_window.show = false;
        }
    }
}
