//! The format panel: every graph setting the renderer understands.

use egui::{ComboBox, DragValue, Slider};
use prism_doc::graph::*;

/// Show the inspector. Returns true when something changed.
pub fn show(ui: &mut egui::Ui, spec: &mut GraphSpec, selected: &mut Option<usize>) -> bool {
    let mut changed = false;

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("Graph");
        changed |= plot_kind(ui, spec);

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Title");
            changed |= ui.text_edit_singleline(&mut spec.title.text).changed();
            changed |= ui.checkbox(&mut spec.title.visible, "show").changed();
        });
        ui.horizontal(|ui| {
            ui.label("Size");
            changed |= ui.add(DragValue::new(&mut spec.size.0).range(80.0..=2000.0).suffix(" pt")).changed();
            ui.label("x");
            changed |= ui.add(DragValue::new(&mut spec.size.1).range(60.0..=2000.0).suffix(" pt")).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Frame");
            changed |= enum_combo(
                ui,
                "frame",
                &mut spec.frame,
                &[
                    (Frame::PlainAxes, "Plain axes"),
                    (Frame::Box, "Box"),
                    (Frame::Offset, "Offset"),
                ],
            );
        });
        ui.horizontal(|ui| {
            ui.label("Legend");
            changed |= enum_combo(
                ui,
                "legend",
                &mut spec.legend.position,
                &[
                    (LegendPosition::Right, "Right"),
                    (LegendPosition::Left, "Left"),
                    (LegendPosition::Above, "Above"),
                    (LegendPosition::Below, "Below"),
                    (LegendPosition::None, "None"),
                ],
            );
            changed |= ui.checkbox(&mut spec.legend.frame, "frame").changed();
        });

        if matches!(spec.kind, PlotKind::Bar { .. }) {
            ui.horizontal(|ui| {
                ui.label("Bar gap");
                changed |= ui.add(Slider::new(&mut spec.bar_gap, 0.0..=0.9)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Group gap");
                changed |= ui.add(Slider::new(&mut spec.group_gap, 0.0..=0.9)).changed();
            });
        }

        ui.add_space(8.0);
        egui::CollapsingHeader::new("X axis").default_open(true).show(ui, |ui| {
            changed |= axis_ui(ui, &mut spec.x, "x");
        });
        egui::CollapsingHeader::new("Y axis").default_open(true).show(ui, |ui| {
            changed |= axis_ui(ui, &mut spec.y, "y");
        });

        ui.add_space(8.0);
        ui.heading("Datasets");
        let kind = spec.kind.clone();
        for (i, s) in spec.series.iter_mut().enumerate() {
            let open = *selected == Some(i);
            let header = egui::CollapsingHeader::new(if s.label.is_empty() {
                format!("Dataset {}", i + 1)
            } else {
                s.label.clone()
            })
            .id_salt(("series", i))
            .open(open.then_some(true));

            let r = header.show(ui, |ui| {
                changed |= series_ui(ui, s, i, &kind);
            });
            if r.header_response.clicked() {
                *selected = if open { None } else { Some(i) };
            }
        }
    });

    changed
}

