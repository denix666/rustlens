use egui::Context;
use serde::{Deserialize, Serialize};
use std::{fs::{self, OpenOptions}, path::PathBuf};
use toml::to_string;
use std::io::Write;

#[derive(Deserialize, Serialize)]
pub struct AppOptions {
    pub last_window_pos_x: f32,
    pub last_window_pos_y: f32,
    pub last_width: f32,
    pub last_height: f32,
}

#[derive(Deserialize, Serialize)]
pub struct AppConfig {
    pub options: AppOptions,
}

fn app_root_path() -> PathBuf {
    let mut app_root_path = match home::home_dir() {
        Some(path) => path,
        None => panic!("Impossible to get your home dir!"),
    };
    app_root_path.push(crate::CONFIG_DIR);
    return app_root_path
}

pub fn write_config_to_file(last_window_pos_x: f32, last_window_pos_y: f32, last_width: f32, last_height: f32) -> Result<(), Box<dyn std::error::Error>> {
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
        }
    };

    let toml_str = match std::fs::read_to_string(config_file_path) {
        Ok(res) => res,
        Err(_) => {
            write_config_to_file(20.0, 10.0, 1600.0, 800.0).unwrap();
            to_string(&new_config).unwrap()
        }
    };

    let app_config = toml::from_str(&toml_str).expect("Failed to load configuration file...");

    app_config
}

pub fn window_moved_or_resized(ctx: &Context, app_config: &mut AppConfig) -> bool {
    let mut changed = false;

    let size_x = ctx.screen_rect().width();
    let size_y = ctx.screen_rect().height();

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
