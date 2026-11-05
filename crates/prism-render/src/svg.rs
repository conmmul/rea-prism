//! Writing a [`Scene`] out as SVG.

use std::fmt::Write;

use prism_doc::graph::Color;

use crate::scene::{DrawCmd, HAlign, Point, Scene, Stroke, VAlign};

/// Render a scene to a standalone SVG document.
pub fn to_svg(scene: &Scene) -> String {
    let (w, h) = scene.size;
    let mut s = String::with_capacity(4096);
    writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.2}pt" height="{h:.2}pt" viewBox="0 0 {w:.2} {h:.2}">"#
    )
    .unwrap();
    writeln!(s, r##"<rect width="{w:.2}" height="{h:.2}" fill="#ffffff"/>"##).unwrap();

    for tagged in scene.iter() {
        write_cmd(&mut s, &tagged.cmd);
    }

    s.push_str("</svg>\n");
    s
}

fn paint(c: Color) -> String {
    if c.a == 0 {
        "none".into()
    } else {
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    }
}

fn opacity(c: Color) -> String {
    if c.a == 255 || c.a == 0 {
        String::new()
    } else {
        format!(r#" opacity="{:.3}""#, c.a as f32 / 255.0)
    }
}

fn stroke_attrs(s: &Option<Stroke>) -> String {
    match s {
        None => r#" stroke="none""#.into(),
        Some(s) => {
            let mut out = format!(
                r#" stroke="{}" stroke-width="{:.2}""#,
                paint(s.color),
                s.width
            );
            if let Some(d) = s.dash_array() {
                let parts: Vec<String> = d.iter().map(|v| format!("{v:.2}")).collect();
                out.push_str(&format!(r#" stroke-dasharray="{}""#, parts.join(",")));
            }
            out.push_str(r#" stroke-linecap="butt" stroke-linejoin="round""#);
            out
        }
    }
}

fn points_attr(points: &[Point]) -> String {
    points
        .iter()
        .map(|p| format!("{:.2},{:.2}", p.x, p.y))
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_cmd(s: &mut String, cmd: &DrawCmd) {
    match cmd {
        DrawCmd::Line { from, to, stroke } => {
            writeln!(
                s,
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}"{}/>"#,
                from.x,
                from.y,
                to.x,
                to.y,
                stroke_attrs(&Some(stroke.clone()))
            )
            .unwrap();
        }
        DrawCmd::Polyline { points, stroke, close } => {
            let tag = if *close { "polygon" } else { "polyline" };
            writeln!(
                s,
                r#"<{tag} points="{}" fill="none"{}/>"#,
                points_attr(points),
                stroke_attrs(&Some(stroke.clone()))
            )
            .unwrap();
        }
        DrawCmd::Polygon { points, fill, stroke } => {
            writeln!(
                s,
                r#"<polygon points="{}" fill="{}"{}{}/>"#,
                points_attr(points),
                paint(*fill),
                opacity(*fill),
                stroke_attrs(stroke)
            )
            .unwrap();
        }
        DrawCmd::Rect { rect, fill, stroke } => {
            writeln!(
                s,
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}"{}{}/>"#,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                paint(*fill),
                opacity(*fill),
                stroke_attrs(stroke)
            )
            .unwrap();
        }
        DrawCmd::Circle { center, radius, fill, stroke } => {
            writeln!(
                s,
                r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}"{}{}/>"#,
                center.x,
                center.y,
                radius,
                paint(*fill),
                opacity(*fill),
                stroke_attrs(stroke)
            )
            .unwrap();
        }
        DrawCmd::Wedge { center, radius, inner_radius, start_angle, sweep, fill, stroke } => {
            writeln!(s, r#"<path d="{}" fill="{}"{}{}/>"#,
                wedge_path(*center, *radius, *inner_radius, *start_angle, *sweep),
                paint(*fill),
                opacity(*fill),
                stroke_attrs(stroke)
            )
            .unwrap();
        }
        DrawCmd::Text(t) => {
            let anchor = match t.h {
                HAlign::Left => "start",
                HAlign::Center => "middle",
                HAlign::Right => "end",
            };
            let baseline = match t.v {
                VAlign::Top => "hanging",
                VAlign::Middle => "central",
                VAlign::Baseline => "alphabetic",
                VAlign::Bottom => "auto",
            };
            let transform = if t.rotation.abs() > f32::EPSILON {
                format!(
                    r#" transform="rotate({:.2} {:.2} {:.2})""#,
                    -t.rotation, t.at.x, t.at.y
                )
            } else {
                String::new()
            };
            writeln!(
                s,
                r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.2}" fill="{}" text-anchor="{anchor}" dominant-baseline="{baseline}"{}{}{}>{}</text>"#,
                t.at.x,
                t.at.y,
                escape(&t.family),
                t.size,
                paint(t.color),
                if t.bold { r#" font-weight="bold""# } else { "" },
                if t.italic { r#" font-style="italic""# } else { "" },
                transform,
                escape(&t.text)
            )
            .unwrap();
        }
    }
}

/// An SVG path for an annular sector. Angles are degrees counter-clockwise from
/// east, and `sweep` is negative for a clockwise slice.
fn wedge_path(center: Point, r: f32, inner: f32, start: f32, sweep: f32) -> String {
    let p = |angle: f32, radius: f32| {
        let a = angle.to_radians();
        (center.x + radius * a.cos(), center.y - radius * a.sin())
    };
    let end = start + sweep;
    let large = if sweep.abs() > 180.0 { 1 } else { 0 };
    // SVG sweeps clockwise when the flag is 1; our angles grow counter-clockwise.
    let dir = if sweep < 0.0 { 1 } else { 0 };

    let (x0, y0) = p(start, r);
    let (x1, y1) = p(end, r);

    if inner <= 0.0 {
        format!(
            "M {:.2} {:.2} L {x0:.2} {y0:.2} A {r:.2} {r:.2} 0 {large} {dir} {x1:.2} {y1:.2} Z",
            center.x, center.y
        )
    } else {
        let (ix1, iy1) = p(end, inner);
        let (ix0, iy0) = p(start, inner);
        let rev = 1 - dir;
        format!(
            "M {x0:.2} {y0:.2} A {r:.2} {r:.2} 0 {large} {dir} {x1:.2} {y1:.2} \
             L {ix1:.2} {iy1:.2} A {inner:.2} {inner:.2} 0 {large} {rev} {ix0:.2} {iy0:.2} Z"
        )
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