fn plot_kind(ui: &mut egui::Ui, spec: &mut GraphSpec) -> bool {
    let mut changed = false;
    let current = kind_name(&spec.kind);

    ui.horizontal(|ui| {
        ui.label("Type");
        ComboBox::from_id_salt("plot_kind")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (name, make) in kind_choices() {
                    if ui.selectable_label(current == name, name).clicked() {
                        spec.kind = make();
                        changed = true;
                    }
                }
            });
    });

    // Settings that belong to the chosen family.
    match &mut spec.kind {
        PlotKind::XY { connect, area, spline } => {
            ui.horizontal(|ui| {
                changed |= ui.checkbox(connect, "Connect").changed();
                changed |= ui.checkbox(area, "Area").changed();
                changed |= ui.checkbox(spline, "Spline").changed();
            });
        }
        PlotKind::Bar { layout, horizontal, center, spread } => {
            ui.horizontal(|ui| {
                ui.label("Layout");
                changed |= enum_combo(
                    ui,
                    "bar_layout",
                    layout,
                    &[
                        (BarLayout::Interleaved, "Interleaved"),
                        (BarLayout::Stacked, "Stacked"),
                        (BarLayout::StackedPercent, "Stacked 100%"),
                        (BarLayout::Separated, "Separated"),
                    ],
                );
            });
            changed |= ui.checkbox(horizontal, "Horizontal").changed();
            changed |= center_spread(ui, center, spread);
        }
        PlotKind::ScatterDot { jitter, center, spread } => {
            ui.horizontal(|ui| {
                ui.label("Spread points");
                changed |= enum_combo(
                    ui,
                    "jitter",
                    jitter,
                    &[
                        (Jitter::Aligned, "Aligned"),
                        (Jitter::Random, "Random"),
                        (Jitter::None, "Stacked"),
                    ],
                );
            });
            changed |= center_spread(ui, center, spread);
        }
        PlotKind::BoxWhisker { style, horizontal, show_points } => {
            ui.horizontal(|ui| {
                ui.label("Whiskers");
                changed |= enum_combo(
                    ui,
                    "bw",
                    style,
                    &[
                        (BoxWhiskerStyle::Tukey, "Tukey"),
                        (BoxWhiskerStyle::MinMax, "Min to max"),
                        (BoxWhiskerStyle::Percentile10_90, "10-90 percentile"),
                        (BoxWhiskerStyle::Percentile5_95, "5-95 percentile"),
                        (BoxWhiskerStyle::MeanSd, "Mean with SD"),
                    ],
                );
            });
            ui.horizontal(|ui| {
                changed |= ui.checkbox(horizontal, "Horizontal").changed();
                changed |= ui.checkbox(show_points, "Show points").changed();
            });
        }
        PlotKind::Violin { show_quartiles, horizontal } => {
            ui.horizontal(|ui| {
                changed |= ui.checkbox(show_quartiles, "Quartile lines").changed();
                changed |= ui.checkbox(horizontal, "Horizontal").changed();
            });
        }
        PlotKind::Survival { show_censor_marks, as_percent } => {
            ui.horizontal(|ui| {
                changed |= ui.checkbox(show_censor_marks, "Censor marks").changed();
                changed |= ui.checkbox(as_percent, "Percent").changed();
            });
        }
        PlotKind::PartsOfWhole { donut_hole, start_angle, .. } => {
            ui.horizontal(|ui| {
                ui.label("Donut hole");
                changed |= ui.add(Slider::new(donut_hole, 0.0..=0.9)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Start angle");
                changed |= ui.add(DragValue::new(start_angle).range(0.0..=360.0).suffix("°")).changed();
            });
        }
        PlotKind::Heatmap { show_values, .. } => {
            changed |= ui.checkbox(show_values, "Show values").changed();
        }
        PlotKind::Bubble { .. } => {
            ui.label("Bubble size and colour follow the table's extra variables.");
        }
        PlotKind::Dendrogram { horizontal } => {
            changed |= ui.checkbox(horizontal, "Horizontal").changed();
        }
    }
    changed
}

fn center_spread(ui: &mut egui::Ui, center: &mut Center, spread: &mut Spread) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Centre");
        changed |= enum_combo(
            ui,
            "center",
            center,
            &[
                (Center::Mean, "Mean"),
                (Center::Median, "Median"),
                (Center::GeometricMean, "Geometric mean"),
                (Center::None, "All points"),
            ],
        );
    });
    ui.horizontal(|ui| {
        ui.label("Error bars");
        changed |= enum_combo(
            ui,
            "spread",
            spread,
            &[
                (Spread::Sd, "SD"),
                (Spread::Sem, "SEM"),
                (Spread::Ci, "95% CI"),
                (Spread::Range, "Range"),
                (Spread::FromTable, "From table"),
                (Spread::None, "None"),
            ],
        );
    });
    changed
}

