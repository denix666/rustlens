use egui::{text::LayoutJob, Color32, Context, FontId, TextFormat, TextStyle, Ui};
use kube::Client;
use std::sync::Arc;
use regex::RegexBuilder;

pub struct YamlEditorWindow {
    pub content: String,
    pub show: bool,
    pub search_query: String,
}

impl YamlEditorWindow {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            show: false,
            search_query: String::new(),
        }
    }
}

pub fn show_yaml_editor(ctx: &Context, editor: &mut YamlEditorWindow, client: Arc<Client>) {
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
        });
        ui.separator();

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
            if ui.button(egui::RichText::new("✅ Save").size(16.0).color(egui::Color32::GREEN)).clicked() {
                let content = editor.content.clone();

                match serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    Ok(_) => {
                        // YAML is valid!
                        let client_clone = Arc::clone(&client);
                        tokio::spawn(async move {
                            if let Err(e) = crate::patch_resource(client_clone, content.as_str()).await {
                                println!("Error applying YAML: {:?}", e);
                            }
                        });
                        editor.show = false;
                    }
                    Err(e) => {
                        eprintln!("YAML Error: {}", e);
                    }
                }
            }
            if ui.button(egui::RichText::new("🗙 Cancel").size(16.0).color(egui::Color32::RED)).clicked() {
                editor.show = false;
            }
        });
    });
}


pub fn make_yaml_layouter(search: String) -> Box<dyn for<'a, 'b> FnMut(&'a Ui, &'b dyn egui::TextBuffer, f32) -> std::sync::Arc<egui::Galley>> {
    let key_re = regex::Regex::new(r"^(\s*)([^:\n#]+):\s*(.*)$").unwrap();
    let comment_re = regex::Regex::new(r"#.*$").unwrap();

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
            color: Color32::from_rgb(150, 200, 255), // light blue
            ..Default::default()
        };

        let string_fmt = TextFormat {
            font_id: FontId::monospace(14.0),
            color: Color32::from_rgb(255, 200, 120), // orange-ish
            ..Default::default()
        };

        let number_fmt = TextFormat {
            font_id: FontId::monospace(14.0),
            color: Color32::from_rgb(230, 150, 255), // magenta-ish
            ..Default::default()
        };

        let comment_fmt = TextFormat {
            font_id: FontId::monospace(14.0),
            color: Color32::from_gray(150),
            ..Default::default()
        };

        let append_with_search = |job: &mut LayoutJob, piece: &str, base: TextFormat| {
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
                    hl.background = Color32::from_rgb(160, 220, 67);
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

        let text_str = text.as_str();
        for line in text_str.lines() {
            if line.trim().is_empty() {
                job.append("\n", 0.0, normal.clone());
                continue;
            }

            if let Some(cm) = comment_re.find(line) {
                let before = &line[..cm.start()];
                let comment = &line[cm.start()..];

                if let Some(cap) = key_re.captures(before) {
                    let indent = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                    let key = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                    let val = cap.get(3).map(|m| m.as_str()).unwrap_or("");

                    append_with_search(&mut job, indent, normal.clone());
                    append_with_search(&mut job, key, key_fmt.clone());
                    append_with_search(&mut job, ": ", normal.clone());

                    if val.starts_with('"') || val.starts_with('\'') {
                        append_with_search(&mut job, val, string_fmt.clone());
                    } else if val.chars().next().map(|c| c.is_digit(10)).unwrap_or(false) {
                        append_with_search(&mut job, val, number_fmt.clone());
                    } else {
                        append_with_search(&mut job, val, normal.clone());
                    }
                } else {
                    append_with_search(&mut job, before, normal.clone());
                }

                append_with_search(&mut job, comment, comment_fmt.clone());
                job.append("\n", 0.0, normal.clone());
                continue;
            }

            if let Some(cap) = key_re.captures(line) {
                let indent = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let key = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                let val = cap.get(3).map(|m| m.as_str()).unwrap_or("");

                append_with_search(&mut job, indent, normal.clone());
                append_with_search(&mut job, key, key_fmt.clone());
                append_with_search(&mut job, ": ", normal.clone());

                if val.starts_with('"') || val.starts_with('\'') {
                    append_with_search(&mut job, val, string_fmt.clone());
                } else if val.chars().next().map(|c| c.is_digit(10)).unwrap_or(false) {
                    append_with_search(&mut job, val, number_fmt.clone());
                } else {
                    append_with_search(&mut job, val, normal.clone());
                }
                job.append("\n", 0.0, normal.clone());
            } else {
                append_with_search(&mut job, line, normal.clone());
                job.append("\n", 0.0, normal.clone());
            }
        }

        job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(job))
    })
}
