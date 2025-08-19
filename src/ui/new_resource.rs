use std::sync::Arc;
use egui::Context;
use kube::Client;

pub struct NewResourceWindow {
    pub resource_type: crate::ResourceType,
    pub content: String,
    pub show: bool,
}

impl NewResourceWindow {
    pub fn new() -> Self {
        Self {
            resource_type: crate::ResourceType::Blank,
            content: String::new(),
            show: false,
        }
    }
}

pub fn show_new_resource_window(ctx: &Context, new_resource_window: &mut NewResourceWindow, client: Arc<Client>) {
    egui::Window::new("Create New Resource").collapsible(false).resizable(true).show(ctx, |ui| {
        if new_resource_window.content.is_empty() {
            new_resource_window.content = match new_resource_window.resource_type {
                crate::ResourceType::NameSpace => crate::NAMESPACE_TEMPLATE.to_string(),
                crate::ResourceType::Pod => crate::POD_TEMPLATE.to_string(),
                crate::ResourceType::Secret => crate::SECRET_TEMPLATE.to_string(),
                crate::ResourceType::ExternalSecret => crate::EXTERNAL_SECRET_TEMPLATE.to_string(),
                crate::ResourceType::Role => crate::ROLE_TEMPLATE.to_string(),
                crate::ResourceType::ServiceAccount => crate::SERVICE_ACCOUNT_TEMPLATE.to_string(),
                crate::ResourceType::PersistenceVolumeClaim => crate::PVC_TEMPLATE.to_string(),
                crate::ResourceType::Blank => "".to_string(),
            };
        }

        ui.horizontal(|ui| {
            ui.label("YAML Template:");
            egui::ComboBox::from_id_salt("templates_combo").width(150.0)
                .selected_text(match new_resource_window.resource_type {
                    crate::ResourceType::NameSpace => "NameSpace",
                    crate::ResourceType::Secret => "Secret",
                    crate::ResourceType::Role => "Role",
                    crate::ResourceType::ExternalSecret => "External secret",
                    crate::ResourceType::Pod => "Pod",
                    crate::ResourceType::ServiceAccount => "Service account",
                    crate::ResourceType::PersistenceVolumeClaim => "PersistenceVolumeClaim",
                    crate::ResourceType::Blank => "Blank",
                }).show_ui(ui, |ui| {
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::NameSpace, "NameSpace",).clicked() {
                        new_resource_window.content = crate::NAMESPACE_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::Secret, "Secret",).clicked() {
                        new_resource_window.content = crate::SECRET_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::Pod, "Pod",).clicked() {
                        new_resource_window.content = crate::POD_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::ServiceAccount, "Service account",).clicked() {
                        new_resource_window.content = crate::SERVICE_ACCOUNT_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::Role, "Role",).clicked() {
                        new_resource_window.content = crate::ROLE_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::ExternalSecret, "External secret",).clicked() {
                        new_resource_window.content = crate::EXTERNAL_SECRET_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::PersistenceVolumeClaim,"PersistenceVolumeClaim",).clicked() {
                        new_resource_window.content = crate::PVC_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::Blank,"Blank",).clicked() {
                        new_resource_window.content = "".to_string();
                    };
                });
        });
        ui.separator();
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut new_resource_window.content)
                .font(egui::TextStyle::Monospace)
                .code_editor()
                .text_color(egui::Color32::LIGHT_YELLOW)
                .desired_rows(25)
                .lock_focus(true)
                .desired_width(f32::INFINITY),
            );
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("✔ Apply").size(16.0).color(egui::Color32::GREEN)).clicked() {
                let yaml = new_resource_window.content.clone();
                let client_clone = Arc::clone(&client);
                let resource_type = new_resource_window.resource_type.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::apply_yaml(client_clone, &yaml, resource_type).await {
                        println!("Error applying YAML: {:?}", e);
                    }
                });
                new_resource_window.show = false;
            }

            if ui.button(egui::RichText::new("🗙 Cancel").size(16.0).color(egui::Color32::RED)).clicked() {
                new_resource_window.show = false;
            }
        });
    });
}
