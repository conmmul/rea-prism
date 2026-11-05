//! Rendering Prism graphs.
//!
//! [`render`] turns a data table plus a [`GraphSpec`] into a [`Scene`]: an ordered
//! list of primitives in points. Nothing here touches a window system, so the same
//! geometry is painted by the GUI and written to SVG.
//!
//! ```no_run
//! # use prism_doc::graph::GraphSpec;
//! let doc = prism_doc::open("experiment.prism")?;
//! let sheet = &doc.data_sheets[0];
//! let spec = GraphSpec::default_for_table(&sheet.table);
//! let scene = prism_render::render(&sheet.table, &spec);
//! std::fs::write("graph.svg", prism_render::to_svg(&scene))?;
//! # Ok::<(), prism_doc::Error>(())
//! ```

pub mod axes;
pub mod layout;
pub mod legend;
pub mod marks;
pub mod plots;
pub mod scale;
pub mod scene;
pub mod series;
pub mod svg;
pub mod text;

pub use scene::{DrawCmd, Point, Rect, Scene, Tag};
pub use svg::to_svg;

use prism_doc::graph::{AreaFill, Center, GraphSpec, PlotKind, Scale, Spread};
use prism_doc::model::DataTable;

/// Lay out and draw a graph.
pub fn render(table: &DataTable, spec: &GraphSpec) -> Scene {
    let (x_range, y_range) = data_extent(table, spec);
    let layout = layout::compute(spec, x_range, y_range);

    let mut scene = Scene {
        size: layout.page,
        plot_area: layout.plot,
        commands: Vec::new(),
    };

    if spec.background.a > 0 {
        scene.push(
            Tag::Background,
            DrawCmd::Rect {
                rect: Rect::new(0.0, 0.0, layout.page.0, layout.page.1),
                fill: spec.background,
                stroke: None,
            },
        );
    }

    // Pie charts have no axes; everything else gets its frame before the data so
    // the marks sit on top of the grid.
    let axis_less = matches!(spec.kind, PlotKind::PartsOfWhole { .. });
    if !axis_less {
        axes::draw(&mut scene, spec, &layout);
    }
    if let Some(stroke) = &spec.identity_line {
        // y = x, clipped to whichever span the two axes share.
        let lo = layout.x_scale.to_data(layout.plot.x).max(layout.y_scale.to_data(layout.plot.bottom()));
        let hi = layout.x_scale.to_data(layout.plot.right()).min(layout.y_scale.to_data(layout.plot.y));
        if hi > lo {
            scene.push(
                Tag::Grid,
                DrawCmd::Line {
                    from: scene::pt(layout.x_scale.to_screen(lo), layout.y_scale.to_screen(lo)),
                    to: scene::pt(layout.x_scale.to_screen(hi), layout.y_scale.to_screen(hi)),
                    stroke: stroke.into(),
                },
            );
        }
    }
    plots::draw(&mut scene, spec, table, &layout);
    legend::draw(&mut scene, spec, &layout);

    scene
}

