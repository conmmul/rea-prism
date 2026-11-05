//! The inspector panel.
//!
//! Prism keeps a narrow inspector beside the sheet whose tabs switch with what
//! is selected — Graph, Axes, Text, Line, Shape, Legend, Page — each a stack of
//! small sections. This mirrors that organisation and offers the settings that
//! are quick to reach for; the full set lives in the Format dialogs.

use egui::{DragValue, RichText, Ui};
use prism_doc::graph::*;

use crate::widgets::{color, combo, group, points, row, stroke, symbol_shape};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Graph,
    Axes,
    Text,
    Line,
    Shape,
    Legend,
    Page,
}

impl Tab {
    pub const ALL: [Tab; 7] =
        [Tab::Graph, Tab::Axes, Tab::Text, Tab::Line, Tab::Shape, Tab::Legend, Tab::Page];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Graph => "Graph",
            Tab::Axes => "Axes",
            Tab::Text => "Text",
            Tab::Line => "Line",
            Tab::Shape => "Shape",
            Tab::Legend => "Legend",
            Tab::Page => "Page",
        }
    }
}

/// Show the inspector. Returns true when the spec changed.
pub fn show(
    ui: &mut Ui,
    tab: &mut Tab,
    spec: &mut GraphSpec,
    selected: &mut Option<usize>,
) -> bool {
    let mut changed = false;

    // The tab strip, which Prism draws as a segmented control across the top.
    ui.horizontal_wrapped(|ui| {
        for t in Tab::ALL {
            if ui.selectable_label(*tab == t, t.label()).clicked() {
                *tab = t;
            }
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match *tab {
        Tab::Graph => changed |= graph_tab(ui, spec, selected),
        Tab::Axes => changed |= axes_tab(ui, spec),
        Tab::Text => changed |= text_tab(ui, spec),
        Tab::Line => changed |= line_tab(ui, spec, selected),
        Tab::Shape => changed |= shape_tab(ui, spec, selected),
        Tab::Legend => changed |= legend_tab(ui, spec),
        Tab::Page => changed |= page_tab(ui, spec),
    });

    changed
}

/// The dataset the Line and Shape tabs act on.
fn current<'a>(spec: &'a mut GraphSpec, selected: &Option<usize>) -> Option<&'a mut Series> {
    let i = selected.unwrap_or(0);
    spec.series.get_mut(i)
}

fn dataset_picker(ui: &mut Ui, spec: &GraphSpec, selected: &mut Option<usize>) {
    if spec.series.len() < 2 {
        return;
    }
    let i = selected.unwrap_or(0).min(spec.series.len() - 1);
    ui.horizontal(|ui| {
        ui.label("Dataset");
        egui::ComboBox::from_id_salt("inspector_dataset")
            .selected_text(spec.series[i].label.clone())
            .show_ui(ui, |ui| {
                for (k, s) in spec.series.iter().enumerate() {
                    if ui.selectable_label(i == k, &s.label).clicked() {
                        *selected = Some(k);
                    }
                }
            });
    });
}

