//! Geometry for each of Prism's graph families.

use prism_doc::graph::{
    BarLayout, BoxWhiskerStyle, Center, Color, Colormap, GraphSpec, Jitter, PlotKind, Series,
    SlotSource, Spread,
};
use prism_doc::model::DataTable;

use crate::layout::Layout;
use crate::marks;
use crate::scene::{pt, DrawCmd, HAlign, Point, Rect, Scene, Stroke, Tag, TextRun, VAlign};
use crate::series;

pub fn draw(scene: &mut Scene, spec: &GraphSpec, table: &DataTable, layout: &Layout) {
    match &spec.kind {
        PlotKind::XY { connect, area, spline } => {
            xy(scene, spec, table, layout, *connect, *area, *spline)
        }
        PlotKind::Bar { layout: bl, horizontal, center, spread } => {
            bars(scene, spec, table, layout, *bl, *horizontal, *center, *spread)
        }
        PlotKind::ScatterDot { jitter, center, spread } => {
            scatter_dot(scene, spec, table, layout, *jitter, *center, *spread)
        }
        PlotKind::BoxWhisker { style, horizontal, show_points } => {
            box_whisker(scene, spec, table, layout, *style, *horizontal, *show_points)
        }
        PlotKind::Violin { show_quartiles, horizontal } => {
            violin(scene, spec, table, layout, *show_quartiles, *horizontal)
        }
        PlotKind::Survival { show_censor_marks, as_percent } => {
            survival(scene, spec, table, layout, *show_censor_marks, *as_percent)
        }
        PlotKind::PartsOfWhole { donut_hole, start_angle, explode } => {
            parts_of_whole(scene, spec, table, layout, *donut_hole, *start_angle, explode)
        }
        PlotKind::Heatmap { colormap, show_values } => {
            heatmap(scene, spec, table, layout, colormap, *show_values)
        }
        PlotKind::Bubble { size_variable, color_variable, colormap } => {
            bubble(scene, spec, table, layout, *size_variable, *color_variable, colormap)
        }
        PlotKind::Dendrogram { horizontal } => dendrogram(scene, spec, table, layout, *horizontal),
    }
}

/// The error half-widths a table stores alongside a mean, when its format has them.
fn stored_error_at(table: &DataTable, dataset: usize, row: usize) -> Option<(f64, f64)> {
    let pts = series::xy_points(table, dataset, Center::Mean, Spread::FromTable);
    let p = pts.iter().find(|p| p.row == row)?;
    Some((p.err_low?, p.err_high?))
}

fn visible(spec: &GraphSpec) -> impl Iterator<Item = (usize, &Series)> {
    spec.series.iter().enumerate().filter(|(_, s)| s.visible)
}

/// The Y scale a series plots against.
fn y_scale<'a>(layout: &'a Layout, s: &Series) -> &'a crate::scale::AxisScale {
    match (&layout.y2_scale, s.secondary_axis) {
        (Some(y2), true) => y2,
        _ => &layout.y_scale,
    }
}

// ---------------------------------------------------------------------------
// XY
// ---------------------------------------------------------------------------

