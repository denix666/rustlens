use egui::{Context, Key, ScrollArea, TextStyle};
use kube::Client;
use regex::Regex;
use std::{collections::HashMap, fs, sync::{Arc, Mutex}, time::{Duration, Instant}};
use rfd::FileDialog;

use crate::ui::LogParserWindow;

pub struct LogWindow {
    pub pod_name: String,
    pub containers: Vec<crate::ContainerStatusItem>,
    pub show: bool,
    pub namespace: String,
    pub selected_container: String,
    pub buffer: Arc<Mutex<String>>,
    pub last_container: Option<String>,
    pub export_message: Option<(String, Instant)>,
    pub show_previous_logs: bool,
}

impl LogWindow {
    pub fn new() -> Self {
        Self {
            pod_name: String::new(),
            containers: Vec::new(),
            selected_container: String::new(),
            namespace: String::new(),
            show: false,
            buffer: Arc::new(Mutex::new(String::new())),
            last_container: None,
            export_message: None,
            show_previous_logs: false,
        }
    }
}

pub fn show_log_window(ctx: &Context, log_window: &mut LogWindow, log_parser_window: &mut LogParserWindow, client: Arc<Client>) {
    let response = egui::Window::new("Logs").collapsible(false).resizable(true).open(&mut log_window.show).auto_sized().max_height(500.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Container:");
            egui::ComboBox::from_id_salt("containers_combo").selected_text(&log_window.selected_container).width(150.0).show_ui(ui, |ui| {
                for container in &log_window.containers {
                    ui.selectable_value(
                        &mut log_window.selected_container,
                        container.name.clone(),
                        &container.name,
                    );
                }
            });
            ui.separator();
            if ui.checkbox(&mut log_window.show_previous_logs, "Logs from previous stopped container").changed() {
                log_window.last_container = Some(log_window.selected_container.clone());
                log_window.buffer = Arc::new(Mutex::new(String::new()));

                let buf_clone = Arc::clone(&log_window.buffer);
                let cur_ns = log_window.namespace.clone();
                let cur_pod = log_window.pod_name.clone();
                let cur_container = log_window.selected_container.clone();
                let prev_logs = log_window.show_previous_logs.clone();
                let client_clone = Arc::clone(&client);

                tokio::spawn(async move {
                    crate::fetch_logs(client_clone,
                    &cur_ns,
                        &cur_pod,
                    &cur_container, buf_clone, prev_logs).await;
                });
            }
            ui.separator();
            if ui.button("💾 Export to file").clicked() {
                if let Ok(logs) = log_window.buffer.lock() {
                    if let Some(path) = FileDialog::new()
                        .set_file_name(format!(
                            "{}_{}_{}.log",
                            log_window.namespace,
                            log_window.pod_name,
                            log_window.selected_container
                        ))
                        .save_file()
                    {
                        match fs::write(&path, logs.as_bytes()) {
                            Ok(_) => {
                                log_window.export_message =
                                    Some((format!("✅ Exported to {:?}", path), Instant::now()));
                            }
                            Err(e) => {
                                log_window.export_message =
                                    Some((format!("❌ Failed to export: {}", e), Instant::now()));
                            }
                        }
                    }
                }
            }
            ui.separator();
            if ui.button(egui::RichText::new("🔎 Log parser").size(16.0).color(egui::Color32::LIGHT_GREEN)).clicked() {
                match crate::ui::log_parser::load_plugins() {
                    Ok(plugins) => {
                        if plugins.is_empty() {
                            log::warn!("{}: plugins not found in ~/.local/share/rustlens/plugins", "Warning");
                        } else {
                            let mut compiled: Vec<(String, super::RuleSpec, Regex)> = Vec::new();
                            for (pname, plugin) in &plugins {
                                for rule in &plugin.rules {
                                    for pat in &rule.patterns {
                                        let re_result = anyhow::Context::with_context(Regex::new(pat), || {
                                            format!("Error in regexp '{}' in plugin '{}'", rule.id, pname)
                                        });
                                        match re_result {
                                            Ok(re) => {
                                                compiled.push((pname.clone(), rule.clone(), re));
                                            }
                                            Err(e) => {
                                                log::error!("Failed to compile regex from plugin '{}' (rule: {}): {:?}", pname, rule.id, e);
                                            }
                                        }
                                    }
                                }
                            }

                            let mut stats: HashMap<(String, String), crate::ui::log_parser::RuleStats> = HashMap::new();

                            if let Ok(log_buffer) = log_window.buffer.lock() {
                                let lines: Vec<_> = log_buffer.lines().collect();
                                for (line_idx, line) in lines.into_iter().enumerate().rev() {
                                    for (pname, rule, re) in &compiled {
                                        if re.is_match(&line) {
                                            let key = (pname.clone(), rule.id.clone());
                                            let entry = stats.entry(key.clone()).or_insert_with(|| crate::ui::log_parser::RuleStats {
                                                plugin: pname.clone(),
                                                id: rule.id.clone(),
                                                title: rule.title.clone(),
                                                level: rule.level.clone(),
                                                matches: 0,
                                                examples: Vec::new(),
                                                message: rule.message.clone(),
                                                recommendation: rule.recommendation.clone(),
                                            });
                                            entry.matches += 1;

                                            if entry.examples.len() < rule.context_lines.unwrap_or(3) {
                                                entry.examples.push(format!("{}: {}", line_idx + 1, line));
                                            }
                                        }
                                    }
                                }

                                let filtered: Vec<crate::ui::log_parser::RuleStats> = stats.into_values().filter(|s| {
                                    let plugin = plugins.get(&s.plugin).unwrap();
                                    let rule = plugin.rules.iter().find(|r| r.id == s.id).unwrap();
                                    if let Some(th) = rule.threshold {
                                        s.matches >= th as u64
                                    } else {
                                        true
                                    }
                                }).collect();

                                log_parser_window.filtered = filtered;
                                log_parser_window.show = true;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to load plugins: {:?}", e);
                    }
                }
            }
        });

        if let Some((msg, when)) = &log_window.export_message {
            ui.colored_label(
                if msg.starts_with('✅') {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::RED
                },
                msg,
            );

            if when.elapsed() > Duration::from_secs(3) {
                log_window.export_message = None;
            }
        }

        ui.separator();
        if log_window.last_container.as_ref() != Some(&log_window.selected_container) {
            log_window.last_container = Some(log_window.selected_container.clone());
            let buf_clone = Arc::clone(&log_window.buffer);
            let cur_ns = log_window.namespace.clone();
            let cur_pod = log_window.pod_name.clone();
            let cur_container = log_window.selected_container.clone();
            let prev_logs = log_window.show_previous_logs.clone();
            let client_clone = Arc::clone(&client);
            tokio::spawn(async move {
                crate::fetch_logs(client_clone,
                &cur_ns,
                    &cur_pod,
                &cur_container, buf_clone, prev_logs).await;
            });
        }

        ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false; 2]).show(ui, |ui| {
            if let Ok(mut logs) = log_window.buffer.lock() {
                let line_count = logs.lines().count();
                let rows = line_count.min(crate::MAX_LOG_LINES);

                if line_count == 0 {
                    *logs = "log not found...".to_string();
                }

                ui.add(egui::TextEdit::multiline(&mut logs.clone())
                    .font(TextStyle::Monospace)
                    .desired_rows(rows)
                    .desired_width(f32::INFINITY)
                    .code_editor()
                    .cursor_at_end(true)
                );
            }
        });
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            log_window.show = false;
        }
    }
}