fn axis_ui(ui: &mut egui::Ui, axis: &mut Axis, id: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Title");
        changed |= ui.text_edit_singleline(&mut axis.title.text).changed();
        changed |= ui.checkbox(&mut axis.title.visible, "show").changed();
    });

    ui.horizontal(|ui| {
        ui.label("Scale");
        let mut log = matches!(axis.scale, Scale::Log(_));
        let categorical = axis.scale == Scale::Categorical;
        if categorical {
            ui.weak("categorical");
        } else {
            if ui.checkbox(&mut log, "log10").changed() {
                axis.scale = if log { Scale::Log(10.0) } else { Scale::Linear };
                changed = true;
                if log {
                    axis.number_format = NumberFormat::Power10;
                }
            }
        }
        changed |= ui.checkbox(&mut axis.reversed, "reverse").changed();
    });

    ui.horizontal(|ui| {
        ui.label("Range");
        let mut auto = axis.range.is_auto();
        if ui.checkbox(&mut auto, "auto").changed() {
            axis.range = if auto { Range::default() } else { Range { min: Some(0.0), max: Some(1.0) } };
            changed = true;
        }
        if !auto {
            let mut lo = axis.range.min.unwrap_or(0.0);
            let mut hi = axis.range.max.unwrap_or(1.0);
            if ui.add(DragValue::new(&mut lo).speed(0.1)).changed() {
                axis.range.min = Some(lo);
                changed = true;
            }
            if ui.add(DragValue::new(&mut hi).speed(0.1)).changed() {
                axis.range.max = Some(hi);
                changed = true;
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Ticks");
        changed |= enum_combo(
            ui,
            &format!("{id}_tickdir"),
            &mut axis.tick_direction,
            &[
                (TickDirection::Out, "Out"),
                (TickDirection::In, "In"),
                (TickDirection::Both, "Both"),
                (TickDirection::None, "None"),
            ],
        );
        ui.label("minor");
        changed |= ui.add(DragValue::new(&mut axis.minor_ticks).range(0..=9)).changed();
    });

    ui.horizontal(|ui| {
        ui.label("Numbers");
        changed |= enum_combo(
            ui,
            &format!("{id}_numfmt"),
            &mut axis.number_format,
            &[
                (NumberFormat::Decimal, "Decimal"),
                (NumberFormat::Scientific, "Scientific"),
                (NumberFormat::Power10, "Power of 10"),
                (NumberFormat::Antilog, "Antilog"),
                (NumberFormat::Percent, "Percent"),
            ],
        );
    });

    ui.horizontal(|ui| {
        changed |= ui.checkbox(&mut axis.grid, "Grid lines").changed();
        changed |= ui.checkbox(&mut axis.visible, "Show axis").changed();
    });
    changed
}

fn series_ui(ui: &mut egui::Ui, s: &mut Series, index: usize, kind: &PlotKind) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Label");
        changed |= ui.text_edit_singleline(&mut s.label).changed();
    });
    changed |= ui.checkbox(&mut s.visible, "Visible").changed();

    let bars_like = matches!(
        kind,
        PlotKind::Bar { .. }
            | PlotKind::BoxWhisker { .. }
            | PlotKind::Violin { .. }
            | PlotKind::PartsOfWhole { .. }
    );

    if bars_like {
        ui.horizontal(|ui| {
            ui.label("Fill");
            changed |= color_edit(ui, &mut s.fill);
            ui.label("Border");
            changed |= color_edit(ui, &mut s.border.color);
            changed |= ui.add(DragValue::new(&mut s.border.width).range(0.0..=6.0).speed(0.1)).changed();
        });
    } else {
        ui.horizontal(|ui| {
            ui.label("Symbol");
            changed |= enum_combo(
                ui,
                &format!("sym{index}"),
                &mut s.symbol.shape,
                &[
                    (SymbolShape::Circle, "Circle"),
                    (SymbolShape::Square, "Square"),
                    (SymbolShape::Triangle, "Triangle up"),
                    (SymbolShape::TriangleDown, "Triangle down"),
                    (SymbolShape::Diamond, "Diamond"),
                    (SymbolShape::Plus, "Plus"),
                    (SymbolShape::Cross, "Cross"),
                    (SymbolShape::Star, "Star"),
                    (SymbolShape::Dash, "Dash"),
                    (SymbolShape::None, "None"),
                ],
            );
            changed |= ui.add(DragValue::new(&mut s.symbol.size).range(0.0..=30.0).speed(0.2)).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Fill");
            changed |= color_edit(ui, &mut s.symbol.fill);
            ui.label("Edge");
            changed |= color_edit(ui, &mut s.symbol.border);
        });
        ui.horizontal(|ui| {
            ui.label("Line");
            changed |= enum_combo(
                ui,
                &format!("line{index}"),
                &mut s.line.style,
                &[
                    (LineStyle::Solid, "Solid"),
                    (LineStyle::Dash, "Dash"),
                    (LineStyle::Dot, "Dot"),
                    (LineStyle::DashDot, "Dash-dot"),
                    (LineStyle::None, "None"),
                ],
            );
            changed |= color_edit(ui, &mut s.line.color);
            changed |= ui.add(DragValue::new(&mut s.line.width).range(0.0..=8.0).speed(0.1)).changed();
        });
    }

    ui.horizontal(|ui| {
        ui.label("Error bars");
        changed |= enum_combo(
            ui,
            &format!("eb{index}"),
            &mut s.error_bars.style,
            &[
                (ErrorBarStyle::Both, "Both"),
                (ErrorBarStyle::Up, "Up only"),
                (ErrorBarStyle::Down, "Down only"),
                (ErrorBarStyle::None, "None"),
            ],
        );
        changed |= ui.add(DragValue::new(&mut s.error_bars.cap_width).range(0.0..=20.0).speed(0.2)).changed();
    });
    changed |= ui.checkbox(&mut s.secondary_axis, "Plot on right Y axis").changed();
    changed
}

