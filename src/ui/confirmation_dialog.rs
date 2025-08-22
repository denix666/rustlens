use egui::{Context, Align2};

pub struct DeleteConfirmation {
    pub show: bool,
    pub resource_name: Option<String>,
    pub namespace: Option<String>,
    pub on_confirm: Option<Box<dyn FnOnce() + Send>>,
}

impl DeleteConfirmation {
    pub fn new() -> Self {
        Self {
            show: false,
            resource_name: None,
            namespace: None,
            on_confirm: None,
        }
    }

    pub fn request<F>(&mut self, resource_name: String, namespace: Option<String>, on_confirm: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.show = true;
        self.resource_name = Some(resource_name);
        self.namespace = namespace;
        self.on_confirm = Some(Box::new(on_confirm));
    }
}

pub fn show_delete_confirmation(ctx: &Context, delete_confirm: &mut DeleteConfirmation) {
    if delete_confirm.show {
        let resource_name = delete_confirm.resource_name.clone().unwrap_or_default();
        egui::Window::new("Confirm deletion")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Are you sure you want to delete \"{}\"?", resource_name));

                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new("Yes, delete").color(crate::RED_BUTTON)).clicked() {
                        if let Some(callback) = delete_confirm.on_confirm.take() {
                            callback();
                        }
                        delete_confirm.show = false;
                    }

                    if ui.button(egui::RichText::new("Cancel").color(crate::GREEN_BUTTON)).clicked() {
                        delete_confirm.show = false;
                    }
                });
            });
    }
}