fn xy(
    scene: &mut Scene,
    spec: &GraphSpec,
    table: &DataTable,
    layout: &Layout,
    connect: bool,
    area: bool,
    use_spline: bool,
) {
    for (i, s) in visible(spec) {
        let ys = y_scale(layout, s);
        let points = series::xy_points(table, s.dataset, Center::Mean, Spread::FromTable);
        let screen: Vec<Point> = points
            .iter()
            .filter(|p| layout.x_scale.is_valid(p.x) && ys.is_valid(p.y))
            .map(|p| pt(layout.x_scale.to_screen(p.x), ys.to_screen(p.y)))
            .collect();
        if screen.is_empty() {
            continue;
        }
        let tag = Tag::Series(i);

        let path = if use_spline { marks::spline(&screen, 12) } else { screen.clone() };

        if area {
            let baseline = ys.to_screen(ys.to_data(ys.screen_min).max(0.0).min(0.0));
            let mut poly = path.clone();
            poly.push(pt(path[path.len() - 1].x, baseline));
            poly.push(pt(path[0].x, baseline));
            scene.push(
                tag,
                DrawCmd::Polygon {
                    points: poly,
                    fill: marks::lighten(s.fill, 0.55).with_alpha(200),
                    stroke: None,
                },
            );
        }

        if connect && s.line.is_visible() && path.len() > 1 {
            scene.push(
                tag,
                DrawCmd::Polyline { points: path, stroke: (&s.line).into(), close: false },
            );
        }

        for p in &points {
            if !layout.x_scale.is_valid(p.x) || !ys.is_valid(p.y) {
                continue;
            }
            let x = layout.x_scale.to_screen(p.x);
            let y = ys.to_screen(p.y);
            marks::error_bar(
                scene,
                tag,
                x,
                y,
                p.err_low.map(|e| ys.to_screen(p.y - e)),
                p.err_high.map(|e| ys.to_screen(p.y + e)),
                &s.error_bars,
                false,
            );
            marks::symbol(scene, tag, pt(x, y), &s.symbol);
        }
    }
}

// ---------------------------------------------------------------------------
// Bars
// ---------------------------------------------------------------------------

/// Geometry of one slot along the categorical axis.
struct Slots {
    /// Screen width of one category slot.
    width: f32,
    /// Width of a single bar within its slot.
    bar: f32,
}

fn slots(spec: &GraphSpec, layout: &Layout, bars_per_slot: usize) -> Slots {
    let width = layout.x_scale.category_width();
    let usable = width * (1.0 - spec.group_gap);
    let n = bars_per_slot.max(1) as f32;
    let bar = (usable / n) * (1.0 - spec.bar_gap);
    Slots { width, bar }
}

