use std::sync::{Arc, Mutex};
use egui::{Context, Key};
use crate::functions::item_color;
use crate::{ui::YamlEditorWindow};
use crate::theme::*;

pub struct ServiceDetailsWindow {
    pub show: bool,
}

impl ServiceDetailsWindow {
    pub fn new() -> Self {
        Self {
            show: false,
        }
    }
}

pub fn show_service_details_window(
        ctx: &Context,
        service_details_window: &mut ServiceDetailsWindow,
        details: Arc<Mutex<crate::ServiceDetails>>,
        services: Arc<Mutex<Vec<crate::ServiceItem>>>,
        yaml_editor_window: Arc<Mutex<YamlEditorWindow>>,
        client: Arc<crate::Client>,
        delete_confirm: &mut super::DeleteConfirmation,
) {
    let guard_details = details.lock().unwrap(); // More detailed info
    let guard_services = services.lock().unwrap(); // Services with base details already we have
    if guard_details.name.is_none() {
        return;
    }
    let service_item = guard_services.iter().find(|item| item.name == guard_details.name.clone().unwrap() && item.namespace == guard_details.namespace.clone());
    if service_item.is_none() {
        return;
    }
    let cur_ns = &service_item.unwrap().namespace;

    let response = egui::Window::new("Service details").min_width(800.0).collapsible(false).resizable(true).open(&mut service_details_window.show).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("📃 Logs").size(16.0).color(crate::GRAY_BUTTON)).clicked() {
                // TODO
                log::warn!("TODO! Not implemented yet");
            }

            if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(crate::GREEN_BUTTON)).clicked() {
                crate::edit_yaml_for::<k8s_openapi::api::core::v1::Service>(
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
                        if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::core::v1::Service>(
                            name,
                            ns.as_deref(),
                            Arc::clone(&client),
                        ).await {
                            log::error!("Failed to delete service: {}", err);
                        }
                    });
                });
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
            egui::Grid::new("service_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                ui.label(egui::RichText::new("Service name:").color(ROW_NAME_COLOR));
                ui.label(egui::RichText::new(guard_details.name.clone().unwrap()).color(DETAIL_COLOR));
                ui.end_row();

                if let Some(item) = service_item {
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

                    ui.label(egui::RichText::new("Type:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(&item.svc_type).color(DETAIL_COLOR));
                    ui.end_row();

                    if &item.cluster_ip != "None" {
                        ui.label(egui::RichText::new("Internal IP:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(&item.cluster_ip).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if &item.external_ip != "None" {
                        ui.label(egui::RichText::new("External IP:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(&item.external_ip).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    ui.separator(); ui.separator(); ui.end_row();

                    ui.label(egui::RichText::new("Ports:").color(ROW_NAME_COLOR));
                    ui.add(egui::Label::new(egui::RichText::new(&item.ports).color(DETAIL_COLOR)).wrap());
                    ui.end_row();
                }

                if let Some(labels) = guard_details.labels.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Labels:").color(ROW_NAME_COLOR));
                    egui::Grid::new("service_details_labels_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for (j, y) in labels.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(selector) = guard_details.selector.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Selector:").color(ROW_NAME_COLOR));
                    egui::Grid::new("service_details_selector_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for (j, y) in selector.iter() {
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
                    egui::Grid::new("service_details_annotations_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
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
                egui::Grid::new("service_details_events_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
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
            service_details_window.show = false;
        }
    }
}
