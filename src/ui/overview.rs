use indexmap::IndexMap;
use egui::{Color32, Pos2, Ui};
use crate::functions::item_color;

#[derive(Default, Debug, Clone)]
pub struct OverviewStats {
    pub pods_running: usize,
    pub pods_pending: usize,
    pub pods_running_total: usize,
    pub pods_capacity: Option<u32>,
    pub deployments_running: usize,
    pub deployments_pending: usize,
    pub daemonsets_running: usize,
    pub daemonsets_pending: usize,
    pub statefulsets_running: usize,
    pub statefulsets_pending: usize,
    pub replicasets_running: usize,
    pub replicasets_pending: usize,
    pub namespaces_with_pending_items: IndexMap<String, i32>,
    pub cpu_total: Option<f32>,
    pub cpu_available: Option<f32>,
    pub cpu_used: Option<f32>,
    pub cpu_requests: f32,
    pub cpu_limits: f32,
    pub memory_total: Option<f32>,
    pub memory_available: Option<f32>,
    pub memory_used: Option<f32>,
    pub memory_requests: f32,
    pub memory_limits: f32,
}

impl OverviewStats {
    pub fn retain_previous_resource_values(&mut self, previous: &Self) {
        if self.cpu_total.is_none() {
            self.cpu_total = previous.cpu_total;
        }
        if self.cpu_available.is_none() {
            self.cpu_available = previous.cpu_available;
        }
        if self.cpu_used.is_none() {
            self.cpu_used = previous.cpu_used;
        }
        if self.memory_total.is_none() {
            self.memory_total = previous.memory_total;
        }
        if self.memory_available.is_none() {
            self.memory_available = previous.memory_available;
        }
        if self.memory_used.is_none() {
            self.memory_used = previous.memory_used;
        }
        if self.pods_capacity.is_none() {
            self.pods_capacity = previous.pods_capacity;
        }
    }
}

fn paint_filled_arc(ui: &Ui, center: Pos2, inner_radius: f32, outer_radius: f32, start_angle: f32, end_angle: f32, color: Color32) {
    let total_span = end_angle - start_angle;
    if total_span.abs() < 0.001 {
        return;
    }

    let max_segment = std::f32::consts::TAU / 72.0;
    let num_segments = (total_span.abs() / max_segment).ceil().max(1.0) as usize;

    for segment in 0..num_segments {
        let segment_start = start_angle + total_span * (segment as f32 / num_segments as f32);
        let segment_end = start_angle + total_span * ((segment + 1) as f32 / num_segments as f32);
        let points = vec![
            center + outer_radius * egui::vec2(segment_start.cos(), segment_start.sin()),
            center + outer_radius * egui::vec2(segment_end.cos(), segment_end.sin()),
            center + inner_radius * egui::vec2(segment_end.cos(), segment_end.sin()),
            center + inner_radius * egui::vec2(segment_start.cos(), segment_start.sin()),
        ];

        ui.painter().add(egui::Shape::convex_polygon(
            points,
            color,
            egui::Stroke::NONE,
        ));
    }
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

pub fn show_resource_overview(ui: &mut egui::Ui, stats: &OverviewStats) {
    ui.horizontal_wrapped(|ui| {
        show_resource_stat(ui, "CPU", stats.cpu_total, stats.cpu_available, stats.cpu_used, stats.cpu_requests, stats.cpu_limits, "cores");
        ui.separator();
        show_resource_stat(ui, "Memory", stats.memory_total, stats.memory_available, stats.memory_used, stats.memory_requests, stats.memory_limits, "GiB");
        ui.separator();
        show_pod_capacity_stat(ui, stats.pods_running_total, stats.pods_capacity);
    });
}

fn format_resource(value: Option<f32>, unit: &str) -> String {
    value
        .map(|value| format!("{value:.2} {unit}"))
        .unwrap_or_else(|| "Loading...".to_string())
}

fn show_resource_stat(
    ui: &mut egui::Ui,
    title: &str,
    total: Option<f32>,
    available: Option<f32>,
    used: Option<f32>,
    requests: f32,
    limits: f32,
    unit: &str,
) {
    let limits_over_allocatable = matches!(available, Some(available) if limits > available);

    ui.group(|ui| {
        ui.set_width(250.0);
        ui.vertical(|ui| {
            ui.vertical_centered(|ui| {
                ui.heading(title);
                ui.separator();
                show_resource_rings(ui, total, available, used, requests, limits);
            });

            show_resource_legend_item(ui, "Usage", used, unit, Color32::from_rgb(175, 121, 255), false);
            show_resource_legend_item(ui, "Requests", Some(requests), unit, Color32::from_rgb(255, 255, 80), false);
            show_resource_legend_item(ui, "Limits", Some(limits), unit, Color32::from_rgb(64, 165, 205), limits_over_allocatable);
            show_resource_legend_item(ui, "Allocatable Capacity", available, unit, Color32::from_rgb(16, 55, 116), false);
            show_resource_legend_item(ui, "Capacity", total, unit, Color32::from_rgb(47, 53, 58), false);
        });
    });
}

fn show_resource_rings(
    ui: &mut egui::Ui,
    total: Option<f32>,
    available: Option<f32>,
    used: Option<f32>,
    requests: f32,
    limits: f32,
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(150.0, 150.0), egui::Sense::hover());
    let center = rect.center();
    let start_angle = -std::f32::consts::FRAC_PI_2;
    let track_color = Color32::from_rgb(47, 53, 58);
    let rings = [
        (used, Color32::from_rgb(175, 121, 255)),
        (Some(requests), Color32::from_rgb(255, 255, 80)),
        (Some(limits), Color32::from_rgb(64, 165, 205)),
        (available, Color32::from_rgb(16, 55, 116)),
        (total, track_color),
    ];

    for (index, (value, color)) in rings.iter().enumerate() {
        let radius = 65.0 - index as f32 * 11.0;
        let inner_radius = radius - 6.0;
        paint_filled_arc(ui, center, inner_radius, radius, 0.0, std::f32::consts::TAU, track_color);

        if let (Some(value), Some(total)) = (*value, total)
            && total > 0.0
        {
            let fraction = (value / total).clamp(0.0, 1.0);
            paint_filled_arc(ui, center, inner_radius, radius, start_angle, start_angle + fraction * std::f32::consts::TAU, *color);
        }
    }
}

