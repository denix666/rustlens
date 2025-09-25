use std::sync::Arc;
use egui::{Context, Key};
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
    let response = egui::Window::new("Create New Resource").collapsible(false).resizable(true).default_width(600.0).show(ctx, |ui| {
        if new_resource_window.content.is_empty() {
            new_resource_window.content = match new_resource_window.resource_type {
                crate::ResourceType::NameSpace => crate::NAMESPACE_TEMPLATE.to_string(),
                crate::ResourceType::Pod => crate::POD_TEMPLATE.to_string(),
                crate::ResourceType::Deployment => crate::DEPLOYMENT_TEMPLATE.to_string(),
                crate::ResourceType::Service => crate::SERVICE_TEMPLATE.to_string(),
                crate::ResourceType::DaemonSet => crate::DAEMONSET_TEMPLATE.to_string(),
                crate::ResourceType::Ingress => crate::INGRESS_TEMPLATE.to_string(),
                crate::ResourceType::ReplicaSet => crate::REPLICASET_TEMPLATE.to_string(),
                crate::ResourceType::PodWithPvc => crate::POD_WITH_PVC_TEMPLATE.to_string(),
                crate::ResourceType::ConfigMap => crate::CONFIGMAP_TEMPLATE.to_string(),
                crate::ResourceType::Secret => crate::SECRET_TEMPLATE.to_string(),
                crate::ResourceType::ExternalSecret => crate::EXTERNAL_SECRET_TEMPLATE.to_string(),
                crate::ResourceType::Role => crate::ROLE_TEMPLATE.to_string(),
                crate::ResourceType::RoleBinding => crate::CLUSTER_ROLE_BINDING_TEMPLATE.to_string(),
                crate::ResourceType::ClusterRole => crate::CLUSTER_ROLE_TEMPLATE.to_string(),
                crate::ResourceType::ClusterRoleBinding => crate::CLUSTER_ROLE_BINDING_TEMPLATE.to_string(),
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
                    crate::ResourceType::Deployment => "Deployment",
                    crate::ResourceType::Role => "Role",
                    crate::ResourceType::RoleBinding => "RoleBinding",
                    crate::ResourceType::Service => "Service",
                    crate::ResourceType::Ingress => "Ingress",
                    crate::ResourceType::DaemonSet => "DaemonSet",
                    crate::ResourceType::ReplicaSet => "ReplicaSet",
                    crate::ResourceType::ConfigMap => "Configmap",
                    crate::ResourceType::ClusterRole => "Cluster role",
                    crate::ResourceType::ClusterRoleBinding => "Cluster role binding",
                    crate::ResourceType::ExternalSecret => "External secret",
                    crate::ResourceType::Pod => "Pod",
                    crate::ResourceType::PodWithPvc => "Pod with PVC",
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
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::Deployment, "Deployment",).clicked() {
                        new_resource_window.content = crate::DEPLOYMENT_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::Service, "Service",).clicked() {
                        new_resource_window.content = crate::SERVICE_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::DaemonSet, "DaemonSet",).clicked() {
                        new_resource_window.content = crate::DAEMONSET_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::Ingress, "Ingress",).clicked() {
                        new_resource_window.content = crate::INGRESS_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::ReplicaSet, "ReplicaSet",).clicked() {
                        new_resource_window.content = crate::REPLICASET_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::Pod, "Pod",).clicked() {
                        new_resource_window.content = crate::POD_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::PodWithPvc, "Pod with PVC",).clicked() {
                        new_resource_window.content = crate::POD_WITH_PVC_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::ConfigMap, "Configmap",).clicked() {
                        new_resource_window.content = crate::CONFIGMAP_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::ServiceAccount, "Service account",).clicked() {
                        new_resource_window.content = crate::SERVICE_ACCOUNT_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::Role, "Role",).clicked() {
                        new_resource_window.content = crate::ROLE_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::RoleBinding, "RoleBinding",).clicked() {
                        new_resource_window.content = crate::ROLE_BINDING_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::ClusterRole, "Cluster role",).clicked() {
                        new_resource_window.content = crate::CLUSTER_ROLE_TEMPLATE.to_string();
                    };
                    if ui.selectable_value(&mut new_resource_window.resource_type, crate::ResourceType::ClusterRoleBinding, "Cluster role binding",).clicked() {
                        new_resource_window.content = crate::CLUSTER_ROLE_TEMPLATE.to_string();
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

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            new_resource_window.show = false;
        }
    }
}
