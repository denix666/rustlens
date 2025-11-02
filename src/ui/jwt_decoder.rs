use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use egui::{Context, Key};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde_json::Value;

use crate::theme::GREEN_BUTTON;

type HmacSha256 = Hmac<Sha256>;

fn decode_base64url(part: &str) -> Result<String, String> {
    let cleaned = part.trim();
    if cleaned.is_empty() {
        return Err("Empty part".into());
    }
    match URL_SAFE_NO_PAD.decode(cleaned) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => Ok(s),
            Err(_) => Err("Decoded bytes are not valid UTF-8".into()),
        },
        Err(e) => Err(format!("Base64url decode error: {}", e)),
    }
}

fn encode_base64url(s: &str) -> String {
    URL_SAFE_NO_PAD.encode(s.as_bytes())
}

fn pretty_json(s: &str) -> Result<String, String> {
    match serde_json::from_str::<Value>(s) {
        Ok(v) => serde_json::to_string_pretty(&v).map_err(|e| e.to_string()),
        Err(e) => Err(format!("Not valid JSON: {}", e)),
    }
}

#[derive(Default)]
pub struct JwtDecoderWindow {
    pub show: bool,
    token: String,
    header: String,
    payload: String,
    signature: String,
    secret: String,
    header_err: Option<String>,
    payload_err: Option<String>,
    updating_from_token: bool,
    updating_from_fields: bool,
}

impl JwtDecoderWindow {
    pub fn default() -> Self {
        Self {
            show: false,
            token: Default::default(),
            header: Default::default(),
            payload: Default::default(),
            signature: Default::default(),
            secret: Default::default(),
            header_err: Default::default(),
            payload_err: Default::default(),
            updating_from_token: Default::default(),
            updating_from_fields: Default::default(),
        }
    }

    fn decode_parts(&mut self) {
        if self.updating_from_fields {
            return;
        }

        let t = self.token.trim();
        if t.is_empty() {
            self.header.clear();
            self.payload.clear();
            self.signature.clear();
            return;
        }

        let parts: Vec<&str> = t.split('.').collect();
        if parts.len() < 2 {
            self.header_err = Some("Invalid token: expected at least 2 parts".into());
            self.payload_err = Some("Invalid token: expected at least 2 parts".into());
            return;
        }

        self.updating_from_token = true;

        // Header
        match decode_base64url(parts[0]) {
            Ok(s) => match pretty_json(&s) {
                Ok(pretty) => {
                    self.header = pretty;
                    self.header_err = None;
                }
                Err(_) => {
                    self.header = s;
                    self.header_err = Some("Decoded but not valid JSON".into());
                }
            },
            Err(e) => {
                self.header = String::new();
                self.header_err = Some(e);
            }
        }

        // Payload
        match decode_base64url(parts[1]) {
            Ok(s) => match pretty_json(&s) {
                Ok(pretty) => {
                    self.payload = pretty;
                    self.payload_err = None;
                }
                Err(_) => {
                    self.payload = s;
                    self.payload_err = Some("Decoded but not valid JSON".into());
                }
            },
            Err(e) => {
                self.payload = String::new();
                self.payload_err = Some(e);
            }
        }

        // Signature (raw base64url)
        self.signature = if parts.len() >= 3 {
            parts[2].to_string()
        } else {
            String::new()
        };

        self.updating_from_token = false;
    }

    fn encode_parts(&mut self) {
        if self.updating_from_token {
            return;
        }

        self.updating_from_fields = true;

        let h_json = self.header.trim();
        let p_json = self.payload.trim();

        let h_str = if serde_json::from_str::<Value>(h_json).is_ok() {
            serde_json::to_string(&serde_json::from_str::<Value>(h_json).unwrap()).unwrap()
        } else {
            h_json.to_string()
        };

        let p_str = if serde_json::from_str::<Value>(p_json).is_ok() {
            serde_json::to_string(&serde_json::from_str::<Value>(p_json).unwrap()).unwrap()
        } else {
            p_json.to_string()
        };

        let header_enc = encode_base64url(&h_str);
        let payload_enc = encode_base64url(&p_str);

        // --- Generate signature HS256 ---
        let signing_input = format!("{}.{}", header_enc, payload_enc);
        let signature = if !self.secret.is_empty() {
            let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
                .expect("HMAC can take key of any size");
            mac.update(signing_input.as_bytes());
            let result = mac.finalize().into_bytes();
            URL_SAFE_NO_PAD.encode(result)
        } else {
            String::new()
        };

        self.signature = signature.clone();

        if self.signature.is_empty() {
            self.token = format!("{}.{}", header_enc, payload_enc);
        } else {
            self.token = format!("{}.{}.{}", header_enc, payload_enc, signature);
        }

        self.updating_from_fields = false;
    }
}

pub fn show_jwt_decoder_window(ctx: &Context, jwt: &mut JwtDecoderWindow,) {
    let response = egui::Window::new("JWT decoder").collapsible(false).resizable(true).show(ctx, |ui| {
        ui.vertical(|ui| {
            ui.group(|ui| {
                ui.set_height(570.0);
                ui.set_width(860.0);
                ui.label("Token:");
                egui::ScrollArea::vertical().id_salt("token").max_height(90.0).show(ui, |ui| {
                    if ui.add(egui::TextEdit::multiline(&mut jwt.token).desired_width(850.0).desired_rows(5)).changed() {
                        jwt.decode_parts();
                    }
                });

                ui.separator();
                ui.label("Header");
                if ui.add(egui::TextEdit::multiline(&mut jwt.header).desired_width(850.0).desired_rows(5)).changed() {
                    jwt.encode_parts();
                }

                ui.separator();
                ui.label("Payload");
                if ui.add(egui::TextEdit::multiline(&mut jwt.payload).desired_width(850.0).desired_rows(7)).changed() {
                    jwt.encode_parts();
                }

                ui.separator();
                ui.label("Signature (HS256, base64url)");
                ui.add_enabled(false, egui::TextEdit::multiline(&mut jwt.signature).desired_width(850.0).desired_rows(3));

                ui.separator();
                ui.label("Secret (for signature):");
                if ui.add(egui::TextEdit::singleline(&mut jwt.secret).desired_width(850.0)).changed() {
                    jwt.encode_parts();
                }

                ui.separator();
                ui.add_space(30.0);
                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new("📋 Copy token").size(17.0).color(GREEN_BUTTON)).on_hover_text("Copy token").clicked() {
                        ui.ctx().copy_text(jwt.token.clone());
                    }
                    if ui.button(egui::RichText::new("Close").size(17.0).color(egui::Color32::GRAY)).on_hover_text("Close window").clicked() {
                        jwt.show = false;
                    }
                });
            });
        });
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            jwt.show = false;
        }
    }
}
