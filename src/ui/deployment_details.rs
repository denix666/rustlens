use std::sync::{Arc, Mutex};
use egui::{Context};
use crate::{ui::YamlEditorWindow, theme::*};

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
        client: Arc<crate::Client>)
{
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

    egui::Window::new("Deployment details").min_width(800.0).collapsible(false).resizable(true).open(&mut deployment_details_window.show).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("📃 Logs").size(16.0).color(crate::GRAY_BUTTON)).clicked() {
                // TODO
            }

            if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(crate::GREEN_BUTTON)).clicked() {
                crate::edit_yaml_for_deployment(
                    guard_details.name.clone().unwrap(),
                    cur_ns.to_owned().unwrap(),
                    Arc::clone(&yaml_editor_window),
                    Arc::clone(&client),
                );
            }

            if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(crate::RED_BUTTON)).clicked() {
                // TODO
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
                }

                if let Some(i) = guard_details.labels.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Labels:").color(ROW_NAME_COLOR));
                    egui::Grid::new("deployment_details_labels_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
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
    });
}
