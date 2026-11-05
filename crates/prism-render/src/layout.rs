//! Deciding where the plotting area sits on the page.
//!
//! Prism sizes the plotting area itself and lets the surrounding furniture (tick
//! labels, axis titles, legend) extend outward from it. We do the same: the spec's
//! `size` is the data area, and the page grows to fit whatever is drawn around it.

use prism_doc::graph::{Axis, GraphSpec, LegendPosition, Scale};

use crate::scale::{self, AxisScale};
use crate::scene::Rect;
use crate::text;

/// Gap between the axis line and its tick labels.
const LABEL_GAP: f32 = 4.0;
/// Gap between tick labels and the axis title.
const TITLE_GAP: f32 = 6.0;
/// Page margin outside everything.
const PAGE_MARGIN: f32 = 12.0;

pub struct Layout {
    pub page: (f32, f32),
    pub plot: Rect,
    pub x_scale: AxisScale,
    pub y_scale: AxisScale,
    pub y2_scale: Option<AxisScale>,
    pub legend: Option<Rect>,
}

/// Width taken by a Y axis's tick labels and title.
fn y_furniture(axis: &Axis, s: &AxisScale) -> f32 {
    if !axis.visible {
        return 0.0;
    }
    let labels = scale::ticks(axis, s)
        .iter()
        .filter_map(|t| t.label.as_ref())
        .map(|l| text::width(l, axis.label_style.size))
        .fold(0.0, f32::max);
    let title = if axis.title.visible && !axis.title.text.is_empty() {
        text::line_height(axis.title.style.size) + TITLE_GAP
    } else {
        0.0
    };
    axis.tick_length + LABEL_GAP + labels + title
}

/// Height taken by an X axis's tick labels and title.
fn x_furniture(axis: &Axis, s: &AxisScale) -> f32 {
    if !axis.visible {
        return 0.0;
    }
    let ticks = scale::ticks(axis, s);
    let labels = if axis.label_rotation.abs() > 1.0 {
        // Rotated labels contribute their length vertically.
        ticks
            .iter()
            .filter_map(|t| t.label.as_ref())
            .map(|l| text::width(l, axis.label_style.size))
            .fold(0.0, f32::max)
            * axis.label_rotation.to_radians().sin().abs()
    } else {
        ticks
            .iter()
            .any(|t| t.label.is_some())
            .then(|| text::line_height(axis.label_style.size))
            .unwrap_or(0.0)
    };
    let title = if axis.title.visible && !axis.title.text.is_empty() {
        text::line_height(axis.title.style.size) + TITLE_GAP
    } else {
        0.0
    };
    axis.tick_length + LABEL_GAP + labels + title
}

/// Build a scale with provisional screen bounds, used while measuring.
fn provisional(axis: &Axis, lo: f64, hi: f64, span: f32, flip: bool) -> AxisScale {
    let (min, max) = match axis.scale {
        Scale::Log(base) => (
            axis.range.min.unwrap_or(lo).max(f64::MIN_POSITIVE).log(base),
            axis.range.max.unwrap_or(hi).max(f64::MIN_POSITIVE).log(base),
        ),
        Scale::Categorical => (0.0, axis.categories.len().max(1) as f64),
        Scale::Linear => (axis.range.min.unwrap_or(lo), axis.range.max.unwrap_or(hi)),
    };
    AxisScale {
        scale: axis.scale,
        min,
        max,
        screen_min: if flip { span } else { 0.0 },
        screen_max: if flip { 0.0 } else { span },
        reversed: axis.reversed,
        categories: axis.categories.len(),
    }
}

