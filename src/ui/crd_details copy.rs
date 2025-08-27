use std::sync::{Arc, Mutex};
use egui::Context;
use kube::{api::{ApiResource, GroupVersionKind}, Api, Client};
// use crate::functions::item_color;
// use crate::{ui::YamlEditorWindow};
use crate::{theme::*};
use kube::ResourceExt;

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

pub async fn get_cr_list(client: Arc<Client>, crd: &crate::CRDItem, ns: Option<String>) -> Result<Vec<String>, kube::Error> {
    let ar = ApiResource::from_gvk_with_plural(
        &GroupVersionKind::gvk(&crd.group, &crd.version, &crd.kind),
        &crd.plural,
    );

    let api: Api<kube::api::DynamicObject> = if crd.scope == "Namespaced" {
        // Если ресурс находится в namespace, нужно указать его.
        // Здесь для примера "default", но в реальном приложении нужно будет выбрать namespace.
        Api::namespaced_with(client.as_ref().clone(), &ns.unwrap_or("default".to_string()), &ar)
    } else {
        Api::all_with(client.as_ref().clone(), &ar)
    };

    let list = api.list(&Default::default()).await?;
    let names = list.items.iter().map(|item| item.name_any()).collect();
    Ok(names)
}

pub fn show_crd_details_window(
    ctx: &Context,
    crd_details_window: &mut CrdDetailsWindow,
    details: Arc<Mutex<crate::CrdDetails>>,
    crds: Arc<Mutex<Vec<crate::CRDItem>>>,
    //yaml_editor_window: Arc<Mutex<YamlEditorWindow>>,
    //client: Arc<crate::Client>,
    delete_confirm: &mut super::DeleteConfirmation,
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

    egui::Window::new("CRD details").min_width(800.0).collapsible(false).resizable(true).open(&mut crd_details_window.show).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(crate::GREEN_BUTTON)).clicked() {
                // crate::edit_yaml_for::<k8s_openapi::api::batch::v1::Crd>(
                //     guard_details.name.clone().unwrap(),
                //     cur_ns.to_owned().unwrap(),
                //     Arc::clone(&yaml_editor_window),
                //     Arc::clone(&client),
                // );
            }

            if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(crate::RED_BUTTON)).clicked() {
                // let name = guard_details.name.clone().unwrap();
                // let ns = cur_ns.clone();

                // delete_confirm.request(name.clone(), ns.clone(), move || {
                //     tokio::spawn(async move {
                //         if let Err(err) = crate::delete_namespaced_component_for::<k8s_openapi::api::batch::v1::Crd>(
                //             name,
                //             ns.as_deref(),
                //             Arc::clone(&client),
                //         ).await {
                //             eprintln!("Failed to delete cronJob: {}", err);
                //         }
                //     });
                // });
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
            egui::Grid::new("crd_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                ui.label(egui::RichText::new("CRD name:").color(ROW_NAME_COLOR));
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

            // ui.separator();
            // ui.heading(egui::RichText::new("Events:").color(ROW_NAME_COLOR));
            // if !guard_details.events.is_empty() {
            //     egui::Grid::new("job_details_events_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
            //         for event in &guard_details.events {
            //             ui.label(egui::RichText::new(event.timestamp.clone().unwrap_or_default()).color(DETAIL_COLOR));
            //             ui.label(egui::RichText::new(event.reason.clone().unwrap_or_default()).color(SECOND_DETAIL_COLOR));
            //             ui.label(egui::RichText::new(event.message.clone().unwrap_or_default()).color(item_color(&event.event_type.clone().unwrap_or_default())));
            //             ui.end_row();
            //         }
            //     });
            // }
        });
    });
    crate::show_delete_confirmation(ctx, delete_confirm);
}
