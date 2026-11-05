//! Drawing the key that names each plotted series.

use prism_doc::graph::{GraphSpec, LegendPosition, PlotKind};

use crate::layout::Layout;
use crate::marks;
use crate::scene::{pt, DrawCmd, HAlign, Rect, Scene, Stroke, Tag, TextRun, VAlign};
use crate::text;

pub fn draw(scene: &mut Scene, spec: &GraphSpec, layout: &Layout) {
    let (Some(area), true) = (layout.legend, spec.legend.position != LegendPosition::None) else {
        return;
    };
    let style = &spec.legend.style;
    let lh = text::line_height(style.size);
    let swatch = style.size * 1.6;

    if spec.legend.frame {
        scene.push(
            Tag::Legend,
            DrawCmd::Rect {
                rect: area,
                fill: prism_doc::graph::Color::TRANSPARENT,
                stroke: Some(Stroke::new(style.color, 0.5)),
            },
        );
    }

    let horizontal = matches!(spec.legend.position, LegendPosition::Above | LegendPosition::Below);
    let mut x = area.x + 3.0;
    let mut y = area.y + 3.0;

    for s in spec.series.iter().filter(|s| s.visible) {
        let cy = y + lh / 2.0;

        // The key mirrors the mark the plot actually draws.
        match &spec.kind {
            PlotKind::Bar { .. }
            | PlotKind::BoxWhisker { .. }
            | PlotKind::Violin { .. }
            | PlotKind::PartsOfWhole { .. }
            | PlotKind::Heatmap { .. } => {
                scene.push(
                    Tag::Legend,
                    DrawCmd::Rect {
                        rect: Rect::new(x, cy - swatch / 3.0, swatch, swatch * 0.66),
                        fill: s.fill,
                        stroke: s.border.is_visible().then(|| (&s.border).into()),
                    },
                );
            }
            PlotKind::XY { connect: true, .. } | PlotKind::Survival { .. } => {
                scene.push(
                    Tag::Legend,
                    DrawCmd::Line {
                        from: pt(x, cy),
                        to: pt(x + swatch, cy),
                        stroke: (&s.line).into(),
                    },
                );
                marks::symbol(scene, Tag::Legend, pt(x + swatch / 2.0, cy), &s.symbol);
            }
            _ => marks::symbol(scene, Tag::Legend, pt(x + swatch / 2.0, cy), &s.symbol),
        }

        scene.push(
            Tag::Legend,
            DrawCmd::Text(TextRun {
                text: s.label.clone(),
                at: pt(x + swatch + 5.0, cy),
                size: style.size,
                family: style.family.clone(),
                color: style.color,
                bold: style.bold,
                italic: style.italic,
                h: HAlign::Left,
                v: VAlign::Middle,
                rotation: 0.0,
            }),
        );

        if horizontal {
            x += swatch + 5.0 + text::width(&s.label, style.size) + 12.0;
        } else {
            y += lh;
        }
    }
}
