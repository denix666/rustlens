use std::{collections::HashMap, fs::File};
use anyhow::{Context, anyhow};
use serde::Deserialize;
use walkdir::WalkDir;
use egui::{Color32, Key};

use crate::{functions::item_color, theme::*};

pub struct LogParserWindow {
    pub show: bool,
    pub filtered: Vec<RuleStats>,
}

impl LogParserWindow {
    pub fn new() -> Self {
        Self {
            show: false,
            filtered: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Plugin {
    pub name: String,
    //pub description: Option<String>,
    pub rules: Vec<RuleSpec>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuleSpec {
    pub id: String,
    pub title: Option<String>,
    pub patterns: Vec<String>,
    pub level: Option<String>,
    pub message: Option<String>,
    pub recommendation: Option<String>,
    pub threshold: Option<u32>,
    pub context_lines: Option<usize>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct RuleStats {
    pub plugin: String,
    pub id: String,
    pub title: Option<String>,
    pub level: Option<String>,
    pub matches: u64,
    pub examples: Vec<String>,
    pub message: Option<String>,
    pub recommendation: Option<String>,
}

pub fn load_plugins() -> anyhow::Result<HashMap<String, Plugin>> {
    let mut plugins_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;

    plugins_dir.push(".local");
    plugins_dir.push("share");
    plugins_dir.push("rustlens");
    plugins_dir.push("plugins");

    let mut map = HashMap::new();

    if !plugins_dir.exists() {
        return Ok(map);
    }

    for entry in WalkDir::new(plugins_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        match path.extension().and_then(|s| s.to_str()) {
            Some("yaml") | Some("yml") => {
                let f = File::open(path)
                    .with_context(|| format!("opening plugin file {}", path.display()))?;
                let plugin: Plugin = serde_yaml::from_reader(f)
                    .with_context(|| format!("parsing YAML {}", path.display()))?;
                map.insert(plugin.name.clone(), plugin);
            }
            _ => {
                // ignore all other files
            }
        }
    }

    Ok(map)
}

pub fn show_log_parser_window(ctx: &egui::Context, window: &mut LogParserWindow) {
    let response = egui::Window::new("Log parser and recomendations")
        .collapsible(false)
        .resizable(true)
        .open(&mut window.show)
        .min_width(850.0)
        .max_height(600.0)
        .show(ctx, |ui|
    {
        if window.filtered.len() == 0 {
            ui.label(egui::RichText::new("😎 Log parser didn't found anything in the log").color(Color32::GREEN));
        } else {
            egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false; 2]).show(ui, |ui| {
                egui::Grid::new("log_parser_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                    let mut grid_idx = 0;
                    for i in &window.filtered {
                        grid_idx += 1;

                        if let Some(lvl) = &i.level {
                            ui.label(egui::RichText::new("Level:").color(ROW_NAME_COLOR));
                            ui.label(egui::RichText::new(lvl).color(item_color(lvl)));
                            ui.end_row();
                        }

                        ui.label(egui::RichText::new("Occured:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(&i.matches.to_string()).color(DETAIL_COLOR));
                        ui.end_row();

                        if let Some(msg) = &i.message {
                            ui.label(egui::RichText::new("Desctiption:").color(ROW_NAME_COLOR));
                            ui.label(egui::RichText::new(msg).color(DETAIL_COLOR));
                            ui.end_row();
                        }

                        if i.examples.len() > 0 {
                            ui.separator(); ui.separator(); ui.end_row();
                            ui.label(egui::RichText::new("Examples:").color(ROW_NAME_COLOR));
                            let grid_id = format!("log_parser_examples_grid_{}", grid_idx);
                            egui::Grid::new(grid_id).striped(true).min_col_width(20.0).show(ui, |ui| {
                                let examples = &i.examples;
                                for ex in examples.iter().rev() {
                                    ui.scope(|ui| {
                                        ui.set_max_width(800.0);
                                        ui.add(egui::Label::new(egui::RichText::new(ex).color(SEARCH_MATCH_COLOR)).wrap());
                                    });
                                    ui.end_row();
                                }
                            });
                            ui.end_row();
                        }

                        if let Some(rec) = &i.recommendation {
                            ui.label(egui::RichText::new("Reccomendations:").color(ROW_NAME_COLOR));
                            ui.label(egui::RichText::new(rec).color(GREEN_BUTTON));
                            ui.end_row();
                        }

                        ui.separator(); ui.separator(); ui.end_row();
                    }
                });
            });
        }
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            window.show = false;
        }
    }
}
