//! Render every data sheet of a Prism document to SVG.
//!
//! ```text
//! cargo run -p prism-render --example export -- input.prism out_dir [plot-kind]
//! ```

use prism_doc::graph::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("usage: export <document> <out-dir> [kind]")?;
    let out_dir = args.next().unwrap_or_else(|| ".".into());
    let want = args.next();

    std::fs::create_dir_all(&out_dir)?;
    let doc = prism_doc::open(&input)?;

    for (i, sheet) in doc.data_sheets.iter().enumerate() {
        let mut spec = GraphSpec::default_for_table(&sheet.table);
        if let Some(k) = &want {
            spec.kind = match k.as_str() {
                "xy" => PlotKind::XY { connect: true, area: false, spline: false },
                "bar" => PlotKind::Bar {
                    layout: BarLayout::Interleaved,
                    horizontal: false,
                    center: Center::Mean,
                    spread: Spread::Sd,
                },
                "scatter" => PlotKind::ScatterDot {
                    jitter: Jitter::Aligned,
                    center: Center::Mean,
                    spread: Spread::Sd,
                },
                "box" => PlotKind::BoxWhisker {
                    style: BoxWhiskerStyle::Tukey,
                    horizontal: false,
                    show_points: false,
                },
                "violin" => PlotKind::Violin { show_quartiles: true, horizontal: false },
                "pie" => PlotKind::PartsOfWhole {
                    donut_hole: 0.0,
                    start_angle: 90.0,
                    explode: vec![],
                },
                "heatmap" => {
                    PlotKind::Heatmap { colormap: Colormap::default(), show_values: false }
                }
                other => return Err(format!("unknown plot kind {other}").into()),
            };
        }
        if sheet.table.x.is_some() {
            if let PlotKind::XY { connect, .. } = &mut spec.kind {
                *connect = true;
            }
        }

        let scene = prism_render::render(&sheet.table, &spec);
        let name = format!("{out_dir}/sheet{i}.svg");
        std::fs::write(&name, prism_render::to_svg(&scene))?;
        println!("{name}  ({} x {} pt)", scene.size.0.round(), scene.size.1.round());
    }
    Ok(())
}
