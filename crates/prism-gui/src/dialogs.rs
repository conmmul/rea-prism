//! The modal dialogs, organised the way Prism organises them.
//!
//! The grouping and tab structure here follow the layout recovered from Prism's
//! own Interface Builder archives: a Change Graph Type sheet with one tab per
//! table family, a Format Graph sheet with Appearance / Datasets / Graph Settings
//! tabs, and a Format Axes sheet with Axis / Frame and Origin / Titles tabs.
//! The wording and the controls themselves are this project's own.

use egui::{DragValue, RichText, Ui};
use prism_doc::graph::*;
use prism_doc::model::{DataTable, TableFormat};

use crate::widgets::{color, combo, group, line_style, points, row, stroke, symbol_shape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    ChangeGraphType,
    FormatGraph,
    FormatAxes,
}

/// Which tab each dialog is showing, and which dataset or axis it is editing.
pub struct DialogState {
    pub open: Option<Dialog>,
    pub graph_tab: GraphTab,
    pub axes_tab: AxesTab,
    /// The dataset the Appearance tab is editing.
    pub dataset: usize,
    /// The axis the Axis tab is editing.
    pub axis: AxisChoice,
    /// Set when the user accepts a dialog, so the caller can commit.
    pub accepted: bool,
}

impl Default for DialogState {
    fn default() -> Self {
        DialogState {
            open: None,
            graph_tab: GraphTab::Appearance,
            axes_tab: AxesTab::Axis,
            dataset: 0,
            axis: AxisChoice::X,
            accepted: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphTab {
    Appearance,
    Datasets,
    GraphSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxesTab {
    Axis,
    FrameAndOrigin,
    Titles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisChoice {
    X,
    LeftY,
    RightY,
}

/// Show whichever dialog is open. Returns true when the spec changed.
pub fn show(
    ctx: &egui::Context,
    state: &mut DialogState,
    spec: &mut GraphSpec,
    table: &DataTable,
) -> bool {
    let Some(which) = state.open else { return false };
    let mut changed = false;
    let mut keep_open = true;

    let title = match which {
        Dialog::ChangeGraphType => "Change Graph Type",
        Dialog::FormatGraph => "Format Graph",
        Dialog::FormatAxes => "Format Axes",
    };

    egui::Window::new(title)
        .open(&mut keep_open)
        .resizable(true)
        .default_width(560.0)
        .default_height(480.0)
        .collapsible(false)
        .show(ctx, |ui| {
            match which {
                Dialog::ChangeGraphType => changed |= change_graph_type(ui, state, spec, table),
                Dialog::FormatGraph => changed |= format_graph(ui, state, spec, table),
                Dialog::FormatAxes => changed |= format_axes(ui, state, spec),
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Done").clicked() {
                        state.accepted = true;
                    }
                });
            });
        });

    if !keep_open || state.accepted {
        state.open = None;
        state.accepted = false;
    }
    changed
}

// ---------------------------------------------------------------------------
// Change Graph Type
// ---------------------------------------------------------------------------

/// The graph families Prism offers for a given table kind.
fn families_for(format: TableFormat) -> Vec<(&'static str, fn() -> PlotKind)> {
    let xy: (&str, fn() -> PlotKind) =
        ("Points only", || PlotKind::XY { connect: false, area: false, spline: false });
    let line: (&str, fn() -> PlotKind) =
        ("Points and line", || PlotKind::XY { connect: true, area: false, spline: false });
    let spline: (&str, fn() -> PlotKind) =
        ("Smoothed curve", || PlotKind::XY { connect: true, area: false, spline: true });
    let area: (&str, fn() -> PlotKind) =
        ("Area fill", || PlotKind::XY { connect: true, area: true, spline: false });

    let scatter: (&str, fn() -> PlotKind) = ("Scatter with mean", || PlotKind::ScatterDot {
        jitter: Jitter::Aligned,
        center: Center::Mean,
        spread: Spread::Sd,
    });
    let bar: (&str, fn() -> PlotKind) = ("Bars", || PlotKind::Bar {
        layout: BarLayout::Interleaved,
        horizontal: false,
        center: Center::Mean,
        spread: Spread::Sd,
    });
    let stacked: (&str, fn() -> PlotKind) = ("Stacked bars", || PlotKind::Bar {
        layout: BarLayout::Stacked,
        horizontal: false,
        center: Center::Mean,
        spread: Spread::None,
    });
    let pct: (&str, fn() -> PlotKind) = ("Stacked to 100%", || PlotKind::Bar {
        layout: BarLayout::StackedPercent,
        horizontal: false,
        center: Center::Mean,
        spread: Spread::None,
    });
    let boxw: (&str, fn() -> PlotKind) = ("Box and whisker", || PlotKind::BoxWhisker {
        style: BoxWhiskerStyle::Tukey,
        horizontal: false,
        show_points: false,
    });
    let violin: (&str, fn() -> PlotKind) =
        ("Violin", || PlotKind::Violin { show_quartiles: true, horizontal: false });
    let pie: (&str, fn() -> PlotKind) = ("Pie", || PlotKind::PartsOfWhole {
        donut_hole: 0.0,
        start_angle: 90.0,
        explode: Vec::new(),
    });
    let donut: (&str, fn() -> PlotKind) = ("Donut", || PlotKind::PartsOfWhole {
        donut_hole: 0.45,
        start_angle: 90.0,
        explode: Vec::new(),
    });
    let surv: (&str, fn() -> PlotKind) = ("Survival staircase", || PlotKind::Survival {
        show_censor_marks: true,
        as_percent: true,
    });
    let heat: (&str, fn() -> PlotKind) =
        ("Heat map", || PlotKind::Heatmap { colormap: Colormap::default(), show_values: false });
    let bubble: (&str, fn() -> PlotKind) = ("Bubble plot", || PlotKind::Bubble {
        size_variable: None,
        color_variable: None,
        colormap: Colormap::default(),
    });
    let dendro: (&str, fn() -> PlotKind) =
        ("Dendrogram", || PlotKind::Dendrogram { horizontal: false });

    match format {
        TableFormat::XY => vec![xy, line, spline, area, bar],
        TableFormat::Column | TableFormat::Nested => {
            vec![scatter, bar, boxw, violin, line]
        }
        TableFormat::Grouped | TableFormat::Contingency => {
            vec![bar, stacked, pct, scatter, boxw, line]
        }
        TableFormat::Survival => vec![surv, line, xy],
        TableFormat::PartsOfWhole => vec![pie, donut, bar, pct],
        TableFormat::MultiVariable => vec![xy, bubble, heat, dendro, line],
    }
}

fn family_name(format: TableFormat) -> &'static str {
    match format {
        TableFormat::XY => "XY",
        TableFormat::Column => "Column",
        TableFormat::Grouped => "Grouped",
        TableFormat::Contingency => "Contingency",
        TableFormat::Survival => "Survival",
        TableFormat::PartsOfWhole => "Parts of whole",
        TableFormat::MultiVariable => "Multiple variables",
        TableFormat::Nested => "Nested",
    }
}

