use std::sync::Arc;
use egui::{Context, TextStyle};
use kube::Client;

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
    egui::Window::new("Edit resource").max_width(1200.0).max_height(600.0).collapsible(false).resizable(true).show(ctx, |ui| {
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

        let search = editor.search_query.clone();
        egui::ScrollArea::vertical().hscroll(true).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut editor.content)
                .font(TextStyle::Monospace)
                .code_editor()
                .layouter(&mut crate::search_layouter(search)),
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
