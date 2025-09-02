use indexmap::IndexMap;
use egui::{Color32, Pos2, Ui};
use crate::functions::item_color;

#[derive(Default, Debug, Clone)]
pub struct OverviewStats {
    pub pods_running: usize,
    pub pods_pending: usize,
    pub deployments_running: usize,
    pub deployments_pending: usize,
    pub daemonsets_running: usize,
    pub daemonsets_pending: usize,
    pub statefulsets_running: usize,
    pub statefulsets_pending: usize,
    pub replicasets_running: usize,
    pub replicasets_pending: usize,
    pub namespaces_with_pending_items: IndexMap<String, i32>,
}

fn paint_filled_arc(ui: &Ui, center: Pos2, inner_radius: f32, outer_radius: f32, start_angle: f32, end_angle: f32, color: Color32) {
    let angle_span = end_angle - start_angle;
    let num_points = (angle_span.abs() * 30.0).ceil().max(5.0) as usize;
    let mut points = Vec::with_capacity(num_points * 2);

    for i in 0..=num_points {
        let angle = start_angle + angle_span * (i as f32 / num_points as f32);
        points.push(center + outer_radius * egui::vec2(angle.cos(), angle.sin()));
    }

    for i in (0..=num_points).rev() {
        let angle = start_angle + angle_span * (i as f32 / num_points as f32);
        points.push(center + inner_radius * egui::vec2(angle.cos(), angle.sin()));
    }

    ui.painter().add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
}

pub fn show_overview(ui: &mut egui::Ui, stats: &OverviewStats) {
    ui.horizontal(|ui| {
        show_stat_circle(ui, "Pods", stats.pods_running, stats.pods_pending);
        ui.separator();
        show_stat_circle(ui, "Deployments", stats.deployments_running, stats.deployments_pending);
        ui.separator();
        show_stat_circle(ui, "Daemonsets", stats.daemonsets_running, stats.daemonsets_pending);
        ui.separator();
        show_stat_circle(ui, "Statefulsets", stats.statefulsets_running, stats.statefulsets_pending);
        ui.separator();
        show_stat_circle(ui, "Replicasets", stats.replicasets_running, stats.replicasets_pending);
    });
}

fn show_stat_circle(ui: &mut egui::Ui, title: &str, ok_count: usize, pending_count: usize) {
    let total = ok_count + pending_count;
    let fraction_ok = if total > 0 { ok_count as f32 / total as f32 } else { 0.0 };
    let fraction_pending = if total > 0 { pending_count as f32 / total as f32 } else { 0.0 };

    let (rect, _) = ui.allocate_exact_size(egui::vec2(100.0, 100.0), egui::Sense::hover());

    let painter = ui.painter();

    // Arc settings
    let outer_radius = rect.width() / 2.0;
    let inner_radius = outer_radius - 17.0; // Arc width
    let start_angle = -std::f32::consts::FRAC_PI_2; // Begin from top

    // Arc "Running" in green color
    let ok_angle_span = fraction_ok * std::f32::consts::TAU;
    let ok_end_angle = start_angle + ok_angle_span;
    if ok_count > 0 {
        paint_filled_arc(
            ui,
            rect.center(),
            inner_radius,
            outer_radius,
            start_angle,
            ok_end_angle,
            crate::GREEN_BUTTON,
        );
    }

    // Arc "Pending" in orange color
    if pending_count > 0 {
        let pending_start_angle = ok_end_angle;
        let pending_angle_span = fraction_pending * std::f32::consts::TAU;
        let pending_end_angle = pending_start_angle + pending_angle_span;
        paint_filled_arc(
            ui,
            rect.center(),
            inner_radius,
            outer_radius,
            pending_start_angle,
            pending_end_angle,
            crate::ORANGE_BUTTON,
        );
    }

    // Circle background
    painter.circle_filled(rect.center(), rect.width() / 3.0, ui.visuals().widgets.active.bg_fill);

    ui.vertical(|ui| {
        ui.label(egui::RichText::new(format!("{} ({})", title, total)).family(egui::FontFamily::Monospace));
        ui.label(egui::RichText::new(format!("◾ Running: {}", ok_count)).small().color(item_color("Running")));
        if pending_count > 0 {
            ui.label(egui::RichText::new(format!("◾ Pending: {}", pending_count)).small().color(item_color("Pending")));
        }
    });
}