fn bars(
    scene: &mut Scene,
    spec: &GraphSpec,
    table: &DataTable,
    layout: &Layout,
    bar_layout: BarLayout,
    horizontal: bool,
    center: Center,
    spread: Spread,
) {
    let series: Vec<(usize, &Series)> = visible(spec).collect();
    if series.is_empty() {
        return;
    }
    let rows = table.values.rows();

    // A Grouped table draws one slot per row and one bar per dataset; a Column
    // table draws one slot per dataset with a single bar in it.
    let per_slot = match bar_layout {
        BarLayout::Stacked | BarLayout::StackedPercent => 1,
        BarLayout::Separated => 1,
        BarLayout::Interleaved => series.len(),
    };
    let geom = slots(spec, layout, per_slot);
    let ys = &layout.y_scale;
    let baseline = ys.to_screen(0.0_f64.clamp(ys.to_data(ys.screen_min), ys.to_data(ys.screen_max)));

    let slot_count = match spec.slots {
        SlotSource::Datasets => series.len(),
        SlotSource::Rows => rows,
    };

    // Running totals for the stacked layouts.
    let mut stack_pos = vec![0.0_f64; slot_count.max(1)];
    let mut stack_neg = vec![0.0_f64; slot_count.max(1)];

    let row_totals: Vec<f64> = (0..slot_count)
        .map(|slot| {
            series
                .iter()
                .filter_map(|(_, s)| summary_value(table, s.dataset, slot, spec.slots, center))
                .map(f64::abs)
                .sum()
        })
        .collect();

    for (si, (i, s)) in series.iter().enumerate() {
        let tag = Tag::Series(*i);
        for slot in 0..slot_count {
            let Some(mut v) = summary_value(table, s.dataset, slot, spec.slots, center) else {
                continue;
            };
            if bar_layout == BarLayout::StackedPercent {
                let total = row_totals[slot];
                if total.abs() < f64::EPSILON {
                    continue;
                }
                v = v / total * 100.0;
            }

            let slot_left = layout.x_scale.to_screen(slot as f64) + geom.width * spec.group_gap / 2.0;
            let x0 = match bar_layout {
                BarLayout::Interleaved => {
                    slot_left + si as f32 * (geom.bar / (1.0 - spec.bar_gap))
                        + geom.bar * spec.bar_gap / 2.0
                }
                _ => slot_left + (geom.width * (1.0 - spec.group_gap) - geom.bar) / 2.0,
            };

            let (base_v, top_v) = match bar_layout {
                BarLayout::Stacked | BarLayout::StackedPercent => {
                    let acc = if v >= 0.0 { &mut stack_pos } else { &mut stack_neg };
                    let b = acc[slot];
                    acc[slot] += v;
                    (b, b + v)
                }
                _ => (0.0, v),
            };

            let y_top = ys.to_screen(top_v.max(base_v));
            let y_bot = ys.to_screen(top_v.min(base_v));
            let rect = if horizontal {
                Rect::new(
                    baseline.min(y_top),
                    x0,
                    (y_bot - y_top).abs().max(0.5),
                    geom.bar.max(1.0),
                )
            } else {
                Rect::new(x0, y_top, geom.bar.max(1.0), (y_bot - y_top).abs().max(0.5))
            };

            scene.push(
                tag,
                DrawCmd::Rect {
                    rect,
                    fill: s.fill,
                    stroke: s.border.is_visible().then(|| (&s.border).into()),
                },
            );

            // Error bars only make sense on the outermost segment of a stack.
            if !matches!(bar_layout, BarLayout::Stacked | BarLayout::StackedPercent) {
                if let Some((lo, hi)) = error_value(table, s.dataset, slot, spec.slots, spread) {
                    let cx = x0 + geom.bar / 2.0;
                    marks::error_bar(
                        scene,
                        tag,
                        cx,
                        ys.to_screen(v),
                        Some(ys.to_screen(v - lo)),
                        Some(ys.to_screen(v + hi)),
                        &s.error_bars,
                        horizontal,
                    );
                }
            }
        }
    }
}

/// The value a column-style plot shows for `slot`.
///
/// With one slot per dataset the slot *is* the dataset and the value summarises
/// its whole column. With one slot per row the slot is the row, and the value
/// summarises that row's replicates for this dataset.
fn summary_value(
    table: &DataTable,
    dataset: usize,
    slot: usize,
    slots: SlotSource,
    center: Center,
) -> Option<f64> {
    let values = match slots {
        SlotSource::Rows => series::replicates(table, dataset, slot),
        SlotSource::Datasets => series::column_values(table, slot),
    };
    if values.is_empty() {
        return None;
    }
    let s = series::summarize(&values);
    Some(match center {
        Center::Median => s.median,
        Center::GeometricMean => {
            let pos: Vec<f64> = values.iter().copied().filter(|v| *v > 0.0).collect();
            if pos.is_empty() {
                return None;
            }
            (pos.iter().map(|v| v.ln()).sum::<f64>() / pos.len() as f64).exp()
        }
        _ => s.mean,
    })
}

fn error_value(
    table: &DataTable,
    dataset: usize,
    slot: usize,
    slots: SlotSource,
    spread: Spread,
) -> Option<(f64, f64)> {
    if spread == Spread::None {
        return None;
    }
    // A table that stores its own error values uses them directly.
    if spread == Spread::FromTable {
        let row = match slots {
            SlotSource::Rows => slot,
            SlotSource::Datasets => 0,
        };
        if let Some(e) = stored_error_at(table, dataset, row) {
            return Some(e);
        }
    }
    let values = match slots {
        SlotSource::Rows => series::replicates(table, dataset, slot),
        SlotSource::Datasets => series::column_values(table, slot),
    };
    let s = series::summarize(&values);
    if s.n < 2 {
        return None;
    }
    Some(match spread {
        Spread::Sem => (s.sem(), s.sem()),
        Spread::Ci => {
            let h = s.ci_half_width(0.95);
            (h, h)
        }
        Spread::Range => (s.mean - s.min, s.max - s.mean),
        _ => (s.sd, s.sd),
    })
}

