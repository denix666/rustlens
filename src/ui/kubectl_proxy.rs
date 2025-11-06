use std::process::Command;

use crate::theme::{ERROR_MESSAGE_COLOR, NORMAL_COLOR, WARNING_COLOR};

pub struct ProxyProcess {
    is_running: bool,
    process: Option<std::process::Child>,
    error_message: Option<String>,
}

impl Default for ProxyProcess {
    fn default() -> Self {
        Self {
            is_running: false,
            process: None,
            error_message: None,
        }
    }
}

impl Drop for ProxyProcess {
    fn drop(&mut self) {
        log::info!("App closed. Stopping all processes...");
        if let Some(mut child) = self.process.take() {
            log::info!("Stopping kubectl proxy...");
            if let Err(e) = child.kill() {
                log::error!("Error occured while stopping kubectl process: {}", e);
            }
        }
    }
}

pub fn show_kubectl_proxy_status(ui: &mut egui::Ui, proxy_process: &mut ProxyProcess,) {
    if let Some(child) = &mut proxy_process.process {
        match child.try_wait() {
            Ok(Some(status)) => {
                log::warn!("Process was killed: {}", status);
                proxy_process.process = None;
                proxy_process.is_running = false;
                proxy_process.error_message = Some(format!("Process unexpectially stopped: {}", status));
            }
            Ok(None) => {
                // process still running
            }
            Err(e) => {
                log::error!("Error getting process status: {}", e);
                proxy_process.process = None;
                proxy_process.is_running = false;
                proxy_process.error_message = Some(format!("Error getting process status: {}", e));
            }
        }
    }

    ui.heading("Proxy status");
    ui.separator();

    let checkbox_response = ui.checkbox(&mut proxy_process.is_running, "Enable proxy");

    if checkbox_response.changed() {
        proxy_process.error_message = None;

        if proxy_process.is_running {
            match Command::new("kubectl").arg("proxy").spawn() {
                Ok(child_process) => {
                    proxy_process.process = Some(child_process);
                    log::info!("Process kubectl proxy started.");
                }
                Err(e) => {
                    log::error!("Failed to start kubectl proxy: {}", e);
                    proxy_process.is_running = false;
                    proxy_process.process = None;
                    proxy_process.error_message = Some(format!("Error running kubectl: {}. Make sure 'kubectl' is installed in your system.",e));
                }
            }
        } else {
            if let Some(mut child) = proxy_process.process.take() {
                if let Err(e) = child.kill() {
                    log::error!("Failed to stop kubectl process: {}", e);
                    proxy_process.error_message = Some(format!("Error stopping process: {}", e));
                } else {
                    log::info!("Successfully stopped kubectl proxy process.");
                }
            }
        }
    }

    // --- process status ---
    ui.separator();
    if proxy_process.is_running {
        ui.label(
            egui::RichText::new("▶ Status: running").color(NORMAL_COLOR).strong(),
        );
    } else {
        ui.label(
            egui::RichText::new("■ Status: stopped").color(WARNING_COLOR).strong(),
        );
    }

    ui.add_space(20.0);

    // Show error if some
    if let Some(err) = &proxy_process.error_message {
        ui.label(egui::RichText::new(err).color(ERROR_MESSAGE_COLOR));
    }
}
