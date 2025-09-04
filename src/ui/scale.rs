use std::sync::Arc;
use egui::{Context, Key};
use kube::Client;

pub struct ScaleWindow {
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub cur_replicas: i32,
    pub desired_replicas: i32,
    pub show: bool,
    pub resource_kind: Option<crate::ScaleTarget>,
}

impl ScaleWindow {
    pub fn new() -> Self {
        Self {
            name: None,
            namespace: None,
            cur_replicas: 0,
            desired_replicas: 0,
            show: false,
            resource_kind: None,
        }
    }
}

pub fn show_scale_window(ctx: &Context, scale_window: &mut ScaleWindow, client: Arc<Client>) {
    let title = format!("Scale {}", scale_window.name.as_ref().unwrap());
    let response = egui::Window::new(title).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("Current replicas scale: {}", scale_window.cur_replicas));
        });
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Desired number of replicas:");
            ui.add(egui::Slider::new(&mut scale_window.desired_replicas, 0..=10));
        });
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("🗙 Cancel").size(16.0).color(egui::Color32::WHITE)).clicked() {
                scale_window.show = false;
            }
            if ui.button(egui::RichText::new("↕ Scale").size(16.0).color(egui::Color32::ORANGE)).clicked() {
                eprintln!("Scaling {} to {} replicas", scale_window.name.as_ref().unwrap(), scale_window.desired_replicas);
                let client_clone = Arc::clone(&client);
                let name = scale_window.name.as_ref().unwrap().clone();
                let namespace = scale_window.namespace.as_ref().unwrap().clone();
                let replicas = scale_window.desired_replicas;
                let kind = scale_window.resource_kind.as_ref().unwrap().clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::scale_workload(client_clone, &name, &namespace, replicas, kind).await {
                        eprintln!("Scale failed: {:?}", e);
                    }
                });
                scale_window.show = false;
            }
        });
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            scale_window.show = false;
        }
    }
}