fn show_resource_legend_item(
    ui: &mut egui::Ui,
    label: &str,
    value: Option<f32>,
    unit: &str,
    color: Color32,
    warning: bool,
) {
    ui.horizontal(|ui| {
        ui.colored_label(color, "■");
        let value_text = format_resource(value, unit);
        let body_font = ui.style().text_styles
            .get(&egui::TextStyle::Body)
            .cloned()
            .unwrap_or_else(|| egui::FontId::proportional(14.0));
        let normal_format = egui::TextFormat::simple(body_font.clone(), ui.visuals().text_color());
        let value_format = egui::TextFormat::simple(
            body_font,
            if warning { Color32::RED } else { ui.visuals().text_color() },
        );
        let mut layout = egui::text::LayoutJob::default();
        layout.append(&format!("{label}: "), 0.0, normal_format);
        layout.append(&value_text, 0.0, value_format);
        let response = ui.label(layout);
        if warning {
            response.on_hover_text("⚠️  specified limits are higher than allocatable capacity!");
        }
    });
}

fn show_pod_capacity_stat(ui: &mut egui::Ui, running: usize, capacity: Option<u32>) {
    ui.group(|ui| {
        ui.set_width(250.0);
        ui.vertical(|ui| {
            ui.heading("Pods");
            ui.separator();
            match capacity {
                Some(capacity) => {
                    ui.label(format!("Running: {running} / {capacity}"));
                    if capacity > 0 {
                        ui.add(
                            egui::ProgressBar::new((running as f32 / capacity as f32).clamp(0.0, 1.0))
                                .show_percentage(),
                        );
                    }
                }
                None => {
                    ui.label("Running: Loading...");
                }
            }
        });
    });
}

fn show_stat_circle(ui: &mut egui::Ui, title: &str, ok_count: usize, pending_count: usize) {
    let total = ok_count + pending_count;
    let fraction_ok = if total > 0 { ok_count as f32 / total as f32 } else { 0.0 };
    let fraction_pending = if total > 0 { pending_count as f32 / total as f32 } else { 0.0 };

    let (rect, _) = ui.allocate_exact_size(egui::vec2(100.0, 100.0), egui::Sense::hover());

    let painter = ui.painter();

    // Arc settings
    let outer_radius = rect.width() / 2.0 - 2.0;
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
    painter.circle_filled(rect.center(), inner_radius + 1.0, ui.visuals().widgets.active.bg_fill);

    ui.vertical(|ui| {
        ui.label(egui::RichText::new(format!("{} ({})", title, total)).family(egui::FontFamily::Monospace));
        ui.label(egui::RichText::new(format!("◾ Running: {}", ok_count)).small().color(item_color("Running")));
        if pending_count > 0 {
            ui.label(egui::RichText::new(format!("◾ Pending: {}", pending_count)).small().color(item_color("Pending")));
        }
    });
}
