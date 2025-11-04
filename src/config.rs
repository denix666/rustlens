use egui::Context;
use serde::{Deserialize, Serialize};
use std::{fs::{self, OpenOptions}, path::PathBuf};
use toml::to_string;
use std::io::Write;

use crate::SortBy;

#[derive(Deserialize, Serialize)]
pub struct AppOptions {
    pub last_window_pos_x: f32,
    pub last_window_pos_y: f32,
    pub last_width: f32,
    pub last_height: f32,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SortPreferences {
    pub nodes_sort_by: SortBy,
    pub nodes_sort_asc: bool,
    pub pods_sort_by: SortBy,
    pub pods_sort_asc: bool,
    pub pvs_sort_by: SortBy,
    pub pvs_sort_asc: bool,
    pub pvcs_sort_by: SortBy,
    pub pvcs_sort_asc: bool,
    pub namespace_sort_by: SortBy,
    pub namespace_sort_asc: bool,
}

#[derive(Deserialize, Serialize)]
pub struct AppConfig {
    pub options: AppOptions,
    pub sort_preferences: SortPreferences,
}

pub fn app_root_path() -> PathBuf {
    let mut app_root_path = match home::home_dir() {
        Some(path) => path,
        None => panic!("Impossible to get your home dir!"),
    };
    app_root_path.push(crate::CONFIG_DIR);
    return app_root_path
}

pub fn write_config_to_file(
    last_window_pos_x: f32,
    last_window_pos_y: f32,
    last_width: f32,
    last_height: f32,
    nodes_sort_by: SortBy,
    nodes_sort_asc: bool,
    pods_sort_by: SortBy,
    pods_sort_asc: bool,
    pvs_sort_by: SortBy,
    pvs_sort_asc: bool,
    pvcs_sort_by: SortBy,
    pvcs_sort_asc: bool,
    namespace_sort_by: SortBy,
    namespace_sort_asc: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config_file_path = app_root_path();
    config_file_path.push(crate::MAIN_CONFIG_FILE_NAME);

    if let Some(parent) = config_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let app_config = AppConfig {
        options: AppOptions {
            last_window_pos_x,
            last_window_pos_y,
            last_height,
            last_width,
        },
        sort_preferences: SortPreferences {
            nodes_sort_by,
            nodes_sort_asc,
            pods_sort_by,
            pods_sort_asc,
            pvs_sort_by,
            pvs_sort_asc,
            pvcs_sort_by,
            pvcs_sort_asc,
            namespace_sort_by,
            namespace_sort_asc,
        }
    };

    let toml_string = toml::to_string(&app_config)?;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&config_file_path)?;

    file.write_all(toml_string.as_bytes())?;
    file.flush()?;

    Ok(())
}

pub fn read_app_config_from_file() -> AppConfig {
    let mut config_file_path = app_root_path();
    config_file_path.push(crate::MAIN_CONFIG_FILE_NAME);

    let new_config = AppConfig {
        // Default configuation
        options: AppOptions {
            last_window_pos_x: 20.0,
            last_window_pos_y: 10.0,
            last_width: 1600.0,
            last_height: 800.0,
        },
        sort_preferences: SortPreferences {
            nodes_sort_by: SortBy::Name,
            nodes_sort_asc: true,
            pods_sort_by: SortBy::Age,
            pods_sort_asc: false,
            pvs_sort_by: SortBy::Age,
            pvs_sort_asc: false,
            pvcs_sort_by: SortBy::Age,
            pvcs_sort_asc: false,
            namespace_sort_by: SortBy::Name,
            namespace_sort_asc: false,
        },
    };

    let toml_str = match std::fs::read_to_string(config_file_path) {
        Ok(res) => res,
        Err(_) => {
            write_config_to_file(
                20.0,
                10.0,
                1600.0,
                800.0,
                SortBy::Name,
                true,
                SortBy::Age,
                false,
                SortBy::Age,
                false,
                SortBy::Age,
                false,
                SortBy::Name,
                true,
            ).unwrap();
            to_string(&new_config).unwrap()
        }
    };

    // Try to use loaded config. In case of error - create new (backward compatibility)
    let app_config = match toml::from_str(&toml_str) {
        Ok(res) => res,
        Err(_) => {
            write_config_to_file(
                20.0,
                10.0,
                1600.0,
                800.0,
                SortBy::Name,
                true,
                SortBy::Age,
                false,
                SortBy::Age,
                false,
                SortBy::Age,
                false,
                SortBy::Name,
                true,
            ).unwrap();
            new_config
        }
    };

    app_config
}

pub fn window_moved_or_resized(ctx: &Context, app_config: &mut AppConfig) -> bool {
    let mut changed = false;

    let size_x = ctx.viewport_rect().width();
    let size_y = ctx.viewport_rect().height();

    ctx.input(|i| {
        if let Some(rect) = i.viewport().outer_rect {
            let pos_x = rect.min.x;
            let pos_y = rect.min.y;

            if app_config.options.last_window_pos_x != pos_x {
                app_config.options.last_window_pos_x = pos_x;
                changed = true;
            }

            if app_config.options.last_window_pos_y != pos_y {
                app_config.options.last_window_pos_y = pos_y;
                changed = true;
            }

            if app_config.options.last_width != size_x {
                app_config.options.last_width = size_x;
                changed = true;
            }

            if app_config.options.last_height != size_y {
                app_config.options.last_height = size_y;
                changed = true;
            }
        }
    });

    return changed
}
