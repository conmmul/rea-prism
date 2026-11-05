//! Graph appearance model.
//!
//! Prism's classic graphs keep their appearance in an undocumented binary blob, so
//! this model is our own description of what a Prism graph *is*, assembled from the
//! graph JSON that the multivariable graphs do expose (`Axis`, `LinearAxisSegment`,
//! `CategoricalAxisSegment`, `CustomAxisTick`, the colormap and legend classes) and
//! from the graph families Prism offers for each table kind. Classic graphs are
//! reconstructed from their data table with Prism's documented defaults.

use crate::model::{DataTable, TableFormat, Uid};

/// sRGB colour with alpha, matching the `#aarrggbb` strings in Prism's JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    pub const TRANSPARENT: Color = Color { r: 0, g: 0, b: 0, a: 0 };
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    /// Parse `#aarrggbb`, `#rrggbb` or `#rgb`.
    pub fn parse(s: &str) -> Option<Color> {
        let h = s.strip_prefix('#')?;
        let v = u32::from_str_radix(h, 16).ok()?;
        Some(match h.len() {
            8 => Color {
                a: (v >> 24) as u8,
                r: (v >> 16) as u8,
                g: (v >> 8) as u8,
                b: v as u8,
            },
            6 => Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8),
            3 => {
                let x = |n: u32| (((v >> n) & 0xf) * 17) as u8;
                Color::rgb(x(8), x(4), x(0))
            }
            _ => return None,
        })
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}{:02x}", self.a, self.r, self.g, self.b)
    }

    pub fn with_alpha(self, a: u8) -> Color {
        Color { a, ..self }
    }
}

/// Prism's default dataset colour cycle, read off the shipped colour schemes.
pub const DEFAULT_PALETTE: [Color; 10] = [
    Color::rgb(0, 0, 0),
    Color::rgb(220, 48, 42),
    Color::rgb(45, 92, 173),
    Color::rgb(0, 152, 70),
    Color::rgb(243, 146, 55),
    Color::rgb(128, 62, 151),
    Color::rgb(0, 174, 199),
    Color::rgb(236, 0, 140),
    Color::rgb(144, 144, 144),
    Color::rgb(140, 98, 57),
];

pub fn palette_color(index: usize) -> Color {
    DEFAULT_PALETTE[index % DEFAULT_PALETTE.len()]
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TextStyle {
    pub family: String,
    pub size: f32,
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            family: "Arial".into(),
            size: 10.0,
            color: Color::BLACK,
            bold: false,
            italic: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Label {
    pub text: String,
    pub visible: bool,
    pub style: TextStyle,
    /// Degrees, counter-clockwise. Prism rotates Y titles by 90.
    pub rotation: f32,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Label { text: text.into(), visible: true, ..Default::default() }
    }

    pub fn hidden() -> Self {
        Label { visible: false, ..Default::default() }
    }
}

// ---------------------------------------------------------------------------
// Axes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    Linear,
    /// Logarithmic with the given base; Prism offers 10 and 2.
    Log(f64),
    /// Evenly spaced slots, one per category.
    Categorical,
}

impl Default for Scale {
    fn default() -> Self {
        Scale::Linear
    }
}

/// An explicit axis range, or automatic limits derived from the data.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Range {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl Range {
    pub fn is_auto(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }
}

#[derive(Debug, Clone)]
pub struct CustomTick {
    pub value: f64,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TickDirection {
    #[default]
    Out,
    In,
    Both,
    None,
}

/// How numbers are written along an axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberFormat {
    #[default]
    Decimal,
    Scientific,
    /// `10^3` style, which Prism uses by default on log axes.
    Power10,
    /// Antilog labels on a log axis: the tick at 3 is labelled 1000.
    Antilog,
    Percent,
}

#[derive(Debug, Clone)]
pub struct Axis {
    pub title: Label,
    pub scale: Scale,
    pub range: Range,
    /// Spacing between major ticks. `None` chooses a round interval automatically.
    pub major_interval: Option<f64>,
    /// Minor ticks drawn between each pair of major ticks.
    pub minor_ticks: usize,
    pub tick_direction: TickDirection,
    pub tick_length: f32,
    pub number_format: NumberFormat,
    pub decimals: Option<u32>,
    pub label_style: TextStyle,
    pub label_rotation: f32,
    /// Replaces the generated ticks when non-empty.
    pub custom_ticks: Vec<CustomTick>,
    /// Category names for a categorical axis.
    pub categories: Vec<String>,
    pub visible: bool,
    pub line_width: f32,
    pub color: Color,
    /// Draw a grid line at every major tick.
    pub grid: bool,
    pub grid_color: Color,
    /// Reverse the direction of increasing values.
    pub reversed: bool,
}

