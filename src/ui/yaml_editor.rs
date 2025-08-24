use egui::{text::LayoutJob, Color32, Context, FontId, TextFormat, TextStyle, Ui};
use kube::Client;
use std::{sync::{Arc, Mutex}, time::{Duration, Instant}};
use regex::RegexBuilder;

use crate::ui::DecoderWindow;

pub struct YamlEditorWindow {
    pub content: String,
    pub show: bool,
    pub apply_button_enabled: bool,
    pub search_query: String,
    pub status_message: Arc<Mutex<Option<(String, Instant)>>>,
    pub apply_flag: Arc<Mutex<bool>>,
}

impl YamlEditorWindow {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            show: false,
            apply_button_enabled: true,
            search_query: String::new(),
            status_message: Arc::new(Mutex::new(None)),
            apply_flag: Arc::new(Mutex::new(true)),
        }
    }
}

fn show_status_banner(ctx: &egui::Context, editor: &mut YamlEditorWindow) {
    let maybe_msg = editor.status_message.lock().unwrap().clone();

    if let Some((msg, when)) = maybe_msg {
        let elapsed = when.elapsed();
        if elapsed > Duration::from_secs(8) {
            *editor.status_message.lock().unwrap() = None;
            return;
        }

        let alpha = if elapsed > Duration::from_secs(3) {
            let left = 5.0 - elapsed.as_secs_f32();
            (left / 2.0).clamp(0.0, 1.0) // уменьшаем прозрачность
        } else {
            1.0
        };

        let bg = if msg.starts_with('✅') {
            Color32::from_rgba_unmultiplied(50, 150, 50, (255.0 * alpha) as u8)
        } else {
            Color32::from_rgba_unmultiplied(180, 60, 60, (255.0 * alpha) as u8)
        };

        egui::Area::new(egui::Id::new("status_banner"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-20.0, 20.0)) // правый верх
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(bg)
                    .corner_radius(5.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(msg)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                    });
            });
    }
}


pub fn show_yaml_editor(ctx: &Context, editor: &mut YamlEditorWindow, decoder: &mut DecoderWindow, client: Arc<Client>) {
    egui::Window::new("Edit resource").max_width(1200.0).max_height(600.0).default_width(800.0).default_height(600.0).collapsible(false).resizable(true).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(egui::TextEdit::singleline(&mut editor.search_query)
                .hint_text("Search...")
                .desired_width(200.0),
            );
            if ui.button("×").clicked() {
                editor.search_query.clear();
            }
            ui.separator();
            if ui.button(egui::RichText::new("🖹 Decoder").size(16.0).color(egui::Color32::LIGHT_BLUE)).clicked() {
                decoder.show = true;
            }
        });
        ui.separator();

        show_status_banner(ctx, editor);

        let mut layouter = make_yaml_layouter(editor.search_query.clone());

        egui::ScrollArea::vertical().hscroll(true).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut editor.content)
                .font(TextStyle::Monospace)
                .code_editor()
                .desired_rows(20)
                .desired_width(800.0)
                .layouter(&mut layouter),
            );
        });

        ui.separator();
        ui.horizontal(|ui| {
            editor.apply_button_enabled = *editor.apply_flag.lock().unwrap();
            if ui.add_enabled(editor.apply_button_enabled, egui::Button::new(egui::RichText::new("✅ Apply").size(16.0).color(egui::Color32::GREEN))).clicked() {
                let content = editor.content.clone();
                editor.apply_button_enabled = false;
                *editor.apply_flag.lock().unwrap() = false;

                match serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    Ok(_) => {
                        // YAML is valid!
                        let client_clone = Arc::clone(&client);
                        let msg_ptr = Arc::clone(&editor.status_message);
                        let apply_flag = Arc::clone(&editor.apply_flag);
                        tokio::spawn(async move {
                            let msg = match crate::patch_resource(client_clone, content.as_str()).await {
                                Ok(_) => ("✅ Applied successfully".to_string(), Instant::now()),
                                Err(e) => (format!("❌ Error applying YAML: {e}"), Instant::now()),
                            };
                            *msg_ptr.lock().unwrap() = Some(msg);
                            *apply_flag.lock().unwrap() = true;
                        });
                    }
                    Err(e) => {
                        *editor.status_message.lock().unwrap() = Some((format!("❌ YAML Error: {e}"), Instant::now()));
                        *editor.apply_flag.lock().unwrap() = true;
                    }
                }
            }
            if ui.button(egui::RichText::new("🗙 Close editor").size(16.0).color(egui::Color32::WHITE)).clicked() {
                editor.show = false;
            }
        });
    });
}