fn change_graph_type(
    ui: &mut Ui,
    _state: &mut DialogState,
    spec: &mut GraphSpec,
    table: &DataTable,
) -> bool {
    let mut changed = false;

    // Prism shows one tab per table family and enables only the one that matches
    // the data. We show the same, with the others disabled rather than hidden.
    ui.horizontal_wrapped(|ui| {
        for f in [
            TableFormat::XY,
            TableFormat::Column,
            TableFormat::Grouped,
            TableFormat::Contingency,
            TableFormat::Survival,
            TableFormat::PartsOfWhole,
            TableFormat::MultiVariable,
            TableFormat::Nested,
        ] {
            let active = f == table.format;
            ui.add_enabled(active, egui::Button::new(family_name(f)).selected(active));
        }
    });
    ui.separator();
    ui.label(
        RichText::new(format!(
            "This sheet is a {} table.",
            family_name(table.format)
        ))
        .small()
        .weak(),
    );
    ui.add_space(4.0);

    let current = crate::inspector::kind_name(&spec.kind);
    egui::Grid::new("graph_kinds").num_columns(3).spacing([8.0, 8.0]).show(ui, |ui| {
        for (i, (name, make)) in families_for(table.format).into_iter().enumerate() {
            let selected = name == current;
            if ui
                .add_sized(
                    [160.0, 48.0],
                    egui::Button::new(RichText::new(name)).selected(selected),
                )
                .clicked()
            {
                spec.kind = make();
                // Keep the categorical axis in step with the new family.
                retarget_axis(spec, table);
                changed = true;
            }
            if i % 3 == 2 {
                ui.end_row();
            }
        }
    });

    changed
}

