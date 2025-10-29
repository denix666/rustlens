use egui::{Context, Key};
use ipnetwork::{IpNetwork, IpNetworkError};
use std::str::FromStr;

pub struct IpCalculatorWindow {
    pub show: bool,
    input_cidr: String,
    calculated_data: Result<Calculated, IpNetworkError>,
}

impl IpCalculatorWindow {
    pub fn new() -> Self {
        let default_cidr = "10.10.10.10/27".to_string();
        let initial_data = calculate_input(&default_cidr);

        Self {
            show: false,
            input_cidr: default_cidr,
            calculated_data: initial_data,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Calculated {
    first_addr: String,
    last_addr: String,
    host_count: String,
}

pub fn calculate_input(cidr: &str) -> Result<Calculated, IpNetworkError> {
    let network = IpNetwork::from_str(cidr.trim())?;

    Ok(Calculated {
        first_addr: network.network().to_string(),
        last_addr: network.broadcast().to_string(),
        host_count: network.size().to_string(),
    })
}

pub fn show_ipcalculator_window(ctx: &Context, ipcalculator_window: &mut IpCalculatorWindow) {
    let response = egui::Window::new("IP/Subnet Calculator")
        .collapsible(false)
        .resizable(true)
        .open(&mut ipcalculator_window.show)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.add_space(10.0);

                let response = ui.text_edit_singleline(&mut ipcalculator_window.input_cidr);

                if response.changed() {
                    ipcalculator_window.calculated_data =
                        calculate_input(&ipcalculator_window.input_cidr);
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                match &ipcalculator_window.calculated_data {
                    Ok(res) => {
                        ui.colored_label(egui::Color32::LIGHT_GREEN, format!("Range:  {} - {}", res.first_addr, res.last_addr));
                        ui.colored_label(egui::Color32::LIGHT_BLUE, format!("Hosts count:  {}", res.host_count));
                    }
                    Err(e) => {
                        if e.to_string().len() > 50 {
                            ui.colored_label(egui::Color32::RED, format!("Error in input"));
                        } else {
                            ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                        }
                    }
                }

                ui.add_space(10.0);
            });
        });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            ipcalculator_window.show = false;
        }
    }
}
