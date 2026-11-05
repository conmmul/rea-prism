//! Small shared controls used by the dialogs and the inspector.

use egui::{ComboBox, DragValue, Response, Ui};
use prism_doc::graph::{Color, LineStyle, Stroke, SymbolShape};

/// A labelled row, so every settings line in a dialog shares one left column.
pub fn row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let mut out = None;
    ui.horizontal(|ui| {
        ui.add_sized([128.0, 18.0], egui::Label::new(label).halign(egui::Align::RIGHT));
        out = Some(add(ui));
    });
    out.expect("closure always runs")
}

/// A bordered group with a heading, matching how Prism boxes related settings.
pub fn group<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(title).strong().size(11.0));
    let r = egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, add);
    r.inner
}

pub fn color(ui: &mut Ui, c: &mut Color) -> bool {
    let mut rgba = [c.r, c.g, c.b, c.a];
    if ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed() {
        *c = Color { r: rgba[0], g: rgba[1], b: rgba[2], a: rgba[3] };
        true
    } else {
        false
    }
}

/// A combo box over a fixed set of values.
pub fn combo<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    let current = options.iter().find(|(v, _)| v == value).map(|(_, n)| *n).unwrap_or("");
    ComboBox::from_id_salt(id).selected_text(current).show_ui(ui, |ui| {
        for (v, name) in options {
            if ui.selectable_label(v == value, *name).clicked() {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

pub fn line_style(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, v: &mut LineStyle) -> bool {
    combo(
        ui,
        id,
        v,
        &[
            (LineStyle::Solid, "Solid"),
            (LineStyle::Dash, "Dashed"),
            (LineStyle::Dot, "Dotted"),
            (LineStyle::DashDot, "Dash-dot"),
            (LineStyle::None, "None"),
        ],
    )
}

pub fn symbol_shape(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, v: &mut SymbolShape) -> bool {
    combo(
        ui,
        id,
        v,
        &[
            (SymbolShape::Circle, "Circle"),
            (SymbolShape::Square, "Square"),
            (SymbolShape::Triangle, "Triangle up"),
            (SymbolShape::TriangleDown, "Triangle down"),
            (SymbolShape::Diamond, "Diamond"),
            (SymbolShape::Plus, "Plus"),
            (SymbolShape::Cross, "Cross"),
            (SymbolShape::Star, "Star"),
            (SymbolShape::Dash, "Bar"),
            (SymbolShape::None, "None"),
        ],
    )
}

/// Style, thickness and colour for one stroke, on a single line.
pub fn stroke(ui: &mut Ui, id: &str, s: &mut Stroke) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= line_style(ui, format!("{id}_style"), &mut s.style);
        changed |= ui.add(DragValue::new(&mut s.width).range(0.0..=12.0).speed(0.1).suffix(" pt")).changed();
        changed |= color(ui, &mut s.color);
    });
    changed
}

pub fn points(ui: &mut Ui, v: &mut f32, range: std::ops::RangeInclusive<f32>) -> Response {
    ui.add(DragValue::new(v).range(range).speed(0.25).suffix(" pt"))
}
