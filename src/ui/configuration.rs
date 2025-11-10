use egui::{Ui};
use crate::config::AppConfig;

pub fn show_configuration(ui: &mut Ui, app_config: &mut AppConfig) {
    let mut config_should_be_saved = false;

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label("AI API URL (be sure that model supports tools):");
            let api_url_box = egui::TextEdit::singleline(&mut app_config.ai_settings.api_url).desired_width(f32::INFINITY);
            if ui.add(api_url_box).changed() {
                config_should_be_saved = true;
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label("AI API KEY:");
            let api_key_box = egui::TextEdit::singleline(&mut app_config.ai_settings.api_key).desired_width(f32::INFINITY);
            if ui.add(api_key_box).changed() {
                config_should_be_saved = true;
            }
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
            app_config.ai_settings.api_url.clone(),
            app_config.ai_settings.api_key.clone(),
        );
    }
}