/// The data extents a plot needs to show, before the axes round them off.
///
/// Explicit axis limits win; otherwise we measure what will actually be drawn,
/// including the reach of error bars and the baseline a bar must return to.
pub fn data_extent(table: &DataTable, spec: &GraphSpec) -> ((f64, f64), (f64, f64)) {
    let mut x = (f64::INFINITY, f64::NEG_INFINITY);
    let mut y = (f64::INFINITY, f64::NEG_INFINITY);

    let (center, spread) = match &spec.kind {
        PlotKind::Bar { center, spread, .. } => (*center, *spread),
        PlotKind::ScatterDot { center, spread, .. } => (*center, *spread),
        _ => (Center::Mean, Spread::FromTable),
    };

    let column_slots = matches!(
        spec.kind,
        PlotKind::ScatterDot { .. } | PlotKind::BoxWhisker { .. } | PlotKind::Violin { .. }
    );

    for s in spec.series.iter().filter(|s| s.visible) {
        if column_slots {
            // Column-style plots show every value in the dataset's column.
            for v in series::column_values(table, s.dataset) {
                y.0 = y.0.min(v);
                y.1 = y.1.max(v);
            }
            // Box and violin reach beyond the raw values only via whiskers, which
            // stay inside the data range for every style we draw.
            continue;
        }
        for p in series::xy_points(table, s.dataset, center, spread) {
            if p.x.is_finite() {
                x.0 = x.0.min(p.x);
                x.1 = x.1.max(p.x);
            }
            if p.y.is_finite() {
                y.0 = y.0.min(p.y - p.err_low.unwrap_or(0.0));
                y.1 = y.1.max(p.y + p.err_high.unwrap_or(0.0));
            }
        }
    }

    if !x.0.is_finite() {
        x = (0.0, 1.0);
    }
    if !y.0.is_finite() {
        y = (0.0, 1.0);
    }

    // Bars and areas are read against the baseline, so the axis must include it.
    let has_area = spec.series.iter().any(|s| s.area_fill != AreaFill::None);
    if (matches!(spec.kind, PlotKind::Bar { .. } | PlotKind::XY { area: true, .. }) || has_area)
        && !matches!(spec.y.scale, Scale::Log(_))
    {
        y.0 = y.0.min(spec.baseline);
        y.1 = y.1.max(spec.baseline);
    }

    let x = (spec.x.range.min.unwrap_or(x.0), spec.x.range.max.unwrap_or(x.1));
    let y = (spec.y.range.min.unwrap_or(y.0), spec.y.range.max.unwrap_or(y.1));
    (x, y)
}

#[cfg(test)]
mod tests {
    use prism_doc::graph::{GraphSpec, PlotKind, Series};
    use prism_doc::model::{DataSet, DataTable, SubcolumnFormat, TableFormat};
    use prism_doc::value::Cell;

    use super::*;

    fn xy_table() -> DataTable {
        let mut t = DataTable::new(TableFormat::XY);
        t.data_format = SubcolumnFormat::Single;
        t.ys = vec![DataSet::new("A"), DataSet::new("B")];
        t.sync_grid_width();
        t.values.resize(3, t.total_width());
        for (r, (x, a, b)) in [(1.0, 10.0, 20.0), (2.0, 15.0, 25.0), (3.0, 12.0, 30.0)]
            .into_iter()
            .enumerate()
        {
            t.values.set(r, 0, Cell::Number(x));
            t.values.set(r, 1, Cell::Number(a));
            t.values.set(r, 2, Cell::Number(b));
        }
        t
    }

    #[test]
    fn an_xy_scene_draws_every_point() {
        let t = xy_table();
        let mut spec = GraphSpec::default();
        spec.series = vec![Series::new(0, "A"), Series::new(1, "B")];
        let scene = render(&t, &spec);

        // Each dataset takes the next shape in Prism's cycle, so the first series
        // draws circles and the second squares.
        let marks = scene
            .iter()
            .filter(|c| matches!(c.tag, Tag::Series(_)))
            .filter(|c| matches!(c.cmd, DrawCmd::Circle { .. } | DrawCmd::Polygon { .. }))
            .count();
        assert_eq!(marks, 6, "two series of three points each");
        assert!(scene.size.0 > spec.size.0, "page is wider than the plot area");
    }

    #[test]
    fn points_land_inside_the_plot_area() {
        let t = xy_table();
        let mut spec = GraphSpec::default();
        spec.series = vec![Series::new(0, "A")];
        let scene = render(&t, &spec);
        let area = scene.plot_area;
        for c in scene.iter().filter(|c| matches!(c.tag, Tag::Series(_))) {
            if let DrawCmd::Circle { center, .. } = c.cmd {
                assert!(area.contains(center), "{center:?} escaped {area:?}");
            }
        }
    }

    #[test]
    fn a_bar_axis_includes_zero() {
        let t = xy_table();
        let mut spec = GraphSpec::default();
        spec.kind = PlotKind::Bar {
            layout: prism_doc::graph::BarLayout::Interleaved,
            horizontal: false,
            center: Center::Mean,
            spread: Spread::None,
        };
        spec.series = vec![Series::new(0, "A")];
        let (_, y) = data_extent(&t, &spec);
        assert!(y.0 <= 0.0, "bars must be read against zero");
    }