impl Default for Axis {
    fn default() -> Self {
        Axis {
            title: Label::default(),
            scale: Scale::Linear,
            range: Range::default(),
            major_interval: None,
            minor_ticks: 0,
            tick_direction: TickDirection::Out,
            tick_length: 5.0,
            number_format: NumberFormat::Decimal,
            decimals: None,
            label_style: TextStyle::default(),
            label_rotation: 0.0,
            custom_ticks: Vec::new(),
            categories: Vec::new(),
            visible: true,
            line_width: 1.0,
            color: Color::BLACK,
            grid: false,
            grid_color: Color::rgb(200, 200, 200),
            reversed: false,
        }
    }
}

/// Where the axes are drawn relative to the plotting area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Frame {
    /// X along the bottom, Y along the left. Prism's default.
    #[default]
    PlainAxes,
    /// A full rectangle around the plot.
    Box,
    /// Axes offset from the data, Prism's "offset" style.
    Offset,
}

// ---------------------------------------------------------------------------
// Marks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SymbolShape {
    None,
    #[default]
    Circle,
    Square,
    Triangle,
    TriangleDown,
    Diamond,
    Plus,
    Cross,
    Star,
    /// A short horizontal bar, used for before-after plots.
    Dash,
    HalfCircleLeft,
    HalfCircleRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineStyle {
    #[default]
    Solid,
    Dash,
    Dot,
    DashDot,
    None,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub shape: SymbolShape,
    /// Diameter in points.
    pub size: f32,
    pub fill: Color,
    pub border: Color,
    pub border_width: f32,
}

impl Symbol {
    pub fn for_index(i: usize) -> Self {
        const SHAPES: [SymbolShape; 6] = [
            SymbolShape::Circle,
            SymbolShape::Square,
            SymbolShape::Triangle,
            SymbolShape::Diamond,
            SymbolShape::TriangleDown,
            SymbolShape::Cross,
        ];
        let c = palette_color(i);
        Symbol {
            shape: SHAPES[i % SHAPES.len()],
            size: 6.0,
            fill: c,
            border: c,
            border_width: 1.0,
        }
    }
}

impl Default for Symbol {
    fn default() -> Self {
        Symbol::for_index(0)
    }
}

#[derive(Debug, Clone)]
pub struct Stroke {
    pub style: LineStyle,
    pub width: f32,
    pub color: Color,
}

impl Stroke {
    pub fn new(color: Color, width: f32) -> Self {
        Stroke { style: LineStyle::Solid, width, color }
    }

    pub fn none() -> Self {
        Stroke { style: LineStyle::None, width: 0.0, color: Color::TRANSPARENT }
    }

    pub fn is_visible(&self) -> bool {
        self.style != LineStyle::None && self.width > 0.0 && self.color.a > 0
    }
}

