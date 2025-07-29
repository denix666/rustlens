use egui::{Context, ScrollArea, TextStyle};
use kube::Client;
use std::sync::{Arc, Mutex};

pub struct LogWindow {
    pub pod_name: String,
    pub containers: Vec<crate::ContainerStatusItem>,
    pub show: bool,
    pub namespace: String,
    pub selected_container: String,
    pub buffer: Arc<Mutex<String>>,
    pub last_container: Option<String>,
}

impl LogWindow {
    pub fn new() -> Self {
        Self {
            pod_name: String::new(),
            containers: Vec::new(),
            selected_container: String::new(),
            namespace: String::new(),
            show: false,
            buffer: Arc::new(Mutex::new(String::new())),
            last_container: None,
        }
    }
}

pub fn show_log_window(ctx: &Context, log_window: &mut LogWindow, client: Arc<Client>) {
    egui::Window::new("Logs")
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Container:");
                egui::ComboBox::from_id_salt("containers_combo")
                    .selected_text(&log_window.selected_container)
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for container in &log_window.containers {
                            ui.selectable_value(
                                &mut log_window.selected_container,
                                container.name.clone(),
                                &container.name,
                            );
                        }
                    });
            });

            if log_window.last_container.as_ref() != Some(&log_window.selected_container) {
                log_window.last_container = Some(log_window.selected_container.clone());
                let buf_clone = Arc::clone(&log_window.buffer);
                let cur_ns = log_window.namespace.clone();
                let cur_pod = log_window.pod_name.clone();
                let cur_container = log_window.selected_container.clone();
                let client_clone = Arc::clone(&client);
                tokio::spawn(async move {
                    crate::fetch_logs(client_clone,
                    &cur_ns,
                     &cur_pod,
                    &cur_container, buf_clone).await;
                });
            }

            ScrollArea::vertical().show(ui, |ui| {
                if let Ok(logs) = log_window.buffer.lock() {
                    ui.add(
                        egui::TextEdit::multiline(&mut logs.clone())
                            .font(TextStyle::Monospace)
                            .desired_rows(crate::MAX_LOG_LINES)
                            .desired_width(f32::INFINITY)
                            .code_editor()
                    );
                }
            });

            ui.separator();
            if ui.button(egui::RichText::new("🗙 Close logs window").size(16.0).color(egui::Color32::WHITE)).clicked() {
                log_window.show = false;
            }
    });
}