fn color_edit(ui: &mut egui::Ui, c: &mut Color) -> bool {
    let mut rgba = [c.r, c.g, c.b, c.a];
    if ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed() {
        *c = Color { r: rgba[0], g: rgba[1], b: rgba[2], a: rgba[3] };
        true
    } else {
        false
    }
}

fn enum_combo<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    let current = options
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, n)| *n)
        .unwrap_or("");
    ComboBox::from_id_salt(id).selected_text(current).show_ui(ui, |ui| {
        for (v, name) in options {
            if ui.selectable_label(v == value, *name).clicked() {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

pub fn kind_name(kind: &PlotKind) -> &'static str {
    match kind {
        PlotKind::XY { .. } => "XY",
        PlotKind::Bar { .. } => "Bar",
        PlotKind::ScatterDot { .. } => "Column scatter",
        PlotKind::BoxWhisker { .. } => "Box and whisker",
        PlotKind::Violin { .. } => "Violin",
        PlotKind::Survival { .. } => "Survival",
        PlotKind::PartsOfWhole { .. } => "Pie",
        PlotKind::Heatmap { .. } => "Heatmap",
        PlotKind::Bubble { .. } => "Bubble",
        PlotKind::Dendrogram { .. } => "Dendrogram",
    }
}

#[allow(clippy::type_complexity)]
fn kind_choices() -> Vec<(&'static str, fn() -> PlotKind)> {
    vec![
        ("XY", || PlotKind::XY { connect: false, area: false, spline: false }),
        ("Bar", || PlotKind::Bar {
            layout: BarLayout::Interleaved,
            horizontal: false,
            center: Center::Mean,
            spread: Spread::Sd,
        }),
        ("Column scatter", || PlotKind::ScatterDot {
            jitter: Jitter::Aligned,
            center: Center::Mean,
            spread: Spread::Sd,
        }),
        ("Box and whisker", || PlotKind::BoxWhisker {
            style: BoxWhiskerStyle::Tukey,
            horizontal: false,
            show_points: false,
        }),
        ("Violin", || PlotKind::Violin { show_quartiles: true, horizontal: false }),
        ("Survival", || PlotKind::Survival { show_censor_marks: true, as_percent: true }),
        ("Pie", || PlotKind::PartsOfWhole {
            donut_hole: 0.0,
            start_angle: 90.0,
            explode: Vec::new(),
        }),
        ("Heatmap", || PlotKind::Heatmap {
            colormap: Colormap::default(),
            show_values: false,
        }),
        ("Bubble", || PlotKind::Bubble {
            size_variable: None,
            color_variable: None,
            colormap: Colormap::default(),
        }),
        ("Dendrogram", || PlotKind::Dendrogram { horizontal: false }),
    ]
}
