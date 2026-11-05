//! Painting a [`prism_render::Scene`] with egui.

use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use prism_render::scene::{DrawCmd, HAlign, Point, VAlign};
use prism_render::{Scene, Tag};

pub fn color(c: prism_doc::graph::Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

fn pos(p: Point, origin: Pos2, zoom: f32) -> Pos2 {
    Pos2::new(origin.x + p.x * zoom, origin.y + p.y * zoom)
}

fn stroke(s: &prism_render::scene::Stroke, zoom: f32) -> Stroke {
    Stroke::new((s.width * zoom).max(0.25), color(s.color))
}

/// Paint a scene into `painter`, with the page's top-left at `origin`.
///
/// `highlight` draws the marks of one series in a contrasting outline, which the
/// inspector uses to show which dataset is selected.
pub fn scene(
    painter: &egui::Painter,
    scene: &Scene,
    origin: Pos2,
    zoom: f32,
    highlight: Option<usize>,
) {
    for tagged in scene.iter() {
        let emphasise = matches!((tagged.tag, highlight), (Tag::Series(i), Some(h)) if i == h);
        paint_cmd(painter, &tagged.cmd, origin, zoom, emphasise);
    }
}

fn paint_cmd(
    painter: &egui::Painter,
    cmd: &DrawCmd,
    origin: Pos2,
    zoom: f32,
    emphasise: bool,
) {
    let accent = Color32::from_rgb(0, 122, 255);
    match cmd {
        DrawCmd::Line { from, to, stroke: s } => {
            painter.line_segment([pos(*from, origin, zoom), pos(*to, origin, zoom)], stroke(s, zoom));
        }
        DrawCmd::Polyline { points, stroke: s, close } => {
            let pts: Vec<Pos2> = points.iter().map(|p| pos(*p, origin, zoom)).collect();
            if *close {
                painter.add(Shape::closed_line(pts, stroke(s, zoom)));
            } else {
                painter.add(Shape::line(pts, stroke(s, zoom)));
            }
        }
        DrawCmd::Polygon { points, fill, stroke: s } => {
            let pts: Vec<Pos2> = points.iter().map(|p| pos(*p, origin, zoom)).collect();
            let st = s
                .as_ref()
                .map(|s| stroke(s, zoom))
                .unwrap_or(Stroke::NONE);
            let st = if emphasise { Stroke::new(st.width.max(1.5), accent) } else { st };
            painter.add(Shape::convex_polygon(pts, color(*fill), st));
        }
        DrawCmd::Rect { rect, fill, stroke: s } => {
            let r = Rect::from_min_size(
                pos(Point { x: rect.x, y: rect.y }, origin, zoom),
                Vec2::new(rect.w * zoom, rect.h * zoom),
            );
            let st = s.as_ref().map(|s| stroke(s, zoom)).unwrap_or(Stroke::NONE);
            let st = if emphasise { Stroke::new(st.width.max(1.5), accent) } else { st };
            painter.rect(r, 0.0, color(*fill), st, egui::StrokeKind::Inside);
        }
        DrawCmd::Circle { center, radius, fill, stroke: s } => {
            let st = s.as_ref().map(|s| stroke(s, zoom)).unwrap_or(Stroke::NONE);
            let st = if emphasise { Stroke::new(st.width.max(1.5), accent) } else { st };
            painter.circle(pos(*center, origin, zoom), radius * zoom, color(*fill), st);
        }
        DrawCmd::Wedge { center, radius, inner_radius, start_angle, sweep, fill, stroke: s } => {
            // egui has no arc primitive, so approximate the sector with a fan.
            let steps = ((sweep.abs() / 4.0).ceil() as usize).max(2);
            let c = pos(*center, origin, zoom);
            let mut pts = Vec::with_capacity(steps * 2 + 2);
            for i in 0..=steps {
                let a = (start_angle + sweep * i as f32 / steps as f32).to_radians();
                pts.push(Pos2::new(
                    c.x + radius * zoom * a.cos(),
                    c.y - radius * zoom * a.sin(),
                ));
            }
            if *inner_radius > 0.0 {
                for i in (0..=steps).rev() {
                    let a = (start_angle + sweep * i as f32 / steps as f32).to_radians();
                    pts.push(Pos2::new(
                        c.x + inner_radius * zoom * a.cos(),
                        c.y - inner_radius * zoom * a.sin(),
                    ));
                }
            } else {
                pts.push(c);
            }
            let st = s.as_ref().map(|s| stroke(s, zoom)).unwrap_or(Stroke::NONE);
            // A wedge is not convex once it passes a half turn; draw it as a mesh.
            painter.add(Shape::Path(egui::epaint::PathShape {
                points: pts,
                closed: true,
                fill: color(*fill),
                stroke: st.into(),
            }));
        }
        DrawCmd::Text(t) => {
            let anchor = match (t.h, t.v) {
                (HAlign::Left, VAlign::Top) => Align2::LEFT_TOP,
                (HAlign::Left, VAlign::Middle) => Align2::LEFT_CENTER,
                (HAlign::Left, _) => Align2::LEFT_BOTTOM,
                (HAlign::Center, VAlign::Top) => Align2::CENTER_TOP,
                (HAlign::Center, VAlign::Middle) => Align2::CENTER_CENTER,
                (HAlign::Center, _) => Align2::CENTER_BOTTOM,
                (HAlign::Right, VAlign::Top) => Align2::RIGHT_TOP,
                (HAlign::Right, VAlign::Middle) => Align2::RIGHT_CENTER,
                (HAlign::Right, _) => Align2::RIGHT_BOTTOM,
            };
            let font = FontId::new(t.size * zoom, FontFamily::Proportional);
            let at = pos(t.at, origin, zoom);

            if t.rotation.abs() < f32::EPSILON {
                painter.text(at, anchor, &t.text, font, color(t.color));
            } else {
                // Rotate by laying the galley out and transforming it about `at`.
                let galley = painter.layout_no_wrap(t.text.clone(), font, color(t.color));
                let rot = egui::emath::Rot2::from_angle(-t.rotation.to_radians());
                let size = galley.size();
                let offset = anchor_offset(anchor, size);
                let shape = egui::epaint::TextShape::new(Pos2::ZERO, galley, color(t.color))
                    .with_angle(-t.rotation.to_radians());
                let mut shape = Shape::Text(shape);
                shape.translate((rot * offset).into());
                shape.translate(at.to_vec2());
                painter.add(shape);
            }
        }
    }
}

/// Top-left corner of a galley of `size` placed under `anchor`.
fn anchor_offset(anchor: Align2, size: Vec2) -> Vec2 {
    let x = match anchor.x() {
        egui::Align::Min => 0.0,
        egui::Align::Center => -size.x / 2.0,
        egui::Align::Max => -size.x,
    };
    let y = match anchor.y() {
        egui::Align::Min => 0.0,
        egui::Align::Center => -size.y / 2.0,
        egui::Align::Max => -size.y,
    };
    Vec2::new(x, y)
}