/// After a graph-type change, make sure the X axis still describes the slots.
fn retarget_axis(spec: &mut GraphSpec, table: &DataTable) {
    let fresh = GraphSpec::default_for_table(table);
    let categorical = fresh.x.scale == Scale::Categorical;
    if categorical && spec.x.scale != Scale::Categorical {
        spec.x.scale = Scale::Categorical;
        spec.x.categories = fresh.x.categories;
        spec.slots = fresh.slots;
    } else if !categorical && spec.x.scale == Scale::Categorical {
        spec.x.scale = Scale::Linear;
        spec.x.categories.clear();
        spec.slots = fresh.slots;
    }
}

// ---------------------------------------------------------------------------
// Format Graph
// ---------------------------------------------------------------------------

fn format_graph(
    ui: &mut Ui,
    state: &mut DialogState,
    spec: &mut GraphSpec,
    table: &DataTable,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (tab, name) in [
            (GraphTab::Appearance, "Appearance"),
            (GraphTab::Datasets, "Datasets"),
            (GraphTab::GraphSettings, "Graph Settings"),
        ] {
            if ui.selectable_label(state.graph_tab == tab, name).clicked() {
                state.graph_tab = tab;
            }
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        match state.graph_tab {
            GraphTab::Appearance => changed |= appearance_tab(ui, state, spec),
            GraphTab::Datasets => changed |= datasets_tab(ui, state, spec, table),
            GraphTab::GraphSettings => changed |= graph_settings_tab(ui, spec),
        }
    });
    changed
}

fn appearance_tab(ui: &mut Ui, state: &mut DialogState, spec: &mut GraphSpec) -> bool {
    let mut changed = false;
    if spec.series.is_empty() {
        ui.label("This graph has no datasets.");
        return false;
    }
    state.dataset = state.dataset.min(spec.series.len() - 1);

    // Prism edits one dataset at a time, with a picker and step buttons.
    ui.horizontal(|ui| {
        ui.label("Dataset");
        let names: Vec<String> = spec.series.iter().map(|s| s.label.clone()).collect();
        egui::ComboBox::from_id_salt("appearance_dataset")
            .selected_text(names[state.dataset].clone())
            .show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(state.dataset == i, n).clicked() {
                        state.dataset = i;
                    }
                }
            });
        if ui.add_enabled(state.dataset > 0, egui::Button::new("◀")).clicked() {
            state.dataset -= 1;
        }
        if ui
            .add_enabled(state.dataset + 1 < spec.series.len(), egui::Button::new("▶"))
            .clicked()
        {
            state.dataset += 1;
        }
    });

    let kind = spec.kind.clone();
    let s = &mut spec.series[state.dataset];

    changed |= group(ui, "Symbols", |ui| {
        let mut c = ui.checkbox(&mut s.show_symbols, "Show symbols").changed();
        ui.add_enabled_ui(s.show_symbols, |ui| {
            c |= row(ui, "Shape", |ui| symbol_shape(ui, "sym_shape", &mut s.symbol.shape));
            c |= row(ui, "Size", |ui| {
                points(ui, &mut s.symbol.size, 0.0..=40.0).changed()
            });
            c |= row(ui, "Fill", |ui| color(ui, &mut s.symbol.fill));
            c |= row(ui, "Border", |ui| {
                let mut x = color(ui, &mut s.symbol.border);
                x |= points(ui, &mut s.symbol.border_width, 0.0..=8.0).changed();
                x
            });
            c |= ui.checkbox(&mut s.label_points, "Label each point with its value").changed();
        });
        c
    });

    changed |= group(ui, "Lines", |ui| {
        let mut c = ui.checkbox(&mut s.show_line, "Show line").changed();
        ui.add_enabled_ui(s.show_line, |ui| {
            c |= row(ui, "Style", |ui| stroke(ui, "line", &mut s.line));
        });
        c
    });

    if matches!(kind, PlotKind::XY { .. }) {
        changed |= group(ui, "Area fill", |ui| {
            let mut c = row(ui, "Fill", |ui| {
                combo(
                    ui,
                    "area_fill",
                    &mut s.area_fill,
                    &[
                        (AreaFill::None, "None"),
                        (AreaFill::Below, "To baseline"),
                        (AreaFill::Above, "To top"),
                    ],
                )
            });
            ui.add_enabled_ui(s.area_fill != AreaFill::None, |ui| {
                c |= row(ui, "Colour", |ui| color(ui, &mut s.area_color));
            });
            c
        });
    }

    if matches!(kind, PlotKind::Bar { .. } | PlotKind::BoxWhisker { .. } | PlotKind::Violin { .. })
    {
        changed |= group(ui, "Bars", |ui| {
            let mut c = ui.checkbox(&mut s.show_bars, "Show bars").changed();
            ui.add_enabled_ui(s.show_bars, |ui| {
                c |= row(ui, "Fill", |ui| color(ui, &mut s.fill));
                c |= row(ui, "Border", |ui| stroke(ui, "bar_border", &mut s.border));
                c |= row(ui, "Width", |ui| {
                    ui.add(DragValue::new(&mut s.bar_width).range(0.05..=2.0).speed(0.02)).changed()
                });
            });
            c
        });
    }

    changed |= group(ui, "Error bars", |ui| {
        let mut c = ui.checkbox(&mut s.show_error_bars, "Show error bars").changed();
        ui.add_enabled_ui(s.show_error_bars, |ui| {
            c |= row(ui, "Direction", |ui| {
                combo(
                    ui,
                    "err_dir",
                    &mut s.error_bars.style,
                    &[
                        (ErrorBarStyle::Both, "Both"),
                        (ErrorBarStyle::Up, "Up only"),
                        (ErrorBarStyle::Down, "Down only"),
                        (ErrorBarStyle::None, "None"),
                    ],
                )
            });
            c |= row(ui, "Style", |ui| stroke(ui, "err", &mut s.error_bars.stroke));
            c |= row(ui, "Cap width", |ui| {
                points(ui, &mut s.error_bars.cap_width, 0.0..=24.0).changed()
            });
        });
        c
    });

    changed |= group(ui, "Placement", |ui| {
        let mut c = ui.checkbox(&mut s.in_legend, "Include in legend").changed();
        c |= ui.checkbox(&mut s.secondary_axis, "Plot on the right Y axis").changed();
        c |= row(ui, "Nudge", |ui| {
            let mut x = ui.add(DragValue::new(&mut s.offset.0).speed(0.2).prefix("x ")).changed();
            x |= ui.add(DragValue::new(&mut s.offset.1).speed(0.2).prefix("y ")).changed();
            x
        });
        c
    });

    changed
}

