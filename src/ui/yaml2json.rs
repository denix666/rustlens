use egui::{Context, Key};
use serde_json::Value as JsonValue;

pub struct Yaml2JsonWindow {
    pub yaml_content: String,
    pub json_content: String,
    pub show: bool,
}

impl Yaml2JsonWindow {
    pub fn new() -> Self {
        Self {
            yaml_content: String::new(),
            json_content: String::new(),
            show: false,
        }
    }
}

pub fn show_yaml2json_window(ctx: &Context, yaml2json_window: &mut Yaml2JsonWindow,) {
    let response = egui::Window::new("YAML/JSON Converter").collapsible(false).resizable(true).open(&mut yaml2json_window.show).show(ctx, |ui| {
        ui.vertical(|ui| {
            ui.group(|ui| {
                ui.set_height(270.0);
                ui.label("YAML:");
                egui::ScrollArea::vertical().id_salt("yaml").show(ui, |ui| {
                    let yaml_text = egui::TextEdit::multiline(&mut yaml2json_window.yaml_content)
                        .code_editor()
                        .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                        .text_color(egui::Color32::GREEN)
                        .desired_rows(15)
                        .desired_width(f32::INFINITY)
                        .lock_focus(true);
                    if ui.add(yaml_text).changed() {
                        match serde_yaml::from_str::<serde_yaml::Value>(yaml2json_window.yaml_content.as_str()) {
                            Ok(v) => {
                                let j: Result<JsonValue, _> = serde_json::from_str(&serde_json::to_string(&v).unwrap());
                                match j {
                                    Ok(jv) => match serde_json::to_string_pretty(&jv) {
                                        Ok(s) => {
                                            yaml2json_window.json_content = s;
                                        }
                                        Err(_) => {},
                                    },
                                    Err(_) => {}
                                }
                            }
                            Err(_) => {}
                        }
                    }
                });
            });
            ui.group(|ui| {
                ui.set_height(270.0);
                ui.label("JSON:");
                egui::ScrollArea::vertical().id_salt("json").show(ui, |ui| {
                    let json_text = egui::TextEdit::multiline(&mut yaml2json_window.json_content)
                        .code_editor()
                        .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                        .text_color(egui::Color32::GREEN)
                        .desired_rows(15)
                        .desired_width(f32::INFINITY)
                        .lock_focus(true);
                    if ui.add(json_text).changed() {
                        match serde_json::from_str::<JsonValue>(yaml2json_window.json_content.as_str()) {
                            Ok(v) => match serde_yaml::to_string(&v) {
                                Ok(y) => {
                                    yaml2json_window.yaml_content = y;
                                }
                                Err(_) => {}
                            },
                            Err(_) => {}
                        }
                    }
                });
            });
        });
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            yaml2json_window.show = false;
        }
    }
}
