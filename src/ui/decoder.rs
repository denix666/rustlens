use egui::Context;
use base64::{Engine as _, engine::general_purpose};

pub struct DecoderWindow {
    pub plain_text_content: String,
    pub encrypted_content: String,
    pub show: bool,
}

impl DecoderWindow {
    pub fn new() -> Self {
        Self {
            plain_text_content: String::new(),
            encrypted_content: String::new(),
            show: false,
        }
    }
}

pub fn show_decoder_window(ctx: &Context, decoder_window: &mut DecoderWindow,) {
    egui::Window::new("Decoder").collapsible(false).resizable(true).open(&mut decoder_window.show).show(ctx, |ui| {
        ui.vertical(|ui| {
            ui.group(|ui| {
                ui.set_height(270.0);
                ui.label("Plain text:");
                egui::ScrollArea::vertical().id_salt("plain").show(ui, |ui| {
                    let plain_text = egui::TextEdit::multiline(&mut decoder_window.plain_text_content)
                        .code_editor()
                        .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                        .text_color(egui::Color32::GREEN)
                        .desired_rows(15)
                        .desired_width(f32::INFINITY)
                        .lock_focus(true);
                    if ui.add(plain_text).changed() {
                        decoder_window.encrypted_content = general_purpose::STANDARD.encode(decoder_window.plain_text_content.to_owned());
                    }
                });
            });
            ui.group(|ui| {
                ui.set_height(270.0);
                ui.label("Decoded base64 text:");
                egui::ScrollArea::vertical().id_salt("encrypted").show(ui, |ui| {
                    let encrypted_text = egui::TextEdit::multiline(&mut decoder_window.encrypted_content)
                        .code_editor()
                        .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                        .text_color(egui::Color32::GREEN)
                        .desired_rows(15)
                        .desired_width(f32::INFINITY)
                        .lock_focus(true);
                    if ui.add(encrypted_text).changed() {
                        match general_purpose::STANDARD.decode(&decoder_window.encrypted_content) {
                            Ok(bytes) => {
                                match std::str::from_utf8(&bytes) {
                                    Ok(res) => {
                                        decoder_window.plain_text_content = res.to_string();
                                    },
                                    Err(_) => {
                                        decoder_window.plain_text_content = "Wrong input...".to_string();
                                    },
                                }
                            },
                            Err(_) => {
                                decoder_window.plain_text_content = "Invalid input...".to_string();
                            }
                        }
                    }
                });
            });
        });
    });
}