fn datasets_tab(
    ui: &mut Ui,
    state: &mut DialogState,
    spec: &mut GraphSpec,
    table: &DataTable,
) -> bool {
    let mut changed = false;
    ui.label(RichText::new("Order and visibility of the plotted datasets.").small().weak());
    ui.add_space(4.0);

    let mut move_from_to: Option<(usize, usize)> = None;
    egui::Grid::new("datasets").num_columns(3).striped(true).show(ui, |ui| {
        for i in 0..spec.series.len() {
            let selected = state.dataset == i;
            if ui.selectable_label(selected, format!("{}", i + 1)).clicked() {
                state.dataset = i;
            }
            let s = &mut spec.series[i];
            changed |= ui.checkbox(&mut s.visible, "").changed();
            changed |= ui.text_edit_singleline(&mut s.label).changed();
            ui.end_row();
        }
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let n = spec.series.len();
        let i = state.dataset;
        if ui.add_enabled(i > 0, egui::Button::new("Move up")).clicked() {
            move_from_to = Some((i, i - 1));
        }
        if ui.add_enabled(i + 1 < n, egui::Button::new("Move down")).clicked() {
            move_from_to = Some((i, i + 1));
        }
        if ui.add_enabled(n > 1, egui::Button::new("Reverse order")).clicked() {
            spec.series.reverse();
            changed = true;
        }
    });
    if let Some((from, to)) = move_from_to {
        spec.series.swap(from, to);
        state.dataset = to;
        changed = true;
    }

    if matches!(table.format, TableFormat::Grouped | TableFormat::Contingency) {
        changed |= group(ui, "Grouping", |ui| {
            let mut c = false;
            if let PlotKind::Bar { layout, .. } = &mut spec.kind {
                c |= row(ui, "Stacking", |ui| {
                    combo(
                        ui,
                        "stacking",
                        layout,
                        &[
                            (BarLayout::Interleaved, "Side by side"),
                            (BarLayout::Stacked, "Stacked"),
                            (BarLayout::StackedPercent, "Stacked to 100%"),
                            (BarLayout::Separated, "Separated"),
                        ],
                    )
                });
            }
            c |= row(ui, "Gap between groups", |ui| {
                ui.add(egui::Slider::new(&mut spec.group_gap, 0.0..=0.9)).changed()
            });
            c
        });
    }
    changed
}

