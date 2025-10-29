use std::sync::{Arc, Mutex};
use egui::{Context, Key};
use crate::{functions::item_color, ui::{LogWindow, YamlEditorWindow}, theme::*};

pub struct PodDetailsWindow {
    pub show: bool,
}

impl PodDetailsWindow {
    pub fn new() -> Self {
        Self {
            show: false,
        }
    }
}

pub fn show_pod_details_window(
        ctx: &Context,
        pod_details_window: &mut PodDetailsWindow,
        details: Arc<Mutex<crate::PodDetails>>,
        pods: Arc<Mutex<Vec<crate::PodItem>>>,
        log_window: Arc<Mutex<LogWindow>>,
        yaml_editor_window: Arc<Mutex<YamlEditorWindow>>,
        client: Arc<crate::Client>,
        delete_confirm: &mut super::DeleteConfirmation,
) {
    let guard_details = details.lock().unwrap(); // More detailed info
    let guard_pods = pods.lock().unwrap(); // Pods with base details already we have

    if guard_details.name.is_none() {
        return;
    }

    let pod_item = guard_pods.iter().find(|item| item.name == guard_details.name.clone().unwrap() && item.namespace == guard_details.namespace.clone());
    if pod_item.is_none() {
        return;
    }
    let cur_ns = &pod_item.unwrap().namespace;


    let response = egui::Window::new("Pod details").min_width(800.0).collapsible(false).resizable(true).open(&mut pod_details_window.show).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("📃 Logs").size(16.0).color(crate::GRAY_BUTTON)).clicked() {
                let name = guard_details.name.clone().unwrap();
                let ns = cur_ns.clone();

                crate::open_logs_for_pod(
                    name,
                    ns.to_owned().unwrap(),
                    pod_item.unwrap().containers.clone(),
                    Arc::clone(&log_window),
                    Arc::clone(&client),
                );
            }

            if ui.button(egui::RichText::new("✏ Edit").size(16.0).color(crate::GREEN_BUTTON)).clicked() {
                let name = guard_details.name.clone().unwrap();
                let ns = cur_ns.clone();

                crate::edit_yaml_for::<k8s_openapi::api::core::v1::Pod>(
                    name.clone(),
                    ns.to_owned().unwrap(),
                    Arc::clone(&yaml_editor_window),
                    Arc::clone(&client),
                );
            }

            if ui.button(egui::RichText::new("🗑 Delete").size(16.0).color(crate::RED_BUTTON)).clicked() {
                let name = guard_details.name.clone().unwrap();
                let ns = cur_ns.clone();

                delete_confirm.request(name.clone(), Some("dd".to_string()), move || {
                    tokio::spawn(async move {
                        if let Err(err) = crate::delete_pod(Arc::clone(&client), name.clone(), ns.as_deref(), true).await {
                            eprintln!("Failed to delete secret: {}", err);
                        }
                    });
                });
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
            egui::Grid::new("pod_details_grid").striped(true).min_col_width(20.0).show(ui, |ui| {

                ui.label(egui::RichText::new("Pod name:").color(ROW_NAME_COLOR));
                ui.label(egui::RichText::new(guard_details.name.clone().unwrap()).color(DETAIL_COLOR));
                ui.end_row();

                if let Some(item) = pod_item {
                    if let Some(creation_timestamp) = &item.creation_timestamp {
                        ui.label(egui::RichText::new("Creation time:").color(ROW_NAME_COLOR));
                        let timestamp_text = format!("{}, {} ago", creation_timestamp.0, crate::format_age(creation_timestamp));
                        ui.label(egui::RichText::new(timestamp_text).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if let Some(namespace) = &item.namespace {
                        ui.label(egui::RichText::new("Namespace:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(namespace).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if let Some(node_name) = &item.node_name {
                        ui.label(egui::RichText::new("Running on:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(node_name).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    if let Some(phase) = &item.phase {
                        ui.label(egui::RichText::new("Status:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(phase).color(item_color(phase)));
                        ui.end_row();
                    }

                    if let Some(qos_class) = &item.qos_class {
                        ui.label(egui::RichText::new("QoS:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(qos_class).color(item_color(qos_class)));
                        ui.end_row();
                    }

                    if let Some(controller) = &item.controller {
                        ui.label(egui::RichText::new("Controlled by:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(controller).color(DETAIL_COLOR));
                        ui.end_row();
                    }

                    ui.label(egui::RichText::new("Restarts count:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(&item.restart_count.to_string()).color(WARNING_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.uid.clone() {
                    ui.label(egui::RichText::new("UID:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.service_account.clone() {
                    ui.label(egui::RichText::new("Service account:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.pod_ip.clone() {
                    ui.label(egui::RichText::new("Pod IP:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(i) = guard_details.host_ip.clone() {
                    ui.label(egui::RichText::new("Host IP:").color(ROW_NAME_COLOR));
                    ui.label(egui::RichText::new(i).color(DETAIL_COLOR));
                    ui.end_row();
                }

                if let Some(labels) = guard_details.labels.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Labels:").color(ROW_NAME_COLOR));
                    egui::Grid::new("pod_details_labels_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for (j, y) in labels.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(annotations) = guard_details.annotations.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Annotations:").color(ROW_NAME_COLOR));
                    egui::Grid::new("pod_details_annotations_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                        for (j, y) in annotations.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if guard_details.tolerations.len() > 0 {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Tolerations:").color(ROW_NAME_COLOR));
                    egui::Grid::new("pod_details_tolerations_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        ui.label(egui::RichText::new("Key").color(TOLERATIONS_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Operator").color(TOLERATIONS_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Value").color(TOLERATIONS_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Effect").color(TOLERATIONS_HEAD_GRID_COLOR));
                        ui.label(egui::RichText::new("Seconds").color(TOLERATIONS_HEAD_GRID_COLOR));
                        ui.end_row();
                        for j in guard_details.tolerations.iter() {
                            if let Some(item) = &j.key {
                                ui.label(egui::RichText::new(item).color(TOLERATION_NAME_COLUMN_COLOR));
                            } else {
                                ui.label("");
                            }
                            if let Some(item) = &j.operator {
                                ui.label(egui::RichText::new(item).color(TOLERATION_NAME_COLUMN_COLOR));
                            } else {
                                ui.label("");
                            }
                            if let Some(item) = &j.value {
                                ui.label(egui::RichText::new(item).color(TOLERATION_NAME_COLUMN_COLOR));
                            } else {
                                ui.label("");
                            }
                            if let Some(item) = &j.effect {
                                ui.label(egui::RichText::new(item).color(TOLERATION_NAME_COLUMN_COLOR));
                            } else {
                                ui.label("");
                            }
                            if let Some(item) = &j.toleration_seconds {
                                ui.label(egui::RichText::new(item.to_string()).color(TOLERATION_NAME_COLUMN_COLOR));
                            } else {
                                ui.label("");
                            }
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(node_selector) = guard_details.node_selector.clone() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Node selector:").color(ROW_NAME_COLOR));
                    egui::Grid::new("pod_details_node_selector_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                        for (j, y) in node_selector.iter() {
                            ui.label(egui::RichText::new(j).color(DETAIL_COLOR));
                            ui.label(egui::RichText::new(y).color(SECOND_DETAIL_COLOR));
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if !guard_details.conditions.is_empty() {
                    ui.separator(); ui.separator(); ui.end_row();
                    ui.label(egui::RichText::new("Conditions:").color(ROW_NAME_COLOR));
                    egui::Grid::new("pod_details_conditions_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                        for item in guard_details.conditions.iter() {
                            let cond_type = item.type_.clone();
                            let status = item.status.clone();
                            let reason = item.reason.clone().unwrap_or_default();
                            let message = item.message.clone().unwrap_or_default();

                            ui.label(egui::RichText::new(&cond_type).color(DETAIL_COLOR));

                            if !reason.is_empty() || !message.is_empty() {
                                let lbl_text = format!("{status} ({reason} - {message})");
                                ui.label(egui::RichText::new(lbl_text).color(DETAIL_COLOR));
                            } else {
                                ui.label(egui::RichText::new(status).color(DETAIL_COLOR));
                            }

                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            });

            ui.separator();
            ui.heading("Containers:");
            egui::Grid::new("containers_grid").striped(true).min_col_width(20.0).show(ui, |ui| {
                if !guard_details.containers.is_empty() {
                    for container in &guard_details.containers {
                        ui.separator(); ui.separator(); ui.end_row();
                        ui.label(egui::RichText::new("■").size(16.0).color(item_color(container.state.as_deref().unwrap_or_default())));
                        ui.label(egui::RichText::new(&container.name).color(SECOND_DETAIL_COLOR));
                        ui.end_row();

                        ui.label(egui::RichText::new("Status:").color(ROW_NAME_COLOR));
                        let status = container.state.as_deref().unwrap_or("");
                        let message = container.message.as_deref().unwrap_or("");
                        let label_text = format!("{} {}", status, message);
                        let label = egui::widgets::Label::new(egui::RichText::new(&label_text).color(DETAIL_COLOR)).wrap();
                        ui.add(label);
                        ui.end_row();

                        if let Some(res) = &container.last_state {
                            ui.label(egui::RichText::new("Last status:").color(ROW_NAME_COLOR));
                            let label_text = format!("{}", res);
                            let label = egui::widgets::Label::new(egui::RichText::new(&label_text).color(WARNING_COLOR)).wrap();
                            ui.add(label);
                            ui.end_row();
                        }

                        if let Some(res) = &container.reason {
                            ui.label(egui::RichText::new("Last reason:").color(ROW_NAME_COLOR));
                            let label_text = format!("{}", res);
                            let label = egui::widgets::Label::new(egui::RichText::new(&label_text).color(WARNING_COLOR)).wrap();
                            ui.add(label);
                            ui.end_row();
                        }

                        if let Some(res) = &container.stop_signal {
                            ui.label(egui::RichText::new("Last stop signal:").color(ROW_NAME_COLOR));
                            let label_text = format!("{}", res);
                            let label = egui::widgets::Label::new(egui::RichText::new(&label_text).color(WARNING_COLOR)).wrap();
                            ui.add(label);
                            ui.end_row();
                        }

                        if let Some(last_exit_code) = &container.last_exit_code {
                            let (exit_code_text, exit_code_color) = match last_exit_code {
                                0 => (format!("{} - Succes", last_exit_code.to_string()), NORMAL_COLOR),
                                1 => (format!("{} - Command line error", last_exit_code.to_string()), ERROR_COLOR),
                                2 => (format!("{} - Misuse of Shell Builtins", last_exit_code.to_string()), ERROR_COLOR),
                                124 => (format!("{} - Timeout", last_exit_code.to_string()), ERROR_COLOR),
                                125 => (format!("{} - The docker run command did not execute successfully", last_exit_code.to_string()), ERROR_COLOR),
                                126 => (format!("{} - A command specified in the image specification could not be invoked", last_exit_code.to_string()), ERROR_COLOR),
                                127 => (format!("{} - Command Not Found", last_exit_code.to_string()), ERROR_COLOR),
                                128 => (format!("{} - Invalid argument", last_exit_code.to_string()), ERROR_COLOR),
                                134 => (format!("{} - Abnormal termination", last_exit_code.to_string()), ERROR_COLOR),
                                137 => (format!("{} - Killed by OOM", last_exit_code.to_string()), ERROR_COLOR),
                                139 => (format!("{} - Segmentation Fault", last_exit_code.to_string()), ERROR_COLOR),
                                143 => (format!("{} - Terminated by Signal", last_exit_code.to_string()), WARNING_COLOR),
                                _ => (format!("{} - Unknown exit code", last_exit_code.to_string()), WARNING_COLOR),
                            };

                            ui.label(egui::RichText::new("Last Exit code:").color(ROW_NAME_COLOR));
                            let label = egui::widgets::Label::new(egui::RichText::new(&exit_code_text).color(exit_code_color)).wrap();
                            ui.add(label);
                            ui.end_row();
                        }

                        ui.label(egui::RichText::new("Image:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(container.image.as_deref().unwrap_or_else(|| "Undefined")).color(DETAIL_COLOR));
                        ui.end_row();

                        ui.label(egui::RichText::new("Image pull policy:").color(ROW_NAME_COLOR));
                        ui.label(egui::RichText::new(container.image_pull_policy.as_deref().unwrap_or_else(|| "Undefined")).color(DETAIL_COLOR));
                        ui.end_row();

                        if let Some(item) = &container.mem_request {
                            ui.label(egui::RichText::new("Memory request:").color(ROW_NAME_COLOR));
                            ui.label(egui::RichText::new(item).color(DETAIL_COLOR));
                            ui.end_row();
                        }

                        if let Some(item) = &container.mem_limit {
                            ui.label(egui::RichText::new("Memory limit:").color(ROW_NAME_COLOR));
                            ui.label(egui::RichText::new(item).color(DETAIL_COLOR));
                            ui.end_row();
                        }

                        if let Some(item) = &container.cpu_request {
                            ui.label(egui::RichText::new("CPU request:").color(ROW_NAME_COLOR));
                            ui.label(egui::RichText::new(item).color(DETAIL_COLOR));
                            ui.end_row();
                        }

                        if let Some(item) = &container.cpu_limit {
                            ui.label(egui::RichText::new("CPU limit:").color(ROW_NAME_COLOR));
                            ui.label(egui::RichText::new(item).color(DETAIL_COLOR));
                            ui.end_row();
                        }

                        if !container.mounts.is_empty() {
                            ui.label(egui::RichText::new("Mounts:").color(ROW_NAME_COLOR));
                            let grid_id = format!("pod_details_mounts_grid_{}", container.name);
                            egui::Grid::new(grid_id).striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                                ui.label("");
                                ui.end_row();
                                for mount in container.mounts.iter() {
                                    ui.label(egui::RichText::new(&mount.volume_name).color(DETAIL_COLOR));
                                    ui.label(egui::RichText::new(&mount.mount_path).color(SECOND_DETAIL_COLOR));
                                    if mount.read_only.unwrap_or_default() {
                                        ui.label(egui::RichText::new("RO".to_string()).color(item_color("RO")));
                                    } else {
                                        ui.label(egui::RichText::new("RW".to_string()).color(item_color("RW")));
                                    }
                                    ui.end_row();
                                }
                            });
                            ui.end_row();
                        }

                        if let Some(args) = &container.args {
                            ui.label(egui::RichText::new("Args:").color(ROW_NAME_COLOR));
                            let grid_id = format!("pod_details_args_grid_{}", container.name);
                            egui::Grid::new(grid_id).striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                                ui.label("");
                                ui.end_row();
                                for arg in args.iter() {
                                    ui.label(egui::RichText::new(&arg.to_string()).color(DETAIL_COLOR));
                                    ui.end_row();
                                }
                            });
                            ui.end_row();
                        }

                        if let Some(cmd) = &container.command {
                            ui.label(egui::RichText::new("Command:").color(ROW_NAME_COLOR));
                            let grid_id = format!("pod_details_command_grid_{}", container.name);
                            egui::Grid::new(grid_id).striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                                ui.label("");
                                ui.end_row();
                                for c in cmd.iter() {
                                    ui.label(egui::RichText::new(&c.to_string()).color(DETAIL_COLOR));
                                    ui.end_row();
                                }
                            });
                            ui.end_row();
                        }

                        if !container.env_vars.is_empty() {
                            ui.label(egui::RichText::new("Environment variables:").color(ROW_NAME_COLOR));
                            let grid_id = format!("pod_details_env_vars_grid_{}", container.name);
                            egui::Grid::new(grid_id).striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                                ui.label("");
                                ui.end_row();
                                for env_var in container.env_vars.iter() {
                                    ui.label(egui::RichText::new(&env_var.name).color(DETAIL_COLOR));
                                    if let Some(value) = &env_var.value {
                                        ui.label(egui::RichText::new(value).color(SECOND_DETAIL_COLOR));
                                    } else {
                                        ui.label("");
                                    }
                                    ui.end_row();
                                }
                            });
                            ui.end_row();
                        }
                    }
                }
            });

            ui.separator();
            ui.heading(egui::RichText::new("Events:").color(ROW_NAME_COLOR));
            if !guard_details.events.is_empty() {
                egui::Grid::new("pod_details_events_grid").striped(true).min_col_width(20.0).max_col_width(600.0).show(ui, |ui| {
                    for event in &guard_details.events {
                        ui.label(egui::RichText::new(event.timestamp.clone().unwrap_or_default()).color(DETAIL_COLOR));
                        ui.label(egui::RichText::new(event.reason.clone().unwrap_or_default()).color(SECOND_DETAIL_COLOR));
                        ui.label(egui::RichText::new(event.message.clone().unwrap_or_default()).color(item_color(&event.event_type.clone().unwrap_or_default())));
                        ui.end_row();
                    }
                });
            }
        });
        ui.separator();
    });
    crate::show_delete_confirmation(ctx, delete_confirm);

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            pod_details_window.show = false;
        }
    }
}
