use egui::{Context, Key};

#[derive(Default)]
pub struct ResConverterWindow {
    // --- Requests ---
    req_cpu_cores: String,
    req_cpu_milli: String,
    req_mem_ki: String,
    req_mem_mi: String,
    req_mem_gi: String,
    // --- Limits ---
    lim_cpu_cores: String,
    lim_cpu_milli: String,
    lim_mem_ki: String,
    lim_mem_mi: String,
    lim_mem_gi: String,
    // --- Output ---
    yaml_preview: String,
    // --- For YAML ---
    req_cpu: String,
    req_mem: String,
    lim_cpu: String,
    lim_mem: String,
    // --- show window ---
    pub show: bool,
}

impl ResConverterWindow {
    pub fn default() -> Self {
        Self {
            req_cpu: "1".to_string(),
            lim_cpu: "2".to_string(),
            req_mem: "1Gi".to_string(),
            lim_mem: "2Gi".to_string(),

            req_cpu_cores: "1".to_string(),
            req_cpu_milli: "1000m".to_string(),
            req_mem_ki: "1048576Ki".to_string(),
            req_mem_mi: "1024Mi".to_string(),
            req_mem_gi: "1Gi".to_string(),
            lim_cpu_cores: "1".to_string(),
            lim_cpu_milli: "1000m".to_string(),
            lim_mem_ki: "2097152Ki".to_string(),
            lim_mem_mi: "1024Mi".to_string(),
            lim_mem_gi: "2Gi".to_string(),

            yaml_preview: Default::default(),

            show: false,
        }
    }
}

fn parse_val(val: &str, unit: &str) -> Result<f64, ()> {
    let cleaned = val.replace(unit, "").trim().to_string();
    if cleaned.is_empty() { return Ok(0.0) };
    cleaned.parse::<f64>().map_err(|_| ())
}

fn cores_to_milli(cores: &str) -> String {
    match parse_val(cores, "") {
        Ok(v) => format!("{:.0}m", v * 1000.0),
        Err(_) => "invalid".to_string(),
    }
}

fn milli_to_cores(milli: &str) -> String {
    match parse_val(milli, "m") {
        Ok(v) => format!("{:.3}", v / 1000.0),
        Err(_) => "invalid".to_string(),
    }
}

fn ki_to_mi_str(ki: &str) -> String {
    match parse_val(ki, "Ki") {
        Ok(v) => format!("{:.3}Mi", v / 1024.0),
        Err(_) => "invalid".to_string(),
    }
}

fn gi_to_mi_str(gi: &str) -> String {
    match parse_val(gi, "Gi") {
        Ok(v) => format!("{:.0}Mi", v * 1024.0),
        Err(_) => "invalid".to_string(),
    }
}

fn ki_to_gi_str(ki: &str) -> String {
    match parse_val(ki, "Ki") {
        Ok(v) => format!("{:.6}Gi", v / 1024.0 / 1024.0),
        Err(_) => "invalid".to_string(),
    }
}

fn gi_to_ki_str(gi: &str) -> String {
    match parse_val(gi, "Gi") {
        Ok(v) => format!("{:.0}Ki", v * 1024.0 * 1024.0),
        Err(_) => "invalid".to_string(),
    }
}

fn mi_to_gi_str(mi: &str) -> String {
    match parse_val(mi, "Mi") {
        Ok(v) => format!("{:.3}Gi", v / 1024.0),
        Err(_) => "invalid".to_string(),
    }
}

fn mi_to_ki_str(mi: &str) -> String {
    match parse_val(mi, "Mi") {
        Ok(v) => format!("{:.0}Ki", v * 1024.0),
        Err(_) => "invalid".to_string(),
    }
}