fn graph_tab(ui: &mut Ui, spec: &mut GraphSpec, selected: &mut Option<usize>) -> bool {
    let mut changed = false;

    changed |= group(ui, "Graph", |ui| {
        let mut c = row(ui, "Type", |ui| {
            ui.label(RichText::new(kind_name(&spec.kind)).strong());
            false
        });
        c |= row(ui, "Width", |ui| points(ui, &mut spec.size.0, 80.0..=2000.0).changed());
        c |= row(ui, "Height", |ui| points(ui, &mut spec.size.1, 60.0..=2000.0).changed());
        c
    });

    // Family-specific settings, the same ones the Change Graph Type sheet offers.
    changed |= group(ui, "Options", |ui| match &mut spec.kind {
        PlotKind::XY { connect, area, spline } => {
            let mut c = ui.checkbox(connect, "Connect the points").changed();
            c |= ui.checkbox(area, "Fill the area beneath").changed();
            c |= ui.checkbox(spline, "Smooth the line").changed();
            c
        }
        PlotKind::Bar { layout, horizontal, center, spread } => {
            let mut c = row(ui, "Arrangement", |ui| {
                combo(
                    ui,
                    "bar_layout",
                    layout,
                    &[
                        (BarLayout::Interleaved, "Side by side"),
                        (BarLayout::Stacked, "Stacked"),
                        (BarLayout::StackedPercent, "Stacked to 100%"),
                        (BarLayout::Separated, "Separated"),
                    ],
                )
            });
            c |= ui.checkbox(horizontal, "Horizontal").changed();
            c |= center_spread(ui, center, spread);
            c
        }
        PlotKind::ScatterDot { jitter, center, spread } => {
            let mut c = row(ui, "Spread points", |ui| {
                combo(
                    ui,
                    "jitter",
                    jitter,
                    &[
                        (Jitter::Aligned, "Aligned"),
                        (Jitter::Random, "Scattered"),
                        (Jitter::None, "Stacked"),
                    ],
                )
            });
            c |= center_spread(ui, center, spread);
            c
        }
        PlotKind::BoxWhisker { style, horizontal, show_points } => {
            let mut c = row(ui, "Whiskers", |ui| {
                combo(
                    ui,
                    "bw",
                    style,
                    &[
                        (BoxWhiskerStyle::Tukey, "Tukey"),
                        (BoxWhiskerStyle::MinMax, "Minimum to maximum"),
                        (BoxWhiskerStyle::Percentile10_90, "10th to 90th centile"),
                        (BoxWhiskerStyle::Percentile5_95, "5th to 95th centile"),
                        (BoxWhiskerStyle::MeanSd, "Mean with SD"),
                    ],
                )
            });
            c |= ui.checkbox(horizontal, "Horizontal").changed();
            c |= ui.checkbox(show_points, "Show every point").changed();
            c
        }
        PlotKind::Violin { show_quartiles, horizontal } => {
            let mut c = ui.checkbox(show_quartiles, "Show quartile lines").changed();
            c |= ui.checkbox(horizontal, "Horizontal").changed();
            c
        }
        PlotKind::Survival { show_censor_marks, as_percent } => {
            let mut c = ui.checkbox(show_censor_marks, "Mark each event").changed();
            c |= ui.checkbox(as_percent, "Show as a percentage").changed();
            c
        }
        PlotKind::PartsOfWhole { donut_hole, start_angle, .. } => {
            let mut c = row(ui, "Hole", |ui| {
                ui.add(egui::Slider::new(donut_hole, 0.0..=0.9)).changed()
            });
            c |= row(ui, "Start angle", |ui| {
                ui.add(DragValue::new(start_angle).range(0.0..=360.0).suffix("°")).changed()
            });
            c
        }
        PlotKind::Heatmap { show_values, .. } => {
            ui.checkbox(show_values, "Print the values").changed()
        }
        PlotKind::Bubble { .. } => {
            ui.label(RichText::new("Size and colour follow the extra variables.").small().weak());
            false
        }
        PlotKind::Dendrogram { horizontal } => ui.checkbox(horizontal, "Horizontal").changed(),
    });

    dataset_picker(ui, spec, selected);
    changed
}

fn center_spread(ui: &mut Ui, center: &mut Center, spread: &mut Spread) -> bool {
    let mut c = row(ui, "Centre", |ui| {
        combo(
            ui,
            "center",
            center,
            &[
                (Center::Mean, "Mean"),
                (Center::Median, "Median"),
                (Center::GeometricMean, "Geometric mean"),
                (Center::None, "Every value"),
            ],
        )
    });
    c |= row(ui, "Error bars", |ui| {
        combo(
            ui,
            "spread",
            spread,
            &[
                (Spread::Sd, "Standard deviation"),
                (Spread::Sem, "Standard error"),
                (Spread::Ci, "95% confidence interval"),
                (Spread::Range, "Full range"),
                (Spread::FromTable, "From the table"),
                (Spread::None, "None"),
            ],
        )
    });
    c
}

