use egui::Ui;
use crate::{config::AppConfig, ui::ALL_AI_PROVIDERS};

pub fn show_configuration(ui: &mut Ui, app_config: &mut AppConfig) {
    let mut config_should_be_saved = false;

    ui.add_space(10.0);

    ui.vertical(|ui| {
        ui.heading("AI Consultant:");
        ui.add_space(10.0);

        egui::Frame::group(ui.style()).fill(crate::theme::SETTINGS_FRAME_COLOR).stroke(ui.visuals().widgets.noninteractive.bg_stroke).corner_radius(egui::CornerRadius::same(8)).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            ui.set_row_height(24.0);
            ui.label("Select AI provider:");

            egui::ComboBox::from_id_salt("provider_combo").selected_text(app_config.ai_settings.selected_ai_provider.to_string()).width(150.0).show_ui(ui, |ui| {
                for provider in ALL_AI_PROVIDERS {
                    let response = ui.selectable_value(
                        &mut app_config.ai_settings.selected_ai_provider,
                        provider,
                        provider.to_string()
                    );

                    if response.changed() {
                        config_should_be_saved = true;
                    }
                }
            });
        });

        ui.add_space(20.0);

        egui::Frame::group(ui.style())
            .fill(crate::theme::SETTINGS_FRAME_COLOR)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                ui.heading("Gemini:");
                egui::Grid::new("ai_settings_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.set_row_height(24.0);
                        ui.label("Gemini API URL (model must support tools):");
                        let gemini_url_res = ui.add_sized([ui.available_width(), 24.0],
                            egui::TextEdit::singleline(&mut app_config.ai_settings.gemini_api_url)
                        );
                        if gemini_url_res.changed() {
                            config_should_be_saved = true;
                        }
                        ui.end_row();

                        ui.set_row_height(24.0);
                        ui.label("Gemini API KEY:");
                        let gemini_key_res = ui.add_sized([ui.available_width(), 24.0],
                            egui::TextEdit::singleline(&mut app_config.ai_settings.gemini_api_key).password(true)
                        );
                        if gemini_key_res.changed() {
                            config_should_be_saved = true;
                        }
                        ui.end_row();

                        ui.set_row_height(24.0);
                        ui.label("Gemini mcp server binary path:");
                        ui.add_sized([ui.available_width(), 24.0],
                            egui::TextEdit::singleline(&mut app_config.ai_settings.gemini_mcp_path).interactive(false)
                        );
                        ui.end_row();

                        let gemini_mcp_path_res = ui.add_sized([80.0, 24.0],
                            egui::Button::new("Search")
                        );
                        if gemini_mcp_path_res.clicked() {
                            if let Some(path) = rfd::FileDialog::new().set_directory("~/").pick_file() {
                                app_config.ai_settings.gemini_mcp_path = path.to_string_lossy().to_string();
                                config_should_be_saved = true;
                            }
                        }
                        ui.end_row();
                    });
            });
    });

    if config_should_be_saved {
        let _ = crate::config::write_config_to_file(
            app_config.options.last_window_pos_x,
            app_config.options.last_window_pos_y,
            app_config.options.last_width,
            app_config.options.last_height,
            app_config.sort_preferences.nodes_sort_by,
            app_config.sort_preferences.nodes_sort_asc,
            app_config.sort_preferences.pods_sort_by,
            app_config.sort_preferences.pods_sort_asc,
            app_config.sort_preferences.pvs_sort_by,
            app_config.sort_preferences.pvs_sort_asc,
            app_config.sort_preferences.pvcs_sort_by,
            app_config.sort_preferences.pvcs_sort_asc,
            app_config.sort_preferences.namespace_sort_by,
            app_config.sort_preferences.namespace_sort_asc,
            app_config.ai_settings.selected_ai_provider.clone(),
            app_config.ai_settings.gemini_api_url.clone(),
            app_config.ai_settings.gemini_api_key.clone(),
            app_config.ai_settings.gemini_mcp_path.clone(),
        );
    }
}