pub fn show_res_conventer_window(ctx: &Context, res_conventer_window: &mut ResConverterWindow,) {
    let response = egui::Window::new("Resource Converter").collapsible(false).resizable(true).open(&mut res_conventer_window.show).show(ctx, |ui| {

        // === REQUESTS ===
        ui.heading("Requests");
        egui::Grid::new("requests_grid").num_columns(6).spacing([10.0, 4.0]).min_col_width(100.0).show(ui, |ui| {
            // --- CPU Requests ---
            ui.label("CPU (cores):");
            if ui.text_edit_singleline(&mut res_conventer_window.req_cpu_cores).changed() {
                res_conventer_window.req_cpu_milli = cores_to_milli(&res_conventer_window.req_cpu_cores);
                res_conventer_window.req_cpu = if res_conventer_window.req_cpu_cores.is_empty() || res_conventer_window.req_cpu_milli.contains("invalid") {
                    "0m".to_string()
                } else {
                    format!("{}", res_conventer_window.req_cpu_cores.trim())
                };
            }
            ui.label("CPU (millicores):");
            if ui.text_edit_singleline(&mut res_conventer_window.req_cpu_milli).changed() {
                res_conventer_window.req_cpu_cores = milli_to_cores(&res_conventer_window.req_cpu_milli);
                res_conventer_window.req_cpu = if res_conventer_window.req_cpu_milli.is_empty() || res_conventer_window.req_cpu_cores.contains("invalid") {
                    "0m".to_string()
                } else {
                    format!("{}m", res_conventer_window.req_cpu_milli.trim())
                };
            }
            ui.end_row();

            // --- Memory Requests ---
            ui.label("Memory (GiB):");
            if ui.text_edit_singleline(&mut res_conventer_window.req_mem_gi).changed() {
                res_conventer_window.req_mem_ki = gi_to_ki_str(&res_conventer_window.req_mem_gi);
                res_conventer_window.req_mem_mi = gi_to_mi_str(&res_conventer_window.req_mem_gi);
                res_conventer_window.req_mem = if res_conventer_window.req_mem_gi.is_empty() || res_conventer_window.req_mem_mi.contains("invalid") || res_conventer_window.req_mem_ki.contains("invalid") {
                    "0Gi".to_string()
                } else {
                    format!("{}Gi", res_conventer_window.req_mem_gi.trim())
                };
            }
            ui.label("Memory (MiB):");
            if ui.text_edit_singleline(&mut res_conventer_window.req_mem_mi).changed() {
                res_conventer_window.req_mem_ki = mi_to_ki_str(&res_conventer_window.req_mem_mi);
                res_conventer_window.req_mem_gi = mi_to_gi_str(&res_conventer_window.req_mem_mi);
                res_conventer_window.req_mem = if res_conventer_window.req_mem_mi.is_empty() || res_conventer_window.req_mem_gi.contains("invalid") || res_conventer_window.req_mem_ki.contains("invalid") {
                    "0Mi".to_string()
                } else {
                    format!("{}Mi", res_conventer_window.req_mem_mi.trim())
                };
            }
            ui.label("Memory (KiB):");
            if ui.text_edit_singleline(&mut res_conventer_window.req_mem_ki).changed() {
                res_conventer_window.req_mem_mi = ki_to_mi_str(&res_conventer_window.req_mem_ki);
                res_conventer_window.req_mem_gi = ki_to_gi_str(&res_conventer_window.req_mem_ki);
                res_conventer_window.req_mem = if res_conventer_window.req_mem_ki.is_empty() || res_conventer_window.req_mem_gi.contains("invalid") || res_conventer_window.req_mem_mi.contains("invalid") {
                    "0Ki".to_string()
                } else {
                    format!("{}Ki", res_conventer_window.req_mem_ki.trim())
                };
            }
            ui.end_row();
        });
        ui.separator();

        // === LIMITS ===
        ui.heading("Limits");
        egui::Grid::new("limits_grid").num_columns(6).spacing([10.0, 4.0]).min_col_width(100.0).show(ui, |ui| {
            // --- CPU Limits ---
            ui.label("CPU (cores):");
            if ui.text_edit_singleline(&mut res_conventer_window.lim_cpu_cores).changed() {
                res_conventer_window.lim_cpu_milli = cores_to_milli(&res_conventer_window.lim_cpu_cores);
                res_conventer_window.lim_cpu = if res_conventer_window.lim_cpu_cores.is_empty() || res_conventer_window.lim_cpu_milli.contains("invalid") {
                    "0m".to_string()
                } else {
                    format!("{}", res_conventer_window.lim_cpu_cores.trim())
                };
            }
            ui.label("CPU (millicores):");
            if ui.text_edit_singleline(&mut res_conventer_window.lim_cpu_milli).changed() {
                res_conventer_window.lim_cpu_cores = milli_to_cores(&res_conventer_window.lim_cpu_milli);
                res_conventer_window.lim_cpu = if res_conventer_window.lim_cpu_milli.is_empty() || res_conventer_window.lim_cpu_cores.contains("invalid") {
                    "0m".to_string()
                } else {
                    format!("{}m", res_conventer_window.lim_cpu_milli.trim())
                };
            }
            ui.end_row();

            // --- Memory Limits ---
            ui.label("Memory (GiB):");
            if ui.text_edit_singleline(&mut res_conventer_window.lim_mem_gi).changed() {
                res_conventer_window.lim_mem_ki = gi_to_ki_str(&res_conventer_window.lim_mem_gi);
                res_conventer_window.lim_mem_mi = gi_to_mi_str(&res_conventer_window.lim_mem_gi);
                res_conventer_window.lim_mem = if res_conventer_window.lim_mem_gi.is_empty() || res_conventer_window.lim_mem_mi.contains("invalid") || res_conventer_window.lim_mem_ki.contains("invalid") {
                    "0Gi".to_string()
                } else {
                    format!("{}Gi", res_conventer_window.lim_mem_gi.trim())
                };
            }
            ui.label("Memory (MiB):");
            if ui.text_edit_singleline(&mut res_conventer_window.lim_mem_mi).changed() {
                res_conventer_window.lim_mem_ki = mi_to_ki_str(&res_conventer_window.lim_mem_mi);
                res_conventer_window.lim_mem_gi = mi_to_gi_str(&res_conventer_window.lim_mem_mi);
                res_conventer_window.lim_mem = if res_conventer_window.lim_mem_mi.is_empty() || res_conventer_window.lim_mem_gi.contains("invalid") || res_conventer_window.lim_mem_ki.contains("invalid") {
                    "0Mi".to_string()
                } else {
                    format!("{}Mi", res_conventer_window.lim_mem_mi.trim())
                };
            }
            ui.label("Memory (KiB):");
            if ui.text_edit_singleline(&mut res_conventer_window.lim_mem_ki).changed() {
                res_conventer_window.lim_mem_mi = ki_to_mi_str(&res_conventer_window.lim_mem_ki);
                res_conventer_window.lim_mem_gi = ki_to_gi_str(&res_conventer_window.lim_mem_ki);
                res_conventer_window.lim_mem = if res_conventer_window.lim_mem_ki.is_empty() || res_conventer_window.lim_mem_gi.contains("invalid") || res_conventer_window.lim_mem_mi.contains("invalid") {
                    "0Ki".to_string()
                } else {
                    format!("{}Ki", res_conventer_window.lim_mem_ki.trim())
                };
            }
            ui.end_row();
        });

        ui.separator();

        res_conventer_window.yaml_preview = format!(r#"resources:
  requests:
    cpu: "{req_cpu}"
    memory: "{req_mem}"
  limits:
    cpu: "{lim_cpu}"
    memory: "{lim_mem}"
"#,
            req_cpu = res_conventer_window.req_cpu,
            req_mem = res_conventer_window.req_mem,
            lim_cpu = res_conventer_window.lim_cpu,
            lim_mem = res_conventer_window.lim_mem
        );

        // --- YAML PREVIEW ---
        ui.heading("Generated YAML");
        ui.add_space(10.0);
        ui.monospace(&res_conventer_window.yaml_preview);

        // --- Copy to clipboard button ---
        if ui.button("📋 Copy YAML to clipboard").clicked() {
            ui.ctx().copy_text(res_conventer_window.yaml_preview.clone());
        }

        ui.add_space(10.0);
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            res_conventer_window.show = false;
        }
    }
}
