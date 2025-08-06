use egui::{Direction, Layout, RichText, Ui};

pub fn show_loading(ui: &mut Ui) {
    ui.with_layout(
        Layout::centered_and_justified(Direction::TopDown),
        |ui| {
            ui.add_space(ui.available_size().y * 0.5 - 50.0);
            ui.vertical_centered_justified(|ui| {
                ui.add(egui::Spinner::new().size(30.0));
                ui.add_space(10.0);
                ui.label(RichText::new("⏳ Loading...").heading());
            });
        },
    );
}

pub fn show_empty(ui: &mut Ui) {
    ui.with_layout(
        egui::Layout::centered_and_justified(Direction::TopDown),
        |ui| {
            ui.label(egui::RichText::new("😟 Empty").heading());
        },
    );
}