pub fn make_yaml_layouter(
    search: String,
) -> Box<dyn for<'a, 'b> FnMut(&'a Ui, &'b dyn egui::TextBuffer, f32) -> Arc<egui::Galley>> {
    let key_re = regex::Regex::new(r"(\s*)([^:\n#]+):\s*").unwrap();
    let comment_re = regex::Regex::new(r"#.*").unwrap();
    let number_re = regex::Regex::new(r"\b\d+(\.\d+)?\b").unwrap();
    let string_re = regex::Regex::new(r#""[^"]*"|'[^']*'"#).unwrap();

    let search_re = if search.is_empty() {
        None
    } else {
        Some(
            RegexBuilder::new(&regex::escape(&search))
                .case_insensitive(true)
                .build()
                .unwrap(),
        )
    };

    Box::new(move |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
        let mut job = LayoutJob::default();

        let normal = TextFormat {
            font_id: FontId::monospace(14.0),
            color: Color32::LIGHT_GRAY,
            ..Default::default()
        };
        let key_fmt = TextFormat {
            font_id: FontId::monospace(14.0),
            color: Color32::from_rgb(150, 200, 255), // голубой
            ..Default::default()
        };
        let string_fmt = TextFormat {
            font_id: FontId::monospace(14.0),
            color: Color32::from_rgb(255, 200, 120), // оранжевый
            ..Default::default()
        };
        let number_fmt = TextFormat {
            font_id: FontId::monospace(14.0),
            color: Color32::from_rgb(230, 150, 255), // фиолетовый
            ..Default::default()
        };
        let comment_fmt = TextFormat {
            font_id: FontId::monospace(14.0),
            color: Color32::from_gray(150),
            ..Default::default()
        };

        let append_with_search =
            |job: &mut LayoutJob, piece: &str, base: TextFormat| {
                if piece.is_empty() {
                    return;
                }
                if let Some(re) = &search_re {
                    let mut last = 0usize;
                    for m in re.find_iter(piece) {
                        let (s, e) = (m.start(), m.end());
                        if s > last {
                            job.append(&piece[last..s], 0.0, base.clone());
                        }
                        let mut hl = base.clone();
                        hl.background = crate::SEARCH_MATCH_COLOR;
                        job.append(&piece[s..e], 0.0, hl);
                        last = e;
                    }
                    if last < piece.len() {
                        job.append(&piece[last..], 0.0, base);
                    }
                } else {
                    job.append(piece, 0.0, base);
                }
            };

        let mut remaining = text.as_str();

        while !remaining.is_empty() {
            let mut next_match: Option<(usize, usize, TextFormat)> = None;

            for (re, fmt) in [
                (&comment_re, comment_fmt.clone()),
                (&string_re, string_fmt.clone()),
                (&number_re, number_fmt.clone()),
                (&key_re, key_fmt.clone()),
            ] {
                if let Some(m) = re.find(remaining) {
                    if next_match.is_none() || m.start() < next_match.clone().unwrap().0 {
                        next_match = Some((m.start(), m.end(), fmt));
                    }
                }
            }

            if let Some((s, e, fmt)) = next_match {
                if s > 0 {
                    append_with_search(&mut job, &remaining[..s], normal.clone());
                }
                append_with_search(&mut job, &remaining[s..e], fmt);
                remaining = &remaining[e..];
            } else {
                append_with_search(&mut job, remaining, normal.clone());
                break;
            }
        }

        job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(job))
    })
}
