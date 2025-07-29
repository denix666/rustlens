use egui::{Direction, Ui};

pub fn show_loading(ui: &mut Ui) {
    ui.with_layout(
        egui::Layout::centered_and_justified(Direction::TopDown),
        |ui| {
            ui.spinner();
            //ui.label(egui::RichText::new("⏳ Loading...").heading());
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
