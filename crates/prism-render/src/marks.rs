//! Drawing the individual marks a plot is built from.

use prism_doc::graph::{Color, ErrorBarStyle, ErrorBars, Symbol, SymbolShape};

use crate::scene::{pt, DrawCmd, Point, Scene, Stroke, Tag};

/// Emit one symbol centred on `at`.
pub fn symbol(scene: &mut Scene, tag: Tag, at: Point, sym: &Symbol) {
    if sym.shape == SymbolShape::None || sym.size <= 0.0 {
        return;
    }
    let r = sym.size / 2.0;
    let stroke = (sym.border_width > 0.0 && sym.border.a > 0)
        .then(|| Stroke::new(sym.border, sym.border_width));

    let poly = |points: Vec<Point>| DrawCmd::Polygon { points, fill: sym.fill, stroke: stroke.clone() };

    let cmd = match sym.shape {
        SymbolShape::None => return,
        SymbolShape::Circle => {
            DrawCmd::Circle { center: at, radius: r, fill: sym.fill, stroke: stroke.clone() }
        }
        SymbolShape::Square => poly(vec![
            pt(at.x - r, at.y - r),
            pt(at.x + r, at.y - r),
            pt(at.x + r, at.y + r),
            pt(at.x - r, at.y + r),
        ]),
        SymbolShape::Triangle => poly(vec![
            pt(at.x, at.y - r),
            pt(at.x + r, at.y + r * 0.8),
            pt(at.x - r, at.y + r * 0.8),
        ]),
        SymbolShape::TriangleDown => poly(vec![
            pt(at.x, at.y + r),
            pt(at.x + r, at.y - r * 0.8),
            pt(at.x - r, at.y - r * 0.8),
        ]),
        SymbolShape::Diamond => poly(vec![
            pt(at.x, at.y - r),
            pt(at.x + r, at.y),
            pt(at.x, at.y + r),
            pt(at.x - r, at.y),
        ]),
        SymbolShape::Plus => {
            let s = Stroke::new(sym.fill, sym.border_width.max(1.0));
            scene.push(tag, DrawCmd::Line { from: pt(at.x - r, at.y), to: pt(at.x + r, at.y), stroke: s.clone() });
            scene.push(tag, DrawCmd::Line { from: pt(at.x, at.y - r), to: pt(at.x, at.y + r), stroke: s });
            return;
        }
        SymbolShape::Cross => {
            let s = Stroke::new(sym.fill, sym.border_width.max(1.0));
            scene.push(tag, DrawCmd::Line { from: pt(at.x - r, at.y - r), to: pt(at.x + r, at.y + r), stroke: s.clone() });
            scene.push(tag, DrawCmd::Line { from: pt(at.x - r, at.y + r), to: pt(at.x + r, at.y - r), stroke: s });
            return;
        }
        SymbolShape::Star => {
            let mut points = Vec::with_capacity(10);
            for i in 0..10 {
                let a = std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let rad = if i % 2 == 0 { r } else { r * 0.45 };
                points.push(pt(at.x + rad * a.cos(), at.y - rad * a.sin()));
            }
            poly(points)
        }
        SymbolShape::Dash => {
            let s = Stroke::new(sym.fill, sym.size.max(1.0) * 0.35);
            scene.push(tag, DrawCmd::Line { from: pt(at.x - r, at.y), to: pt(at.x + r, at.y), stroke: s });
            return;
        }
        SymbolShape::HalfCircleLeft | SymbolShape::HalfCircleRight => {
            let right = sym.shape == SymbolShape::HalfCircleRight;
            let mut points = Vec::with_capacity(18);
            for i in 0..=16 {
                let t = i as f32 / 16.0 * std::f32::consts::PI;
                let a = if right { t - std::f32::consts::FRAC_PI_2 } else { t + std::f32::consts::FRAC_PI_2 };
                points.push(pt(at.x + r * a.cos(), at.y - r * a.sin()));
            }
            poly(points)
        }
    };
    scene.push(tag, cmd);
}

/// Emit a vertical error bar from `center` reaching `low` and `high` screen Y.
pub fn error_bar(
    scene: &mut Scene,
    tag: Tag,
    x: f32,
    center_y: f32,
    low_y: Option<f32>,
    high_y: Option<f32>,
    bars: &ErrorBars,
    horizontal: bool,
) {
    if bars.style == ErrorBarStyle::None || !bars.stroke.is_visible() {
        return;
    }
    let s: Stroke = (&bars.stroke).into();
    let cap = bars.cap_width / 2.0;

    let mut arm = |end: f32| {
        if horizontal {
            scene.push(tag, DrawCmd::Line { from: pt(center_y, x), to: pt(end, x), stroke: s.clone() });
            if cap > 0.0 {
                scene.push(tag, DrawCmd::Line { from: pt(end, x - cap), to: pt(end, x + cap), stroke: s.clone() });
            }
        } else {
            scene.push(tag, DrawCmd::Line { from: pt(x, center_y), to: pt(x, end), stroke: s.clone() });
            if cap > 0.0 {
                scene.push(tag, DrawCmd::Line { from: pt(x - cap, end), to: pt(x + cap, end), stroke: s.clone() });
            }
        }
    };

    // "Up" and "down" are in data space: up is the smaller screen Y.
    if matches!(bars.style, ErrorBarStyle::Both | ErrorBarStyle::Up) {
        if let Some(h) = high_y {
            arm(h);
        }
    }
    if matches!(bars.style, ErrorBarStyle::Both | ErrorBarStyle::Down) {
        if let Some(l) = low_y {
            arm(l);
        }
    }
}

/// Catmull-Rom spline through `points`, sampled into a polyline.
pub fn spline(points: &[Point], steps_per_span: usize) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(points.len() * steps_per_span);
    let n = points.len();
    for i in 0..n - 1 {
        let p0 = points[i.saturating_sub(1)];
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = points[(i + 2).min(n - 1)];
        for s in 0..steps_per_span {
            let t = s as f32 / steps_per_span as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * ((2.0 * b)
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push(pt(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y)));
        }
    }
    out.push(points[n - 1]);
    out
}

/// Blend `color` toward white by `t`, used for violin and area fills.
pub fn lighten(color: Color, t: f32) -> Color {
    let mix = |c: u8| (c as f32 + (255.0 - c as f32) * t).round() as u8;
    Color { r: mix(color.r), g: mix(color.g), b: mix(color.b), a: color.a }
}