// ---------------------------------------------------------------------------
// Column scatter
// ---------------------------------------------------------------------------

fn scatter_dot(
    scene: &mut Scene,
    spec: &GraphSpec,
    table: &DataTable,
    layout: &Layout,
    jitter: Jitter,
    center: Center,
    spread: Spread,
) {
    let ys = &layout.y_scale;
    let slot_w = layout.x_scale.category_width();

    for (i, s) in visible(spec) {
        let tag = Tag::Series(i);
        let values = series::column_values(table, s.dataset);
        if values.is_empty() {
            continue;
        }
        let cx = layout.x_scale.category_center(i);
        let spread_w = slot_w * 0.35;

        // Offset coincident points so they stay distinguishable, the way Prism's
        // aligned scatter does.
        let mut seen: Vec<(f32, usize)> = Vec::new();
        for (k, v) in values.iter().enumerate() {
            if !ys.is_valid(*v) {
                continue;
            }
            let y = ys.to_screen(*v);
            let dx = match jitter {
                Jitter::None => 0.0,
                Jitter::Random => {
                    // Deterministic pseudo-jitter so repeated renders match.
                    let h = ((k as u32).wrapping_mul(2654435761)) as f32 / u32::MAX as f32;
                    (h - 0.5) * spread_w
                }
                Jitter::Aligned => {
                    let slot = seen.iter_mut().find(|(sy, _)| (*sy - y).abs() < s.symbol.size * 0.9);
                    let n = match slot {
                        Some((_, n)) => {
                            *n += 1;
                            *n
                        }
                        None => {
                            seen.push((y, 1));
                            1
                        }
                    };
                    // 1 -> 0, 2 -> +1, 3 -> -1, 4 -> +2, ...
                    let step = (n as i32) / 2;
                    let sign = if n % 2 == 0 { 1.0 } else { -1.0 };
                    sign * step as f32 * s.symbol.size * 0.9
                }
            };
            marks::symbol(scene, tag, pt(cx + dx, y), &s.symbol);
        }

        let stats = series::summarize(&values);
        let c = match center {
            Center::None => continue,
            Center::Median => stats.median,
            Center::GeometricMean => {
                let pos: Vec<f64> = values.iter().copied().filter(|v| *v > 0.0).collect();
                if pos.is_empty() {
                    continue;
                }
                (pos.iter().map(|v| v.ln()).sum::<f64>() / pos.len() as f64).exp()
            }
            Center::Mean => stats.mean,
        };
        let cy = ys.to_screen(c);
        let half = slot_w * 0.25;
        scene.push(
            tag,
            DrawCmd::Line {
                from: pt(cx - half, cy),
                to: pt(cx + half, cy),
                stroke: Stroke::new(s.line.color, s.line.width.max(1.5)),
            },
        );

        if spread != Spread::None && stats.n > 1 {
            let (lo, hi) = match spread {
                Spread::Sem => (stats.sem(), stats.sem()),
                Spread::Ci => {
                    let h = stats.ci_half_width(0.95);
                    (h, h)
                }
                Spread::Range => (c - stats.min, stats.max - c),
                _ => (stats.sd, stats.sd),
            };
            marks::error_bar(
                scene,
                tag,
                cx,
                cy,
                Some(ys.to_screen(c - lo)),
                Some(ys.to_screen(c + hi)),
                &s.error_bars,
                false,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Box, whisker and violin
// ---------------------------------------------------------------------------

fn box_whisker(
    scene: &mut Scene,
    spec: &GraphSpec,
    table: &DataTable,
    layout: &Layout,
    style: BoxWhiskerStyle,
    _horizontal: bool,
    show_points: bool,
) {
    let ys = &layout.y_scale;
    let slot_w = layout.x_scale.category_width();

    for (i, s) in visible(spec) {
        let tag = Tag::Series(i);
        let values = series::column_values(table, s.dataset);
        if values.is_empty() {
            continue;
        }
        let st = series::summarize(&values);
        let cx = layout.x_scale.category_center(i);
        let w = slot_w * 0.5;

        let (lo_w, hi_w) = match style {
            BoxWhiskerStyle::MinMax => (st.min, st.max),
            BoxWhiskerStyle::Percentile10_90 => {
                let mut v = values.clone();
                v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                (series::quantile(&v, 0.10), series::quantile(&v, 0.90))
            }
            BoxWhiskerStyle::Percentile5_95 => {
                let mut v = values.clone();
                v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                (series::quantile(&v, 0.05), series::quantile(&v, 0.95))
            }
            BoxWhiskerStyle::MeanSd => (st.mean - st.sd, st.mean + st.sd),
            BoxWhiskerStyle::Tukey => {
                let fence = 1.5 * st.iqr();
                let lo = values
                    .iter()
                    .copied()
                    .filter(|v| *v >= st.q1 - fence)
                    .fold(f64::INFINITY, f64::min);
                let hi = values
                    .iter()
                    .copied()
                    .filter(|v| *v <= st.q3 + fence)
                    .fold(f64::NEG_INFINITY, f64::max);
                (lo, hi)
            }
        };

        let (body_lo, body_hi) = if style == BoxWhiskerStyle::MeanSd {
            (st.mean - st.sd, st.mean + st.sd)
        } else {
            (st.q1, st.q3)
        };

        let y_hi = ys.to_screen(body_hi);
        let y_lo = ys.to_screen(body_lo);
        scene.push(
            tag,
            DrawCmd::Rect {
                rect: Rect::new(cx - w / 2.0, y_hi, w, (y_lo - y_hi).abs().max(1.0)),
                fill: s.fill,
                stroke: Some((&s.border).into()),
            },
        );

        let stroke: Stroke = (&s.border).into();
        // Median line across the body.
        let med = ys.to_screen(if style == BoxWhiskerStyle::MeanSd { st.mean } else { st.median });
        scene.push(
            tag,
            DrawCmd::Line {
                from: pt(cx - w / 2.0, med),
                to: pt(cx + w / 2.0, med),
                stroke: Stroke::new(stroke.color, stroke.width.max(1.5)),
            },
        );

        for (from_v, to_v) in [(body_hi, hi_w), (body_lo, lo_w)] {
            if !to_v.is_finite() {
                continue;
            }
            let a = ys.to_screen(from_v);
            let b = ys.to_screen(to_v);
            scene.push(tag, DrawCmd::Line { from: pt(cx, a), to: pt(cx, b), stroke: stroke.clone() });
            scene.push(
                tag,
                DrawCmd::Line {
                    from: pt(cx - w / 4.0, b),
                    to: pt(cx + w / 4.0, b),
                    stroke: stroke.clone(),
                },
            );
        }

        // Tukey style draws the points beyond the fences individually.
        if style == BoxWhiskerStyle::Tukey || show_points {
            for v in &values {
                let outside = *v < lo_w || *v > hi_w;
                if (style == BoxWhiskerStyle::Tukey && outside) || show_points {
                    marks::symbol(scene, tag, pt(cx, ys.to_screen(*v)), &s.symbol);
                }
            }
        }
    }
}

fn violin(
    scene: &mut Scene,
    spec: &GraphSpec,
    table: &DataTable,
    layout: &Layout,
    show_quartiles: bool,
    _horizontal: bool,
) {
    let ys = &layout.y_scale;
    let slot_w = layout.x_scale.category_width();

    for (i, s) in visible(spec) {
        let tag = Tag::Series(i);
        let values = series::column_values(table, s.dataset);
        if values.len() < 2 {
            continue;
        }
        let st = series::summarize(&values);
        let cx = layout.x_scale.category_center(i);
        let half = slot_w * 0.4;

        // Gaussian kernel density, with Silverman's rule for the bandwidth.
        let n = values.len() as f64;
        let bw = 1.06 * st.sd * n.powf(-0.2);
        let bw = if bw > 0.0 { bw } else { (st.max - st.min).abs().max(1e-9) / 10.0 };
        const STEPS: usize = 64;
        let mut density = Vec::with_capacity(STEPS + 1);
        let mut peak: f64 = 0.0;
        for k in 0..=STEPS {
            let y = st.min + (st.max - st.min) * k as f64 / STEPS as f64;
            let d: f64 = values
                .iter()
                .map(|v| {
                    let u = (y - v) / bw;
                    (-0.5 * u * u).exp()
                })
                .sum::<f64>()
                / (n * bw * (2.0 * std::f64::consts::PI).sqrt());
            peak = peak.max(d);
            density.push((y, d));
        }
        if peak <= 0.0 {
            continue;
        }

        let mut left = Vec::with_capacity(density.len());
        let mut right = Vec::with_capacity(density.len());
        for (y, d) in &density {
            let w = (d / peak) as f32 * half;
            let sy = ys.to_screen(*y);
            left.push(pt(cx - w, sy));
            right.push(pt(cx + w, sy));
        }
        right.reverse();
        left.extend(right);

        scene.push(
            tag,
            DrawCmd::Polygon {
                points: left,
                fill: marks::lighten(s.fill, 0.4),
                stroke: Some((&s.border).into()),
            },
        );

        if show_quartiles {
            let stroke = Stroke::new(s.border.color, s.border.width.max(1.0));
            for (v, w) in [(st.q1, 0.5), (st.median, 0.8), (st.q3, 0.5)] {
                let y = ys.to_screen(v);
                scene.push(
                    tag,
                    DrawCmd::Line {
                        from: pt(cx - half * w, y),
                        to: pt(cx + half * w, y),
                        stroke: stroke.clone(),
                    },
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Survival
// ---------------------------------------------------------------------------

fn survival(
    scene: &mut Scene,
    spec: &GraphSpec,
    table: &DataTable,
    layout: &Layout,
    show_censor_marks: bool,
    _as_percent: bool,
) {
    let ys = &layout.y_scale;
    for (i, s) in visible(spec) {
        let tag = Tag::Series(i);
        let points = series::xy_points(table, s.dataset, Center::Mean, Spread::None);
        if points.is_empty() {
            continue;
        }
        // A survival curve is a staircase: hold the previous level until the next
        // event time, then step down.
        let mut path: Vec<Point> = Vec::with_capacity(points.len() * 2);
        let mut prev: Option<(f32, f32)> = None;
        for p in &points {
            if !layout.x_scale.is_valid(p.x) || !ys.is_valid(p.y) {
                continue;
            }
            let x = layout.x_scale.to_screen(p.x);
            let y = ys.to_screen(p.y);
            if let Some((_, py)) = prev {
                path.push(pt(x, py));
            }
            path.push(pt(x, y));
            prev = Some((x, y));
        }
        if path.len() > 1 && s.line.is_visible() {
            scene.push(
                tag,
                DrawCmd::Polyline { points: path, stroke: (&s.line).into(), close: false },
            );
        }
        if show_censor_marks {
            // Prism marks censored subjects with a tick on the curve. The table
            // does not distinguish them here, so every plotted point is marked.
            for p in &points {
                if layout.x_scale.is_valid(p.x) && ys.is_valid(p.y) {
                    marks::symbol(
                        scene,
                        tag,
                        pt(layout.x_scale.to_screen(p.x), ys.to_screen(p.y)),
                        &s.symbol,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Parts of whole
// ---------------------------------------------------------------------------

fn parts_of_whole(
    scene: &mut Scene,
    spec: &GraphSpec,
    table: &DataTable,
    layout: &Layout,
    donut_hole: f32,
    start_angle: f32,
    explode: &[usize],
) {
    // Slices come from the first dataset's rows, which is how Prism fills a
    // Parts of whole table.
    let values: Vec<f64> = (0..table.values.rows())
        .filter_map(|r| table.values.get(r, table.y_offset(0)).as_number())
        .filter(|v| *v > 0.0)
        .collect();
    let total: f64 = values.iter().sum();
    if total <= 0.0 {
        return;
    }

    let plot = layout.plot;
    let radius = plot.w.min(plot.h) / 2.0 * 0.85;
    let center = plot.center();
    let mut angle = start_angle;

    for (i, v) in values.iter().enumerate() {
        let sweep = (-(v / total) * 360.0) as f32;
        let fill = spec
            .series
            .get(i)
            .map(|s| s.fill)
            .unwrap_or_else(|| prism_doc::graph::palette_color(i));

        // An exploded slice is pushed out along its own bisector.
        let (cx, cy) = if explode.contains(&i) {
            let mid = (angle + sweep / 2.0).to_radians();
            (center.x + radius * 0.1 * mid.cos(), center.y - radius * 0.1 * mid.sin())
        } else {
            (center.x, center.y)
        };

        scene.push(
            Tag::Series(i),
            DrawCmd::Wedge {
                center: pt(cx, cy),
                radius,
                inner_radius: radius * donut_hole.clamp(0.0, 0.95),
                start_angle: angle,
                sweep,
                fill,
                stroke: Some(Stroke::new(Color::WHITE, 1.0)),
            },
        );
        angle += sweep;
    }
}

// ---------------------------------------------------------------------------
// Multivariable plots
// ---------------------------------------------------------------------------

fn heatmap(
    scene: &mut Scene,
    spec: &GraphSpec,
    table: &DataTable,
    layout: &Layout,
    colormap: &Colormap,
    show_values: bool,
) {
    let rows = table.values.rows();
    let cols = table.ys.len();
    if rows == 0 || cols == 0 {
        return;
    }
    let plot = layout.plot;
    let cw = plot.w / cols as f32;
    let ch = plot.h / rows as f32;

    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for c in 0..cols {
        for r in 0..rows {
            if let Some(v) = table.values.get(r, table.y_offset(c)).as_number() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    }
    if !lo.is_finite() || !hi.is_finite() {
        return;
    }
    let span = if (hi - lo).abs() < f64::EPSILON { 1.0 } else { hi - lo };

    for c in 0..cols {
        for r in 0..rows {
            let Some(v) = table.values.get(r, table.y_offset(c)).as_number() else { continue };
            let t = (v - lo) / span;
            let rect = Rect::new(plot.x + c as f32 * cw, plot.y + r as f32 * ch, cw, ch);
            scene.push(
                Tag::Series(c),
                DrawCmd::Rect { rect, fill: colormap.sample(t), stroke: None },
            );
            if show_values {
                scene.push(
                    Tag::Series(c),
                    DrawCmd::Text(TextRun {
                        text: format!("{v:.3}"),
                        at: rect.center(),
                        size: (ch * 0.4).min(9.0),
                        family: spec.legend.style.family.clone(),
                        color: Color::BLACK,
                        bold: false,
                        italic: false,
                        h: HAlign::Center,
                        v: VAlign::Middle,
                        rotation: 0.0,
                    }),
                );
            }
        }
    }
}

fn bubble(
    scene: &mut Scene,
    spec: &GraphSpec,
    table: &DataTable,
    layout: &Layout,
    size_variable: Option<(usize, usize)>,
    color_variable: Option<(usize, usize)>,
    colormap: &Colormap,
) {
    let ys = &layout.y_scale;
    let value_at = |v: Option<(usize, usize)>, row: usize| -> Option<f64> {
        let (ds, sub) = v?;
        table.values.get(row, table.y_offset(ds) + sub).as_number()
    };

    let range = |v: Option<(usize, usize)>| -> Option<(f64, f64)> {
        let (ds, sub) = v?;
        let col = table.y_offset(ds) + sub;
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for r in 0..table.values.rows() {
            if let Some(x) = table.values.get(r, col).as_number() {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
        (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
    };
    let size_range = range(size_variable);
    let color_range = range(color_variable);

    for (i, s) in visible(spec) {
        let tag = Tag::Series(i);
        for p in series::xy_points(table, s.dataset, Center::Mean, Spread::None) {
            if !layout.x_scale.is_valid(p.x) || !ys.is_valid(p.y) {
                continue;
            }
            let mut sym = s.symbol.clone();
            if let (Some(v), Some((lo, hi))) = (value_at(size_variable, p.row), size_range) {
                let t = if (hi - lo).abs() < f64::EPSILON { 0.5 } else { (v - lo) / (hi - lo) };
                // Encode by area, so the eye compares sizes correctly.
                sym.size = (4.0 + 20.0 * t.max(0.0).sqrt() as f32).max(2.0);
            }
            if let (Some(v), Some((lo, hi))) = (value_at(color_variable, p.row), color_range) {
                let t = if (hi - lo).abs() < f64::EPSILON { 0.5 } else { (v - lo) / (hi - lo) };
                sym.fill = colormap.sample(t);
                sym.border = sym.fill;
            }
            marks::symbol(
                scene,
                tag,
                pt(layout.x_scale.to_screen(p.x), ys.to_screen(p.y)),
                &sym,
            );
        }
    }
}

fn dendrogram(
    scene: &mut Scene,
    spec: &GraphSpec,
    table: &DataTable,
    layout: &Layout,
    horizontal: bool,
) {
    // Prism's dendrogram sheets store a merge map: each row joins two clusters at
    // a height. Columns are (left, right, height); leaves are negative indices.
    if table.ys.len() < 3 {
        return;
    }
    let rows = table.values.rows();
    let col = |c: usize, r: usize| table.values.get(r, table.y_offset(c)).as_number();

    let stroke: Stroke = spec
        .series
        .first()
        .map(|s| (&s.line).into())
        .unwrap_or_else(|| Stroke::new(Color::BLACK, 1.0));

    // Position of each cluster once merged, in (across, height) data space.
    let mut pos: Vec<Option<(f64, f64)>> = vec![None; rows];
    let mut next_leaf = 0.0;

    let place = |idx: f64, pos: &[Option<(f64, f64)>], next_leaf: &mut f64| -> Option<(f64, f64)> {
        if idx < 1.0 {
            // A leaf, which Prism writes as a negative index. Zero is not a valid
            // one-based cluster reference, so treat it as a leaf too.
            let p = (*next_leaf, 0.0);
            *next_leaf += 1.0;
            Some(p)
        } else {
            // Cluster references are one-based rows of the merge map.
            *pos.get(idx as usize - 1)?
        }
    };

    for r in 0..rows {
        let (Some(l), Some(rt), Some(h)) = (col(0, r), col(1, r), col(2, r)) else { continue };
        let Some(a) = place(l, &pos, &mut next_leaf) else { continue };
        let Some(b) = place(rt, &pos, &mut next_leaf) else { continue };

        let to = |across: f64, height: f64| {
            if horizontal {
                pt(layout.y_scale.to_screen(height), layout.x_scale.to_screen(across))
            } else {
                pt(layout.x_scale.to_screen(across), layout.y_scale.to_screen(height))
            }
        };

        // The bracket joining the two children at height `h`.
        scene.push(
            Tag::Series(0),
            DrawCmd::Polyline {
                points: vec![to(a.0, a.1), to(a.0, h), to(b.0, h), to(b.0, b.1)],
                stroke: stroke.clone(),
                close: false,
            },
        );
        pos[r] = Some(((a.0 + b.0) / 2.0, h));
    }
}
