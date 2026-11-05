//! Drawing axes, ticks, tick labels and the frame.

use prism_doc::graph::{Axis, Frame, GraphSpec, Scale, TickDirection};

use crate::layout::Layout;
use crate::scale::{self, AxisScale};
use crate::scene::{pt, DrawCmd, HAlign, Rect, Scene, Stroke, Tag, TextRun, VAlign};
use crate::text;

const TITLE_GAP: f32 = 6.0;

pub fn draw(scene: &mut Scene, spec: &GraphSpec, layout: &Layout) {
    let plot = layout.plot;

    if spec.plot_background.a > 0 {
        scene.push(
            Tag::Background,
            DrawCmd::Rect { rect: plot, fill: spec.plot_background, stroke: None },
        );
    }

    draw_grid(scene, &spec.x, &layout.x_scale, plot, true);
    draw_grid(scene, &spec.y, &layout.y_scale, plot, false);

    match spec.frame {
        Frame::Box => {
            scene.push(
                Tag::Axis,
                DrawCmd::Rect {
                    rect: plot,
                    fill: prism_doc::graph::Color::TRANSPARENT,
                    stroke: Some(Stroke::new(spec.axis_color, spec.axis_width)),
                },
            );
        }
        Frame::PlainAxes | Frame::Offset => {
            let off = if spec.frame == Frame::Offset { 6.0 } else { 0.0 };
            if spec.x.visible {
                scene.push(
                    Tag::Axis,
                    DrawCmd::Line {
                        from: pt(plot.x, plot.bottom() + off),
                        to: pt(plot.right(), plot.bottom() + off),
                        stroke: Stroke::new(spec.axis_color, spec.axis_width),
                    },
                );
            }
            if spec.y.visible {
                scene.push(
                    Tag::Axis,
                    DrawCmd::Line {
                        from: pt(plot.x - off, plot.y),
                        to: pt(plot.x - off, plot.bottom()),
                        stroke: Stroke::new(spec.axis_color, spec.axis_width),
                    },
                );
            }
        }
    }

    draw_x_ticks(scene, &spec.x, &layout.x_scale, plot);
    draw_y_ticks(scene, &spec.y, &layout.y_scale, plot, false);
    if let (Some(a), Some(s)) = (&spec.y2, &layout.y2_scale) {
        draw_y_ticks(scene, a, s, plot, true);
    }

    if spec.title.visible && !spec.title.text.is_empty() {
        scene.push(
            Tag::Title,
            DrawCmd::Text(TextRun {
                text: spec.title.text.clone(),
                at: pt(plot.center().x, plot.y - TITLE_GAP),
                size: spec.title.style.size,
                family: spec.title.style.family.clone(),
                color: spec.title.style.color,
                bold: spec.title.style.bold,
                italic: spec.title.style.italic,
                h: HAlign::Center,
                v: VAlign::Bottom,
                rotation: 0.0,
            }),
        );
    }
}

fn draw_grid(scene: &mut Scene, axis: &Axis, s: &AxisScale, plot: Rect, vertical: bool) {
    if !axis.grid {
        return;
    }
    for t in scale::ticks(axis, s).iter().filter(|t| t.major || axis.minor_grid) {
        let mut stroke = Stroke::new(axis.grid_color, axis.grid_width.max(0.1));
        stroke.style = axis.grid_style;
        if !t.major {
            stroke.width *= 0.6;
        }
        if vertical {
            let x = s.to_screen(t.value);
            scene.push(
                Tag::Grid,
                DrawCmd::Line { from: pt(x, plot.y), to: pt(x, plot.bottom()), stroke },
            );
        } else {
            let y = s.to_screen(t.value);
            scene.push(
                Tag::Grid,
                DrawCmd::Line { from: pt(plot.x, y), to: pt(plot.right(), y), stroke },
            );
        }
    }
}

/// Tick mark ends, relative to the axis line, for the configured direction.
fn tick_span(dir: TickDirection, len: f32) -> (f32, f32) {
    match dir {
        TickDirection::Out => (0.0, len),
        TickDirection::In => (-len, 0.0),
        TickDirection::Both => (-len, len),
        TickDirection::None => (0.0, 0.0),
    }
}