fn graph_settings_tab(ui: &mut Ui, spec: &mut GraphSpec) -> bool {
    let mut changed = false;

    if matches!(spec.kind, PlotKind::Bar { .. }) {
        changed |= group(ui, "Bars", |ui| {
            let mut c = row(ui, "Direction", |ui| {
                combo(
                    ui,
                    "direction",
                    &mut spec.direction,
                    &[(Direction::Vertical, "Vertical"), (Direction::Horizontal, "Horizontal")],
                )
            });
            c |= row(ui, "Baseline at", |ui| {
                ui.add(DragValue::new(&mut spec.baseline).speed(0.1)).changed()
            });
            c |= ui.checkbox(&mut spec.hide_baseline, "Hide the baseline").changed();
            c |= row(ui, "Gap between bars", |ui| {
                ui.add(egui::Slider::new(&mut spec.bar_gap, 0.0..=0.9)).changed()
            });
            c
        });
    }

    if matches!(spec.kind, PlotKind::XY { .. }) {
        changed |= group(ui, "Lines", |ui| {
            let mut c = row(ui, "Missing values", |ui| {
                combo(
                    ui,
                    "missing",
                    &mut spec.missing_values,
                    &[
                        (MissingValues::Connect, "Join across the gap"),
                        (MissingValues::Gap, "Break the line"),
                    ],
                )
            });
            let mut show_identity = spec.identity_line.is_some();
            if ui.checkbox(&mut show_identity, "Draw a y = x reference line").changed() {
                spec.identity_line = show_identity
                    .then(|| Stroke { style: LineStyle::Dash, width: 1.0, color: Color::rgb(140, 140, 140) });
                c = true;
            }
            if let Some(l) = &mut spec.identity_line {
                c |= row(ui, "Reference line", |ui| stroke(ui, "identity", l));
            }
            c
        });
    }

    changed |= group(ui, "Legend", |ui| {
        let mut c = row(ui, "Position", |ui| {
            combo(
                ui,
                "legend_pos",
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
        c |= ui.checkbox(&mut spec.legend.frame, "Draw a box around the legend").changed();
        c |= row(ui, "Text size", |ui| {
            points(ui, &mut spec.legend.style.size, 5.0..=24.0).changed()
        });
        c
    });

    changed
}

// ---------------------------------------------------------------------------
// Format Axes
// ---------------------------------------------------------------------------

fn format_axes(ui: &mut Ui, state: &mut DialogState, spec: &mut GraphSpec) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (tab, name) in [
            (AxesTab::Axis, "Axis"),
            (AxesTab::FrameAndOrigin, "Frame and Origin"),
            (AxesTab::Titles, "Titles"),
        ] {
            if ui.selectable_label(state.axes_tab == tab, name).clicked() {
                state.axes_tab = tab;
            }
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        match state.axes_tab {
            AxesTab::Axis => changed |= axis_tab(ui, state, spec),
            AxesTab::FrameAndOrigin => changed |= frame_tab(ui, spec),
            AxesTab::Titles => changed |= titles_tab(ui, state, spec),
        }
    });
    changed
}

/// The axis picker shared by the Axis and Titles tabs.
fn axis_picker(ui: &mut Ui, state: &mut DialogState, has_right: bool) {
    ui.horizontal(|ui| {
        ui.label("Axis");
        for (c, name) in [(AxisChoice::X, "X"), (AxisChoice::LeftY, "Left Y")] {
            if ui.selectable_label(state.axis == c, name).clicked() {
                state.axis = c;
            }
        }
        if ui
            .add_enabled(
                has_right,
                egui::Button::new("Right Y").selected(state.axis == AxisChoice::RightY),
            )
            .clicked()
        {
            state.axis = AxisChoice::RightY;
        }
    });
}

fn pick<'a>(spec: &'a mut GraphSpec, which: AxisChoice) -> &'a mut Axis {
    match which {
        AxisChoice::X => &mut spec.x,
        AxisChoice::LeftY => &mut spec.y,
        AxisChoice::RightY => spec.y2.get_or_insert_with(Axis::default),
    }
}