impl Default for Stroke {
    fn default() -> Self {
        Stroke::new(Color::BLACK, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorBarStyle {
    #[default]
    Both,
    Up,
    Down,
    None,
}

#[derive(Debug, Clone)]
pub struct ErrorBars {
    pub style: ErrorBarStyle,
    pub cap_width: f32,
    pub stroke: Stroke,
}

impl Default for ErrorBars {
    fn default() -> Self {
        ErrorBars { style: ErrorBarStyle::Both, cap_width: 5.0, stroke: Stroke::default() }
    }
}

// ---------------------------------------------------------------------------
// Plots
// ---------------------------------------------------------------------------

/// How a column of replicate values is summarised into a single mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Center {
    #[default]
    Mean,
    Median,
    GeometricMean,
    /// Draw every replicate; no summary mark.
    None,
}

/// What the error bars of a summarised plot span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Spread {
    #[default]
    Sd,
    Sem,
    /// A confidence interval at the stored level.
    Ci,
    /// Full range of the data.
    Range,
    /// Values already stored in the table's error subcolumns.
    FromTable,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarLayout {
    /// Bars of each row sit side by side. Prism's default for grouped tables.
    #[default]
    Interleaved,
    /// Bars accumulate upward within a row.
    Stacked,
    /// Stacked and normalised so each row totals 100%.
    StackedPercent,
    /// Each dataset gets its own slot along the axis.
    Separated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxWhiskerStyle {
    /// Whiskers to 1.5 IQR, outliers drawn individually.
    #[default]
    Tukey,
    MinMax,
    Percentile10_90,
    Percentile5_95,
    MeanSd,
}

/// How the individual points of a column scatter are spread sideways.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Jitter {
    #[default]
    None,
    /// Points nudged apart symmetrically, Prism's "aligned" scatter.
    Aligned,
    Random,
}

/// The family a plot belongs to. Each variant carries only its own settings;
/// per-dataset appearance lives in [`Series`].
#[derive(Debug, Clone, PartialEq)]
pub enum PlotKind {
    /// Points, optionally joined. The XY workhorse.
    XY {
        connect: bool,
        /// Draw a filled band between the line and the baseline.
        area: bool,
        /// Smooth the connecting line through the points.
        spline: bool,
    },
    /// Vertical or horizontal bars.
    Bar { layout: BarLayout, horizontal: bool, center: Center, spread: Spread },
    /// One mark per replicate, grouped by column.
    ScatterDot { jitter: Jitter, center: Center, spread: Spread },
    BoxWhisker { style: BoxWhiskerStyle, horizontal: bool, show_points: bool },
    Violin { show_quartiles: bool, horizontal: bool },
    /// Kaplan-Meier style staircase.
    Survival { show_censor_marks: bool, as_percent: bool },
    /// Pie or donut.
    PartsOfWhole { donut_hole: f32, start_angle: f32, explode: Vec<usize> },
    /// A coloured grid of the table's values.
    Heatmap { colormap: Colormap, show_values: bool },
    /// Scatter whose symbol size and colour encode further variables.
    Bubble { size_variable: Option<Usize2>, color_variable: Option<Usize2>, colormap: Colormap },
    /// Clustering tree.
    Dendrogram { horizontal: bool },
}

/// A `(dataset index, subcolumn)` reference into the source table.
pub type Usize2 = (usize, usize);

impl Default for PlotKind {
    fn default() -> Self {
        PlotKind::XY { connect: false, area: false, spline: false }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Colormap {
    /// One colour per category.
    Categorical(Vec<Color>),
    /// A continuous ramp through the given stops.
    Gradient(Vec<(f64, Color)>),
    Single(Color),
}

impl Default for Colormap {
    fn default() -> Self {
        // Prism's default heatmap ramp: blue through white to red.
        Colormap::Gradient(vec![
            (0.0, Color::rgb(45, 92, 173)),
            (0.5, Color::WHITE),
            (1.0, Color::rgb(220, 48, 42)),
        ])
    }
}

impl Colormap {
    /// Colour for a value already normalised to `0..=1`.
    pub fn sample(&self, t: f64) -> Color {
        match self {
            Colormap::Single(c) => *c,
            Colormap::Categorical(cs) if cs.is_empty() => Color::BLACK,
            Colormap::Categorical(cs) => {
                let i = ((t * cs.len() as f64) as usize).min(cs.len() - 1);
                cs[i]
            }
            Colormap::Gradient(stops) if stops.is_empty() => Color::BLACK,
            Colormap::Gradient(stops) => {
                let t = t.clamp(0.0, 1.0);
                let mut lo = &stops[0];
                let mut hi = stops.last().unwrap();
                for w in stops.windows(2) {
                    if t >= w[0].0 && t <= w[1].0 {
                        lo = &w[0];
                        hi = &w[1];
                        break;
                    }
                }
                let span = hi.0 - lo.0;
                let f = if span.abs() < f64::EPSILON { 0.0 } else { (t - lo.0) / span };
                let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * f).round() as u8;
                Color {
                    r: mix(lo.1.r, hi.1.r),
                    g: mix(lo.1.g, hi.1.g),
                    b: mix(lo.1.b, hi.1.b),
                    a: mix(lo.1.a, hi.1.a),
                }
            }
        }
    }
}

/// One plotted dataset and how it is drawn.
///
/// For every family except [`PlotKind::PartsOfWhole`] a series is one Y dataset.
/// A pie has no datasets to compare — its slices are the *rows* of one column —
/// so there the series are the slices and `dataset` is the row index.
#[derive(Debug, Clone)]
pub struct Series {
    /// Index of the Y dataset in the source table, or of the slice for a pie.
    pub dataset: usize,
    pub label: String,
    pub visible: bool,
    pub symbol: Symbol,
    pub line: Stroke,
    /// Fill for bars, areas, violins and box bodies.
    pub fill: Color,
    pub border: Stroke,
    pub error_bars: ErrorBars,
    /// Plot against the right-hand Y axis instead of the left.
    pub secondary_axis: bool,
}

impl Series {
    pub fn new(dataset: usize, label: impl Into<String>) -> Self {
        let c = palette_color(dataset);
        Series {
            dataset,
            label: label.into(),
            visible: true,
            symbol: Symbol::for_index(dataset),
            line: Stroke::new(c, 1.5),
            fill: c,
            border: Stroke::new(Color::BLACK, 1.0),
            error_bars: ErrorBars::default(),
            secondary_axis: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegendPosition {
    #[default]
    Right,
    Left,
    Above,
    Below,
    /// Placed by the user at an explicit point.
    Floating,
    None,
}

#[derive(Debug, Clone)]
pub struct Legend {
    pub position: LegendPosition,
    pub style: TextStyle,
    pub frame: bool,
    /// Offset from the anchor, in points, used when `position` is `Floating`.
    pub offset: (f32, f32),
}

impl Default for Legend {
    fn default() -> Self {
        Legend {
            position: LegendPosition::Right,
            style: TextStyle::default(),
            frame: false,
            offset: (0.0, 0.0),
        }
    }
}

/// What one slot along a categorical axis stands for.
///
/// A Column table draws one slot per dataset, summarising that dataset's whole
/// column. A Grouped table draws one slot per *row* with one bar per dataset
/// inside it. Getting this wrong collapses a grouped graph into a column graph,
/// so it is recorded explicitly rather than guessed from the category count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlotSource {
    /// One slot per Y dataset.
    #[default]
    Datasets,
    /// One slot per table row.
    Rows,
}

/// Everything needed to draw one graph.
#[derive(Debug, Clone)]
pub struct GraphSpec {
    pub title: Label,
    /// Plotting-area size in points, excluding titles and legend.
    pub size: (f32, f32),
    pub frame: Frame,
    pub background: Color,
    pub plot_background: Color,
    pub x: Axis,
    pub y: Axis,
    /// Right-hand axis, present when any series sets `secondary_axis`.
    pub y2: Option<Axis>,
    pub kind: PlotKind,
    pub series: Vec<Series>,
    pub legend: Legend,
    /// Whether a categorical slot is a dataset or a row.
    pub slots: SlotSource,
    /// Gap between adjacent bars within a group, as a fraction of slot width.
    pub bar_gap: f32,
    /// Gap between groups, as a fraction of slot width.
    pub group_gap: f32,
    /// The data sheet this graph draws from.
    pub source: Option<Uid>,
}

impl Default for GraphSpec {
    fn default() -> Self {
        GraphSpec {
            title: Label::hidden(),
            // Prism's default plotting area is about 4.5 x 3.3 inches.
            size: (324.0, 237.0),
            frame: Frame::PlainAxes,
            background: Color::WHITE,
            plot_background: Color::TRANSPARENT,
            x: Axis::default(),
            y: Axis::default(),
            y2: None,
            kind: PlotKind::default(),
            series: Vec::new(),
            legend: Legend::default(),
            slots: SlotSource::Datasets,
            bar_gap: 0.1,
            group_gap: 0.4,
            source: None,
        }
    }
}

impl GraphSpec {
    /// Build the graph Prism creates by default for `table`.
    ///
    /// Prism picks the family from the table kind: XY tables get points, Column
    /// tables a scatter with a mean bar, Grouped tables interleaved bars, Survival
    /// tables a staircase, and Parts of whole tables a pie.
    pub fn default_for_table(table: &DataTable) -> Self {
        let mut spec = GraphSpec::default();
        let format = table.format;

        spec.kind = match format {
            TableFormat::XY | TableFormat::MultiVariable => {
                PlotKind::XY { connect: false, area: false, spline: false }
            }
            TableFormat::Column | TableFormat::Nested => PlotKind::ScatterDot {
                jitter: Jitter::Aligned,
                center: Center::Mean,
                spread: Spread::Sd,
            },
            TableFormat::Grouped | TableFormat::Contingency => PlotKind::Bar {
                layout: BarLayout::Interleaved,
                horizontal: false,
                center: Center::Mean,
                spread: Spread::FromTable,
            },
            TableFormat::Survival => {
                PlotKind::Survival { show_censor_marks: true, as_percent: true }
            }
            TableFormat::PartsOfWhole => {
                PlotKind::PartsOfWhole { donut_hole: 0.0, start_angle: 90.0, explode: Vec::new() }
            }
        };

        // Grouped and contingency tables index their slots by row; the other
        // column-like kinds index them by dataset.
        let categorical = matches!(
            format,
            TableFormat::Column
                | TableFormat::Grouped
                | TableFormat::Contingency
                | TableFormat::Nested
        );
        spec.slots = if matches!(format, TableFormat::Grouped | TableFormat::Contingency) {
            SlotSource::Rows
        } else {
            SlotSource::Datasets
        };

        let labels: Vec<String> = table
            .ys
            .iter()
            .enumerate()
            .map(|(i, y)| {
                if y.title.trim().is_empty() {
                    format!("Dataset {}", i + 1)
                } else {
                    y.title.clone()
                }
            })
            .collect();

        if format == TableFormat::PartsOfWhole {
            // Slices are named in the legend, not along an axis.
            spec.x.visible = false;
            spec.y.visible = false;
        } else if categorical {
            spec.x.scale = Scale::Categorical;
            spec.x.tick_direction = TickDirection::None;
            spec.x.categories = match spec.slots {
                SlotSource::Rows => {
                    let rows = table.values.last_used_row().map_or(0, |r| r + 1);
                    (0..rows)
                        .map(|r| {
                            table
                                .row_titles
                                .get(r)
                                .filter(|t| !t.trim().is_empty())
                                .cloned()
                                .unwrap_or_else(|| format!("{}", r + 1))
                        })
                        .collect()
                }
                SlotSource::Datasets => labels.clone(),
            };
        }

        if format == TableFormat::Survival {
            spec.y.range = Range { min: Some(0.0), max: Some(100.0) };
            spec.y.title = Label::new("Percent survival");
        }

        if let Some(x) = &table.x {
            if !x.title.trim().is_empty() {
                spec.x.title = Label::new(x.title.clone());
            }
        }

        // Prism names an untitled dataset by its position, so the legend still
        // distinguishes the series in a file that never had column titles.
        //
        // A pie's series are its slices, which come from the rows of one column
        // and are named by the row titles.
        spec.series = if format == TableFormat::PartsOfWhole {
            let base = table.y_offset(0);
            (0..table.values.rows())
                .filter(|r| {
                    table.values.get(*r, base).as_number().is_some_and(|v| v > 0.0)
                })
                .enumerate()
                .map(|(slice, row)| {
                    let label = table
                        .row_titles
                        .get(row)
                        .filter(|t| !t.trim().is_empty())
                        .cloned()
                        .unwrap_or_else(|| format!("Slice {}", slice + 1));
                    Series::new(slice, label)
                })
                .collect()
        } else {
            labels
                .iter()
                .enumerate()
                .map(|(i, t)| Series::new(i, t.clone()))
                .collect()
        };

        // A single dataset needs no legend to tell the series apart.
        if spec.series.len() < 2 {
            spec.legend.position = LegendPosition::None;
        }
        spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_round_trip_through_hex() {
        let c = Color { r: 0x12, g: 0x34, b: 0x56, a: 0x78 };
        assert_eq!(Color::parse(&c.to_hex()), Some(c));
        assert_eq!(Color::parse("#ffffff"), Some(Color::WHITE));
        assert_eq!(Color::parse("#f00"), Some(Color::rgb(255, 0, 0)));
    }

    #[test]
    fn gradient_interpolates_between_stops() {
        let m = Colormap::Gradient(vec![(0.0, Color::BLACK), (1.0, Color::WHITE)]);
        assert_eq!(m.sample(0.0), Color::BLACK);
        assert_eq!(m.sample(1.0), Color::WHITE);
        assert_eq!(m.sample(0.5).r, 128);
    }

    #[test]
    fn column_tables_default_to_a_categorical_scatter_over_datasets() {
        let mut t = DataTable::new(TableFormat::Column);
        t.ys = vec![crate::model::DataSet::new("Control"), crate::model::DataSet::new("Treated")];
        let s = GraphSpec::default_for_table(&t);
        assert!(matches!(s.kind, PlotKind::ScatterDot { .. }));
        assert_eq!(s.x.scale, Scale::Categorical);
        assert_eq!(s.slots, SlotSource::Datasets);
        assert_eq!(s.x.categories, vec!["Control", "Treated"]);
        assert_eq!(s.series.len(), 2);
    }

    #[test]
    fn grouped_tables_put_rows_on_the_axis_and_datasets_in_each_group() {
        let mut t = DataTable::new(TableFormat::Grouped);
        t.ys = vec![crate::model::DataSet::new("Control"), crate::model::DataSet::new("Treated")];
        t.row_titles = vec!["Day 1".into(), "Day 2".into()];
        t.sync_grid_width();
        t.values.resize(2, t.total_width());
        t.values.set(0, 0, crate::value::Cell::Number(1.0));
        t.values.set(1, 1, crate::value::Cell::Number(2.0));
        let s = GraphSpec::default_for_table(&t);
        assert!(matches!(s.kind, PlotKind::Bar { .. }));
        assert_eq!(s.slots, SlotSource::Rows);
        assert_eq!(s.x.categories, vec!["Day 1", "Day 2"]);
        assert_eq!(s.series.len(), 2, "one series per dataset, drawn inside each group");
    }
}