fn axes_tab(ui: &mut Ui, spec: &mut GraphSpec) -> bool {
    let mut changed = false;
    for (name, axis) in [("X axis", &mut spec.x), ("Y axis", &mut spec.y)] {
        changed |= group(ui, name, |ui| {
            let mut c = row(ui, "Title", |ui| {
                ui.text_edit_singleline(&mut axis.title.text).changed()
            });
            c |= ui.checkbox(&mut axis.title.visible, "Show the title").changed();
            if axis.scale != Scale::Categorical {
                let mut log = matches!(axis.scale, Scale::Log(_));
                if ui.checkbox(&mut log, "Logarithmic").changed() {
                    axis.scale = if log { Scale::Log(10.0) } else { Scale::Linear };
                    if log {
                        axis.number_format = NumberFormat::Power10;
                    }
                    c = true;
                }
            }
            let mut auto = axis.range.is_auto();
            if ui.checkbox(&mut auto, "Automatic range").changed() {
                axis.range = if auto {
                    Range::default()
                } else {
                    Range { min: Some(0.0), max: Some(1.0) }
                };
                c = true;
            }
            if !auto {
                let mut lo = axis.range.min.unwrap_or(0.0);
                let mut hi = axis.range.max.unwrap_or(1.0);
                c |= row(ui, "From", |ui| {
                    let r = ui.add(DragValue::new(&mut lo).speed(0.1)).changed();
                    if r {
                        axis.range.min = Some(lo);
                    }
                    r
                });
                c |= row(ui, "To", |ui| {
                    let r = ui.add(DragValue::new(&mut hi).speed(0.1)).changed();
                    if r {
                        axis.range.max = Some(hi);
                    }
                    r
                });
            }
            c |= ui.checkbox(&mut axis.grid, "Grid lines").changed();
            c
        });
    }
    changed
}

fn text_tab(ui: &mut Ui, spec: &mut GraphSpec) -> bool {
    let mut changed = false;
    changed |= group(ui, "Graph title", |ui| {
        let mut c = row(ui, "Text", |ui| {
            ui.text_edit_singleline(&mut spec.title.text).changed()
        });
        c |= ui.checkbox(&mut spec.title.visible, "Show").changed();
        c |= row(ui, "Size", |ui| points(ui, &mut spec.title.style.size, 6.0..=36.0).changed());
        c |= ui.checkbox(&mut spec.title.style.bold, "Bold").changed();
        c |= ui.checkbox(&mut spec.title.style.italic, "Italic").changed();
        c
    });
    changed |= group(ui, "Axis numbering", |ui| {
        let mut c = row(ui, "Size", |ui| {
            let r = points(ui, &mut spec.x.label_style.size, 5.0..=24.0).changed();
            if r {
                spec.y.label_style.size = spec.x.label_style.size;
            }
            r
        });
        c |= row(ui, "X rotation", |ui| {
            ui.add(DragValue::new(&mut spec.x.label_rotation).range(-90.0..=90.0).suffix("°"))
                .changed()
        });
        c
    });
    changed
}

fn line_tab(ui: &mut Ui, spec: &mut GraphSpec, selected: &mut Option<usize>) -> bool {
    dataset_picker(ui, spec, selected);
    let sel = *selected;
    let Some(s) = current(spec, &sel) else {
        ui.label("No dataset selected.");
        return false;
    };
    group(ui, "Line", |ui| {
        let mut c = ui.checkbox(&mut s.show_line, "Show the line").changed();
        ui.add_enabled_ui(s.show_line, |ui| {
            c |= row(ui, "Style", |ui| stroke(ui, "insp_line", &mut s.line));
        });
        c
    })
}