fn axis_tab(ui: &mut Ui, state: &mut DialogState, spec: &mut GraphSpec) -> bool {
    let has_right = spec.series.iter().any(|s| s.secondary_axis);
    axis_picker(ui, state, has_right);
    ui.separator();

    let which = state.axis;
    let axis = pick(spec, which);
    let mut changed = false;

    changed |= group(ui, "Scale", |ui| {
        let mut c = false;
        if axis.scale == Scale::Categorical {
            ui.label(RichText::new("Categorical: one slot per column or row.").small().weak());
        } else {
            let mut log = matches!(axis.scale, Scale::Log(_));
            if ui.checkbox(&mut log, "Logarithmic (base 10)").changed() {
                axis.scale = if log { Scale::Log(10.0) } else { Scale::Linear };
                if log {
                    axis.number_format = NumberFormat::Power10;
                }
                c = true;
            }
        }
        c |= ui.checkbox(&mut axis.reversed, "Reverse the direction").changed();
        c |= ui.checkbox(&mut axis.visible, "Show this axis").changed();
        c
    });

    changed |= group(ui, "Range", |ui| {
        let mut auto = axis.range.is_auto();
        let mut c = false;
        if ui.checkbox(&mut auto, "Choose automatically").changed() {
            axis.range = if auto {
                Range::default()
            } else {
                Range { min: Some(0.0), max: Some(1.0) }
            };
            c = true;
        }
        ui.add_enabled_ui(!auto, |ui| {
            let mut lo = axis.range.min.unwrap_or(0.0);
            let mut hi = axis.range.max.unwrap_or(1.0);
            c |= row(ui, "Minimum", |ui| {
                let r = ui.add(DragValue::new(&mut lo).speed(0.1)).changed();
                if r {
                    axis.range.min = Some(lo);
                }
                r
            });
            c |= row(ui, "Maximum", |ui| {
                let r = ui.add(DragValue::new(&mut hi).speed(0.1)).changed();
                if r {
                    axis.range.max = Some(hi);
                }
                r
            });
        });
        c
    });

    changed |= group(ui, "Ticks", |ui| {
        let mut c = row(ui, "Direction", |ui| {
            combo(
                ui,
                "tick_dir",
                &mut axis.tick_direction,
                &[
                    (TickDirection::Out, "Outward"),
                    (TickDirection::In, "Inward"),
                    (TickDirection::Both, "Both"),
                    (TickDirection::None, "None"),
                ],
            )
        });
        c |= row(ui, "Length", |ui| points(ui, &mut axis.tick_length, 0.0..=20.0).changed());
        let mut auto_interval = axis.major_interval.is_none();
        if ui.checkbox(&mut auto_interval, "Automatic interval").changed() {
            axis.major_interval = if auto_interval { None } else { Some(1.0) };
            c = true;
        }
        if let Some(iv) = &mut axis.major_interval {
            c |= row(ui, "Major every", |ui| {
                ui.add(DragValue::new(iv).speed(0.1).range(1e-9..=f64::MAX)).changed()
            });
        }
        c |= row(ui, "Minor per major", |ui| {
            ui.add(DragValue::new(&mut axis.minor_ticks).range(0..=9)).changed()
        });
        c
    });

    changed |= group(ui, "Numbering", |ui| {
        let mut c = row(ui, "Format", |ui| {
            combo(
                ui,
                "num_fmt",
                &mut axis.number_format,
                &[
                    (NumberFormat::Decimal, "Decimal"),
                    (NumberFormat::Scientific, "Scientific"),
                    (NumberFormat::Power10, "Power of 10"),
                    (NumberFormat::Antilog, "Antilog"),
                    (NumberFormat::Percent, "Percentage"),
                ],
            )
        });
        let mut auto_dp = axis.decimals.is_none();
        if ui.checkbox(&mut auto_dp, "Automatic decimal places").changed() {
            axis.decimals = if auto_dp { None } else { Some(2) };
            c = true;
        }
        if let Some(d) = &mut axis.decimals {
            c |= row(ui, "Decimals", |ui| {
                ui.add(DragValue::new(d).range(0..=10)).changed()
            });
        }
        c |= ui.checkbox(&mut axis.thousands_separator, "Group thousands").changed();
        c |= row(ui, "Prefix", |ui| ui.text_edit_singleline(&mut axis.prefix).changed());
        c |= row(ui, "Suffix", |ui| ui.text_edit_singleline(&mut axis.suffix).changed());
        c |= row(ui, "Rotation", |ui| {
            ui.add(DragValue::new(&mut axis.label_rotation).range(-90.0..=90.0).suffix("°"))
                .changed()
        });
        c |= row(ui, "Text size", |ui| {
            points(ui, &mut axis.label_style.size, 5.0..=24.0).changed()
        });
        c
    });

    changed
}