    #[test]
    fn svg_output_is_well_formed_and_contains_the_marks() {
        let t = xy_table();
        let mut spec = GraphSpec::default();
        spec.series = vec![Series::new(0, "A")];
        let svg = to_svg(&render(&t, &spec));
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>\n"));
        assert!(svg.contains("<circle"));
        assert_eq!(svg.matches("<svg").count(), 1);
    }

    /// A grouped table: three rows, two datasets, one slot per row.
    fn grouped_table() -> DataTable {
        let mut t = DataTable::new(TableFormat::Grouped);
        t.ys = vec![DataSet::new("Control"), DataSet::new("Treated")];
        t.row_titles = vec!["Day 1".into(), "Day 2".into(), "Day 3".into()];
        t.sync_grid_width();
        t.values.resize(3, t.total_width());
        for r in 0..3 {
            t.values.set(r, 0, Cell::Number(10.0 + r as f64));
            t.values.set(r, 1, Cell::Number(20.0 + r as f64));
        }
        t
    }

    #[test]
    fn grouped_bars_sit_inside_their_slot_without_overlapping() {
        let t = grouped_table();
        let spec = GraphSpec::default_for_table(&t);
        assert_eq!(spec.slots, prism_doc::graph::SlotSource::Rows);

        let scene = render(&t, &spec);
        let area = scene.plot_area;

        // Collect each series' bars in draw order.
        let mut bars: Vec<(usize, Rect)> = Vec::new();
        for c in scene.iter() {
            if let (Tag::Series(i), DrawCmd::Rect { rect, .. }) = (c.tag, &c.cmd) {
                bars.push((i, *rect));
            }
        }
        assert_eq!(bars.len(), 6, "three rows x two datasets");

        for (_, r) in &bars {
            assert!(
                r.x >= area.x - 0.5 && r.right() <= area.right() + 0.5,
                "bar {r:?} escaped {area:?}"
            );
            assert!(r.w > 0.0 && r.h > 0.0);
        }

        // Within one slot the two bars must not overlap.
        let slot_w = area.w / 3.0;
        for slot in 0..3 {
            let lo = area.x + slot as f32 * slot_w;
            let hi = lo + slot_w;
            let mut in_slot: Vec<&Rect> = bars
                .iter()
                .map(|(_, r)| r)
                .filter(|r| r.center().x > lo && r.center().x < hi)
                .collect();
            assert_eq!(in_slot.len(), 2, "slot {slot} should hold both datasets");
            in_slot.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
            assert!(
                in_slot[0].right() <= in_slot[1].x + 0.01,
                "bars overlap in slot {slot}: {:?} and {:?}",
                in_slot[0],
                in_slot[1]
            );
        }
    }

    #[test]
    fn column_slots_summarise_whole_datasets() {
        let mut t = DataTable::new(TableFormat::Column);
        t.ys = vec![DataSet::new("A"), DataSet::new("B")];
        t.sync_grid_width();
        t.values.resize(4, t.total_width());
        for r in 0..4 {
            t.values.set(r, 0, Cell::Number(1.0));
            t.values.set(r, 1, Cell::Number(5.0));
        }
        let spec = GraphSpec::default_for_table(&t);
        assert_eq!(spec.slots, prism_doc::graph::SlotSource::Datasets);
        assert_eq!(spec.x.categories, vec!["A", "B"]);

        // Both datasets are constant, so the Y range is just their two values.
        let (_, y) = data_extent(&t, &spec);
        assert!(y.0 <= 1.0 && y.1 >= 5.0);
    }

    #[test]
    fn an_empty_table_still_renders() {
        let t = DataTable::new(TableFormat::XY);
        let scene = render(&t, &GraphSpec::default());
        assert!(scene.size.0 > 0.0 && scene.size.1 > 0.0);
    }
}