fn shape_tab(ui: &mut Ui, spec: &mut GraphSpec, selected: &mut Option<usize>) -> bool {
    dataset_picker(ui, spec, selected);
    let sel = *selected;
    let Some(s) = current(spec, &sel) else {
        ui.label("No dataset selected.");
        return false;
    };
    let mut changed = group(ui, "Symbols", |ui| {
        let mut c = ui.checkbox(&mut s.show_symbols, "Show symbols").changed();
        ui.add_enabled_ui(s.show_symbols, |ui| {
            c |= row(ui, "Shape", |ui| symbol_shape(ui, "insp_shape", &mut s.symbol.shape));
            c |= row(ui, "Size", |ui| points(ui, &mut s.symbol.size, 0.0..=40.0).changed());
            c |= row(ui, "Fill", |ui| color(ui, &mut s.symbol.fill));
            c |= row(ui, "Border", |ui| color(ui, &mut s.symbol.border));
        });
        c
    });
    changed |= group(ui, "Fill", |ui| {
        let mut c = row(ui, "Colour", |ui| color(ui, &mut s.fill));
        c |= row(ui, "Border", |ui| stroke(ui, "insp_border", &mut s.border));
        c
    });
    changed
}

fn legend_tab(ui: &mut Ui, spec: &mut GraphSpec) -> bool {
    let mut changed = group(ui, "Legend", |ui| {
        let mut c = row(ui, "Position", |ui| {
            combo(
                ui,
                "insp_legend",
                &mut spec.legend.position,
                &[
                    (LegendPosition::Right, "Right"),
                    (LegendPosition::Left, "Left"),
                    (LegendPosition::Above, "Above"),
                    (LegendPosition::Below, "Below"),
                    (LegendPosition::None, "Hidden"),
                ],
            )
        });
        c |= ui.checkbox(&mut spec.legend.frame, "Draw a box").changed();
        c |= row(ui, "Size", |ui| points(ui, &mut spec.legend.style.size, 5.0..=24.0).changed());
        c
    });
    changed |= group(ui, "Entries", |ui| {
        let mut c = false;
        for s in spec.series.iter_mut() {
            ui.horizontal(|ui| {
                c |= ui.checkbox(&mut s.in_legend, "").changed();
                c |= ui.text_edit_singleline(&mut s.label).changed();
            });
        }
        c
    });
    changed
}

fn page_tab(ui: &mut Ui, spec: &mut GraphSpec) -> bool {
    group(ui, "Page", |ui| {
        let mut c = row(ui, "Background", |ui| color(ui, &mut spec.background));
        c |= row(ui, "Plotting area", |ui| color(ui, &mut spec.plot_background));
        c |= row(ui, "Frame", |ui| {
            combo(
                ui,
                "insp_frame",
                &mut spec.frame,
                &[
                    (Frame::PlainAxes, "Two axes"),
                    (Frame::Box, "Box"),
                    (Frame::Offset, "Offset"),
                ],
            )
        });
        c
    })
}

pub fn kind_name(kind: &PlotKind) -> &'static str {
    match kind {
        PlotKind::XY { connect: false, .. } => "Points only",
        PlotKind::XY { area: true, .. } => "Area fill",
        PlotKind::XY { spline: true, .. } => "Smoothed curve",
        PlotKind::XY { .. } => "Points and line",
        PlotKind::Bar { layout: BarLayout::Stacked, .. } => "Stacked bars",
        PlotKind::Bar { layout: BarLayout::StackedPercent, .. } => "Stacked to 100%",
        PlotKind::Bar { .. } => "Bars",
        PlotKind::ScatterDot { .. } => "Scatter with mean",
        PlotKind::BoxWhisker { .. } => "Box and whisker",
        PlotKind::Violin { .. } => "Violin",
        PlotKind::Survival { .. } => "Survival staircase",
        PlotKind::PartsOfWhole { donut_hole, .. } if *donut_hole > 0.0 => "Donut",
        PlotKind::PartsOfWhole { .. } => "Pie",
        PlotKind::Heatmap { .. } => "Heat map",
        PlotKind::Bubble { .. } => "Bubble plot",
        PlotKind::Dendrogram { .. } => "Dendrogram",
    }
}