fn frame_tab(ui: &mut Ui, spec: &mut GraphSpec) -> bool {
    let mut changed = false;

    changed |= group(ui, "Frame", |ui| {
        let mut c = row(ui, "Style", |ui| {
            combo(
                ui,
                "frame_style",
                &mut spec.frame,
                &[
                    (Frame::PlainAxes, "Two plain axes"),
                    (Frame::Box, "Box around the plot"),
                    (Frame::Offset, "Offset axes"),
                ],
            )
        });
        c |= row(ui, "Axis line", |ui| {
            let mut x = points(ui, &mut spec.axis_width, 0.0..=8.0).changed();
            x |= color(ui, &mut spec.axis_color);
            x
        });
        c
    });

    changed |= group(ui, "Size", |ui| {
        let mut c = row(ui, "Width", |ui| points(ui, &mut spec.size.0, 80.0..=2000.0).changed());
        c |= row(ui, "Height", |ui| points(ui, &mut spec.size.1, 60.0..=2000.0).changed());
        c
    });

    changed |= group(ui, "Grid lines", |ui| {
        let mut c = false;
        for (name, axis) in [("X axis", &mut spec.x), ("Y axis", &mut spec.y)] {
            ui.label(RichText::new(name).small().weak());
            c |= ui.checkbox(&mut axis.grid, "Show grid lines").changed();
            ui.add_enabled_ui(axis.grid, |ui| {
                c |= row(ui, "Style", |ui| {
                    let mut x = line_style(ui, format!("{name}_grid_style"), &mut axis.grid_style);
                    x |= points(ui, &mut axis.grid_width, 0.0..=4.0).changed();
                    x |= color(ui, &mut axis.grid_color);
                    x
                });
                c |= ui.checkbox(&mut axis.minor_grid, "Include minor ticks").changed();
            });
        }
        c
    });

    changed |= group(ui, "Colours", |ui| {
        let mut c = row(ui, "Page background", |ui| color(ui, &mut spec.background));
        c |= row(ui, "Plotting area", |ui| color(ui, &mut spec.plot_background));
        c
    });

    changed
}

fn titles_tab(ui: &mut Ui, state: &mut DialogState, spec: &mut GraphSpec) -> bool {
    let mut changed = false;

    changed |= group(ui, "Graph title", |ui| {
        let mut c = ui.checkbox(&mut spec.title.visible, "Show the graph title").changed();
        ui.add_enabled_ui(spec.title.visible, |ui| {
            c |= row(ui, "Text", |ui| ui.text_edit_singleline(&mut spec.title.text).changed());
            c |= row(ui, "Size", |ui| {
                points(ui, &mut spec.title.style.size, 6.0..=36.0).changed()
            });
            c |= ui.checkbox(&mut spec.title.style.bold, "Bold").changed();
        });
        c
    });

    let has_right = spec.series.iter().any(|s| s.secondary_axis);
    axis_picker(ui, state, has_right);
    let which = state.axis;
    let axis = pick(spec, which);

    changed |= group(ui, "Axis title", |ui| {
        let mut c = ui.checkbox(&mut axis.title.visible, "Show the axis title").changed();
        ui.add_enabled_ui(axis.title.visible, |ui| {
            c |= row(ui, "Text", |ui| ui.text_edit_singleline(&mut axis.title.text).changed());
            c |= row(ui, "Size", |ui| {
                points(ui, &mut axis.title.style.size, 5.0..=28.0).changed()
            });
            c |= ui.checkbox(&mut axis.title.style.bold, "Bold").changed();
            c |= row(ui, "Distance from axis", |ui| {
                points(ui, &mut axis.title_distance, 0.0..=60.0).changed()
            });
        });
        c
    });

    changed |= group(ui, "Numbering", |ui| {
        row(ui, "Distance from axis", |ui| {
            points(ui, &mut axis.numbering_distance, 0.0..=40.0).changed()
        })
    });

    changed
}