fn draw_x_ticks(scene: &mut Scene, axis: &Axis, s: &AxisScale, plot: Rect) {
    if !axis.visible {
        return;
    }
    let stroke = Stroke::new(axis.color, axis.line_width);
    let (a, b) = tick_span(axis.tick_direction, axis.tick_length);
    let mut label_y = plot.bottom() + axis.tick_length.max(0.0) + axis.numbering_distance;

    for t in scale::ticks(axis, s) {
        let x = s.to_screen(t.value);
        if x < plot.x - 0.5 || x > plot.right() + 0.5 {
            continue;
        }
        let len = if t.major { 1.0 } else { 0.6 };
        if axis.tick_direction != TickDirection::None && axis.scale != Scale::Categorical {
            scene.push(
                Tag::Axis,
                DrawCmd::Line {
                    from: pt(x, plot.bottom() + a * len),
                    to: pt(x, plot.bottom() + b * len),
                    stroke: stroke.clone(),
                },
            );
        }
        if let Some(label) = &t.label {
            let rot = axis.label_rotation;
            scene.push(
                Tag::Axis,
                DrawCmd::Text(TextRun {
                    text: label.clone(),
                    at: pt(x, label_y),
                    size: axis.label_style.size,
                    family: axis.label_style.family.clone(),
                    color: axis.label_style.color,
                    bold: axis.label_style.bold,
                    italic: axis.label_style.italic,
                    h: if rot.abs() > 1.0 { HAlign::Right } else { HAlign::Center },
                    v: if rot.abs() > 1.0 { VAlign::Middle } else { VAlign::Top },
                    rotation: rot,
                }),
            );
        }
    }

    if axis.title.visible && !axis.title.text.is_empty() {
        let ticks = scale::ticks(axis, s);
        let label_h = if axis.label_rotation.abs() > 1.0 {
            ticks
                .iter()
                .filter_map(|t| t.label.as_ref())
                .map(|l| text::width(l, axis.label_style.size))
                .fold(0.0, f32::max)
                * axis.label_rotation.to_radians().sin().abs()
        } else if ticks.iter().any(|t| t.label.is_some()) {
            text::line_height(axis.label_style.size)
        } else {
            0.0
        };
        label_y += label_h + axis.title_distance;
        scene.push(
            Tag::Axis,
            DrawCmd::Text(TextRun {
                text: axis.title.text.clone(),
                at: pt(plot.center().x, label_y),
                size: axis.title.style.size,
                family: axis.title.style.family.clone(),
                color: axis.title.style.color,
                bold: axis.title.style.bold,
                italic: axis.title.style.italic,
                h: HAlign::Center,
                v: VAlign::Top,
                rotation: 0.0,
            }),
        );
    }
}

fn draw_y_ticks(scene: &mut Scene, axis: &Axis, s: &AxisScale, plot: Rect, right: bool) {
    if !axis.visible {
        return;
    }
    let stroke = Stroke::new(axis.color, axis.line_width);
    let (a, b) = tick_span(axis.tick_direction, axis.tick_length);
    let axis_x = if right { plot.right() } else { plot.x };
    let dir = if right { 1.0 } else { -1.0 };
    let mut widest: f32 = 0.0;

    for t in scale::ticks(axis, s) {
        let y = s.to_screen(t.value);
        if y < plot.y - 0.5 || y > plot.bottom() + 0.5 {
            continue;
        }
        let len = if t.major { 1.0 } else { 0.6 };
        if axis.tick_direction != TickDirection::None {
            scene.push(
                Tag::Axis,
                DrawCmd::Line {
                    from: pt(axis_x - a * len * dir, y),
                    to: pt(axis_x - b * len * dir, y),
                    stroke: stroke.clone(),
                },
            );
        }
        if let Some(label) = &t.label {
            widest = widest.max(text::width(label, axis.label_style.size));
            scene.push(
                Tag::Axis,
                DrawCmd::Text(TextRun {
                    text: label.clone(),
                    at: pt(axis_x + dir * (axis.tick_length.max(0.0) + axis.numbering_distance), y),
                    size: axis.label_style.size,
                    family: axis.label_style.family.clone(),
                    color: axis.label_style.color,
                    bold: axis.label_style.bold,
                    italic: axis.label_style.italic,
                    h: if right { HAlign::Left } else { HAlign::Right },
                    v: VAlign::Middle,
                    rotation: 0.0,
                }),
            );
        }
    }

    if axis.title.visible && !axis.title.text.is_empty() {
        let x = axis_x
            + dir
                * (axis.tick_length.max(0.0)
                    + axis.numbering_distance
                    + widest
                    + axis.title_distance);
        scene.push(
            Tag::Axis,
            DrawCmd::Text(TextRun {
                text: axis.title.text.clone(),
                at: pt(x, plot.center().y),
                size: axis.title.style.size,
                family: axis.title.style.family.clone(),
                color: axis.title.style.color,
                bold: axis.title.style.bold,
                italic: axis.title.style.italic,
                h: HAlign::Center,
                v: if right { VAlign::Top } else { VAlign::Bottom },
                // Prism writes the left Y title reading upward.
                rotation: 90.0,
            }),
        );
    }
}
