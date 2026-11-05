//! Render every table in every document Prism ships, in every graph family, and
//! check the output is sane. This is the broadest correctness net we have: it
//! exercises the layout and scale code against real-world data shapes, including
//! empty tables, text columns, single rows and very wide tables.

use std::path::{Path, PathBuf};

use prism_doc::graph::*;
use prism_render::{DrawCmd, Tag};

const BUNDLE: &str = "/Applications/Prism 11.app/Contents/SharedSupport";

fn samples() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| matches!(x, "prismt" | "pzfx" | "xml"))
            {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new(BUNDLE), &mut out);
    out.sort();
    out
}

fn all_kinds() -> Vec<PlotKind> {
    vec![
        PlotKind::XY { connect: true, area: false, spline: false },
        PlotKind::XY { connect: true, area: true, spline: true },
        PlotKind::Bar {
            layout: BarLayout::Interleaved,
            horizontal: false,
            center: Center::Mean,
            spread: Spread::Sd,
        },
        PlotKind::Bar {
            layout: BarLayout::Stacked,
            horizontal: false,
            center: Center::Mean,
            spread: Spread::None,
        },
        PlotKind::Bar {
            layout: BarLayout::StackedPercent,
            horizontal: true,
            center: Center::Mean,
            spread: Spread::None,
        },
        PlotKind::ScatterDot {
            jitter: Jitter::Aligned,
            center: Center::Mean,
            spread: Spread::Sd,
        },
        PlotKind::BoxWhisker {
            style: BoxWhiskerStyle::Tukey,
            horizontal: false,
            show_points: true,
        },
        PlotKind::Violin { show_quartiles: true, horizontal: false },
        PlotKind::Survival { show_censor_marks: true, as_percent: true },
        PlotKind::PartsOfWhole { donut_hole: 0.4, start_angle: 90.0, explode: vec![0] },
        PlotKind::Heatmap { colormap: Colormap::default(), show_values: false },
        PlotKind::Bubble {
            size_variable: Some((0, 0)),
            color_variable: Some((0, 0)),
            colormap: Colormap::default(),
        },
        PlotKind::Dendrogram { horizontal: false },
    ]
}

#[test]
fn every_shipped_table_renders_in_every_graph_family() {
    let files = samples();
    if files.is_empty() {
        eprintln!("Prism 11 not installed; skipping");
        return;
    }

    let mut rendered = 0usize;
    for f in &files {
        let doc = prism_doc::open(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        for sheet in &doc.data_sheets {
            for kind in all_kinds() {
                let mut spec = GraphSpec::default_for_table(&sheet.table);
                spec.kind = kind;
                let scene = prism_render::render(&sheet.table, &spec);
                rendered += 1;

                assert!(
                    scene.size.0.is_finite() && scene.size.1.is_finite(),
                    "{} / {}: non-finite page size",
                    f.display(),
                    sheet.title
                );
                assert!(
                    scene.size.0 > 0.0 && scene.size.1 > 0.0,
                    "{} / {}: empty page",
                    f.display(),
                    sheet.title
                );

                for c in scene.iter() {
                    assert_finite(&c.cmd, f, &sheet.title);
                }
            }
        }
    }
    eprintln!("rendered {rendered} scenes from {} documents", files.len());
    assert!(rendered > 0);
}

/// Every coordinate must be a real number: a NaN silently blanks a backend.
fn assert_finite(cmd: &DrawCmd, f: &Path, sheet: &str) {
    let ok = |x: f32, y: f32| {
        assert!(
            x.is_finite() && y.is_finite(),
            "{} / {sheet}: non-finite coordinate ({x}, {y})",
            f.display()
        );
    };
    match cmd {
        DrawCmd::Line { from, to, .. } => {
            ok(from.x, from.y);
            ok(to.x, to.y);
        }
        DrawCmd::Polyline { points, .. } | DrawCmd::Polygon { points, .. } => {
            for p in points {
                ok(p.x, p.y);
            }
        }
        DrawCmd::Rect { rect, .. } => {
            ok(rect.x, rect.y);
            ok(rect.w, rect.h);
        }
        DrawCmd::Circle { center, radius, .. } => {
            ok(center.x, center.y);
            assert!(radius.is_finite());
        }
        DrawCmd::Wedge { center, radius, sweep, .. } => {
            ok(center.x, center.y);
            assert!(radius.is_finite() && sweep.is_finite());
        }
        DrawCmd::Text(t) => ok(t.at.x, t.at.y),
    }
}

#[test]
fn series_marks_stay_within_the_plot_area() {
    let files = samples();
    if files.is_empty() {
        eprintln!("Prism 11 not installed; skipping");
        return;
    }
    // A generous tolerance: symbols are centred on their point and legitimately
    // overhang the axis by half their size.
    const SLACK: f32 = 16.0;

    for f in files.iter().take(30) {
        let doc = prism_doc::open(f).unwrap();
        for sheet in &doc.data_sheets {
            if sheet.table.ys.is_empty() {
                continue;
            }
            let spec = GraphSpec::default_for_table(&sheet.table);
            let scene = prism_render::render(&sheet.table, &spec);
            let a = scene.plot_area;
            for c in scene.iter().filter(|c| matches!(c.tag, Tag::Series(_))) {
                if let DrawCmd::Circle { center, .. } = c.cmd {
                    assert!(
                        center.x >= a.x - SLACK
                            && center.x <= a.right() + SLACK
                            && center.y >= a.y - SLACK
                            && center.y <= a.bottom() + SLACK,
                        "{} / {}: mark at {center:?} outside {a:?}",
                        f.display(),
                        sheet.title
                    );
                }
            }
        }
    }
}

#[test]
fn svg_export_is_balanced_for_every_sample() {
    let files = samples();
    if files.is_empty() {
        eprintln!("Prism 11 not installed; skipping");
        return;
    }
    for f in files.iter().take(30) {
        let doc = prism_doc::open(f).unwrap();
        for sheet in &doc.data_sheets {
            let spec = GraphSpec::default_for_table(&sheet.table);
            let svg = prism_render::to_svg(&prism_render::render(&sheet.table, &spec));
            assert_eq!(svg.matches("<svg").count(), 1);
            assert_eq!(svg.matches("</svg>").count(), 1);
            assert!(!svg.contains("NaN"), "{}: NaN leaked into SVG", f.display());
            assert!(!svg.contains("inf"), "{}: infinity leaked into SVG", f.display());
        }
    }
}
