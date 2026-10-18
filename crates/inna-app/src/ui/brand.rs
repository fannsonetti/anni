//! A clean open-book mark with a small completion tick.
use super::theme::{ACCENT, TEXT_PRIMARY};
use egui::{Color32, Pos2, Rect, RichText, Sense, Ui, Vec2};

pub const NAME: &str = "anni";
const INK: Color32 = Color32::from_rgb(22, 38, 43);
const BOOK: &[(f32, f32)] = &[
    (16.0, 20.0),
    (25.0, 20.0),
    (32.0, 24.0),
    (39.0, 20.0),
    (48.0, 20.0),
    (48.0, 43.0),
    (39.0, 43.0),
    (32.0, 47.0),
    (25.0, 43.0),
    (16.0, 43.0),
    (16.0, 20.0),
];
const TICK: &[(f32, f32)] = &[(37.0, 32.0), (40.0, 35.0), (45.0, 29.0)];

pub fn wordmark(ui: &mut Ui, compact: bool) -> egui::Response {
    let size = if compact { 30.0 } else { 38.0 };
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        paint_mark(ui.painter(), rect);
        ui.label(
            RichText::new(NAME)
                .size(if compact { 25.0 } else { 31.0 })
                .strong()
                .color(TEXT_PRIMARY),
        );
    })
    .response
    .interact(Sense::click())
    .on_hover_text("Open Today")
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_mark(painter: &egui::Painter, rect: Rect) {
    let unit = rect.width() / 64.0;
    let point = |x, y| rect.min + egui::vec2(x * unit, y * unit);
    painter.rect_filled(rect, 16.0 * unit, INK);
    for (path, color, width) in [
        (BOOK, ACCENT, 3.0),
        (&[(32.0, 24.0), (32.0, 47.0)][..], ACCENT, 3.0),
        (TICK, TEXT_PRIMARY, 2.5),
    ] {
        for pair in path.windows(2) {
            painter.line_segment(
                [point(pair[0].0, pair[0].1), point(pair[1].0, pair[1].1)],
                egui::Stroke::new(width * unit, color),
            );
        }
        for &(x, y) in path {
            painter.circle_filled(point(x, y), width * unit * 0.5, color);
        }
    }
}

/// Rasterize the same geometry for native window/taskbar icons, without an image runtime.
pub fn icon() -> egui::IconData {
    const SIZE: usize = 128;
    let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut channels = [0_u32; 4];
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = (x as f32 + (sx as f32 + 0.5) / 4.0) / 2.0;
                    let py = (y as f32 + (sy as f32 + 0.5) / 4.0) / 2.0;
                    let color = sample(px, py);
                    for (total, value) in channels.iter_mut().zip(color.to_array()) {
                        *total += u32::from(value);
                    }
                }
            }
            rgba.extend(channels.map(|v| (v / 16) as u8));
        }
    }
    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}

fn sample(x: f32, y: f32) -> Color32 {
    let corner = Pos2::new(x.clamp(16.0, 48.0), y.clamp(16.0, 48.0));
    if corner.distance(Pos2::new(x, y)) > 16.0 {
        return Color32::TRANSPARENT;
    }
    let point = Pos2::new(x, y);
    for (path, color, width) in [
        (TICK, TEXT_PRIMARY, 2.5),
        (BOOK, ACCENT, 3.0),
        (&[(32.0, 24.0), (32.0, 47.0)][..], ACCENT, 3.0),
    ] {
        for pair in path.windows(2) {
            let a = Pos2::new(pair[0].0, pair[0].1);
            let b = Pos2::new(pair[1].0, pair[1].1);
            let direction = b - a;
            let t = ((point - a).dot(direction) / direction.length_sq()).clamp(0.0, 1.0);
            if point.distance(a + direction * t) <= width * 0.5 {
                return color;
            }
        }
    }
    INK
}