/// Place the plotting area and build the final scales.
///
/// `x_range` and `y_range` are the data extents the plot needs to show; axis
/// settings override them when the user has fixed the limits.
pub fn compute(spec: &GraphSpec, x_range: (f64, f64), y_range: (f64, f64)) -> Layout {
    let (w, h) = spec.size;

    let (ax0, ax1) = if spec.x.scale == Scale::Categorical {
        (0.0, spec.x.categories.len().max(1) as f64)
    } else {
        scale::nice_bounds(x_range.0, x_range.1, spec.x.scale)
    };
    let (ay0, ay1) = scale::nice_bounds(y_range.0, y_range.1, spec.y.scale);

    // Measure with provisional scales, then place the plot and rebuild them.
    let xs_probe = provisional(&spec.x, ax0, ax1, w, false);
    let ys_probe = provisional(&spec.y, ay0, ay1, h, true);

    let left = y_furniture(&spec.y, &ys_probe);
    let bottom = x_furniture(&spec.x, &xs_probe);
    let right_axis = spec
        .y2
        .as_ref()
        .map(|a| y_furniture(a, &provisional(a, ay0, ay1, h, true)))
        .unwrap_or(0.0);

    let title_h = if spec.title.visible && !spec.title.text.is_empty() {
        text::block_height(&spec.title.text, spec.title.style.size) + TITLE_GAP
    } else {
        0.0
    };

    // Legend box, sized from the longest series label.
    let legend_size = if spec.legend.position == LegendPosition::None || spec.series.is_empty() {
        None
    } else {
        let s = spec.legend.style.size;
        let swatch = s * 1.6;
        let lw = spec
            .series
            .iter()
            .filter(|x| x.visible)
            .map(|x| text::width(&x.label, s))
            .fold(0.0, f32::max);
        let n = spec.series.iter().filter(|x| x.visible).count().max(1) as f32;
        match spec.legend.position {
            LegendPosition::Above | LegendPosition::Below => {
                Some((lw * n + swatch * n + 12.0 * n, text::line_height(s) + 6.0))
            }
            _ => Some((swatch + 6.0 + lw + 10.0, text::line_height(s) * n + 6.0)),
        }
    };

    let (mut legend_w, mut legend_h) = (0.0, 0.0);
    if let Some((lw, lh)) = legend_size {
        match spec.legend.position {
            LegendPosition::Left | LegendPosition::Right | LegendPosition::Floating => legend_w = lw,
            LegendPosition::Above | LegendPosition::Below => legend_h = lh,
            LegendPosition::None => {}
        }
    }

    let left_pad = PAGE_MARGIN
        + left
        + if spec.legend.position == LegendPosition::Left { legend_w } else { 0.0 };
    let top_pad = PAGE_MARGIN
        + title_h
        + if spec.legend.position == LegendPosition::Above { legend_h } else { 0.0 };

    let plot = Rect::new(left_pad, top_pad, w, h);

    let page_w = left_pad
        + w
        + right_axis
        + PAGE_MARGIN
        + if matches!(spec.legend.position, LegendPosition::Right | LegendPosition::Floating) {
            legend_w
        } else {
            0.0
        };
    let page_h = top_pad
        + h
        + bottom
        + PAGE_MARGIN
        + if spec.legend.position == LegendPosition::Below { legend_h } else { 0.0 };

    let mut x_scale = provisional(&spec.x, ax0, ax1, w, false);
    x_scale.screen_min = plot.x;
    x_scale.screen_max = plot.right();

    let mut y_scale = provisional(&spec.y, ay0, ay1, h, true);
    y_scale.screen_min = plot.bottom();
    y_scale.screen_max = plot.y;

    let y2_scale = spec.y2.as_ref().map(|a| {
        let (lo, hi) = scale::nice_bounds(y_range.0, y_range.1, a.scale);
        let mut s = provisional(a, lo, hi, h, true);
        s.screen_min = plot.bottom();
        s.screen_max = plot.y;
        s
    });

    let legend = legend_size.map(|(lw, lh)| match spec.legend.position {
        LegendPosition::Left => Rect::new(PAGE_MARGIN, plot.y, lw, lh),
        LegendPosition::Above => Rect::new(plot.x, PAGE_MARGIN + title_h, lw, lh),
        LegendPosition::Below => Rect::new(plot.x, plot.bottom() + bottom, lw, lh),
        // Right and floating both sit to the right; floating adds the user offset.
        _ => Rect::new(
            plot.right() + right_axis + 8.0 + spec.legend.offset.0,
            plot.y + spec.legend.offset.1,
            lw,
            lh,
        ),
    });

    Layout { page: (page_w, page_h), plot, x_scale, y_scale, y2_scale, legend }
}
