use egui::{Context, Key};
use uuid::Uuid;

#[derive(Default)]
pub struct UUIDGenWindow {
    pub show: bool,
    pub uuid: String,
}

impl UUIDGenWindow {
    pub fn default() -> Self {
        Self {
            show: false,
            uuid: Uuid::new_v4().to_string(),
        }
    }
}

pub fn show_uuid_gen_window(ctx: &Context, uuid_gen_window: &mut UUIDGenWindow,) {
    let response = egui::Window::new("UUID Generator").collapsible(false).resizable(true).open(&mut uuid_gen_window.show).show(ctx, |ui| {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.colored_label(egui::Color32::LIGHT_GREEN, &uuid_gen_window.uuid);
            ui.separator();
            if ui.button("📋").on_hover_text("Copy UUID to clipboard").clicked() {
                ui.ctx().copy_text(uuid_gen_window.uuid.clone());
            }
            ui.separator();
            if ui.button("🔃 Generate").clicked() {
                uuid_gen_window.uuid = Uuid::new_v4().to_string();
            }
        });
        ui.add_space(10.0);
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            uuid_gen_window.show = false;
        }
    }
}
