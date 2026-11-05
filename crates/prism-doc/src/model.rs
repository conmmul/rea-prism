//! The in-memory document model.
//!
//! This mirrors the structure Prism 11 writes into a `.prism` container, which is a
//! ZIP holding `document.json` plus per-sheet JSON and per-table CSV. The same model
//! also backs `.pzfx`, which carries data and info tables only.

use std::collections::BTreeMap;

use crate::value::Grid;

/// A stable identifier. Prism uses upper-case hyphenated UUIDs throughout.
pub type Uid = String;

pub fn new_uid() -> Uid {
    uuid::Uuid::new_v4().to_string().to_uppercase()
}

/// A whole Prism document.
#[derive(Debug, Clone, Default)]
pub struct Document {
    pub meta: Meta,
    pub data_sheets: Vec<DataSheet>,
    pub graph_sheets: Vec<GraphSheet>,
    pub info_sheets: Vec<InfoSheet>,
    pub analyses: Vec<AnalysisSheet>,
    /// Container entries we parse but do not model, kept verbatim so that writing a
    /// document we read does not discard anything Prism put there.
    pub preserved: Preserved,
}

#[derive(Debug, Clone)]
pub struct Meta {
    pub created: Option<String>,
    pub modified: Option<String>,
    pub created_by: Option<Producer>,
    pub modified_by: Option<Producer>,
    /// Container format version, e.g. `1-6-0`. Distinct from the Prism version.
    pub format_version: String,
    pub min_format_version: String,
    pub min_prism_version: String,
}

impl Default for Meta {
    fn default() -> Self {
        // Values Prism 11.0.x writes for a newly created document.
        Meta {
            created: None,
            modified: None,
            created_by: None,
            modified_by: None,
            format_version: "1-6-0".into(),
            min_format_version: "1-4-0".into(),
            min_prism_version: "10.3.0".into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Producer {
    pub name: String,
    pub user: String,
    pub version: String,
    pub platform: String,
}

/// Opaque container entries preserved across a read/write round trip.
#[derive(Debug, Clone, Default)]
pub struct Preserved {
    /// Raw bytes keyed by their path inside the ZIP.
    pub entries: BTreeMap<String, Vec<u8>>,
    /// `document.json` keys we do not model, re-emitted unchanged.
    pub document_extra: serde_json::Map<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Data sheets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DataSheet {
    pub uid: Uid,
    pub title: String,
    pub table: DataTable,
    pub font: Font,
    /// True for sheets Prism generates as analysis output rather than user input.
    pub is_analysis_view: bool,
}

impl DataSheet {
    pub fn new(title: impl Into<String>, format: TableFormat) -> Self {
        DataSheet {
            uid: new_uid(),
            title: title.into(),
            table: DataTable::new(format),
            font: Font::default(),
            is_analysis_view: false,
        }
    }
}

/// The shape of a data table. These are Prism's eight table kinds; the first six
/// are stored in the `format` field and the last two additionally set `ExtTableType`
/// in the XML format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableFormat {
    #[default]
    XY,
    /// One grouping per column, no X. Prism calls this a Column table.
    Column,
    /// Rows and columns both group. Prism calls this Grouped (two-way).
    Grouped,
    Contingency,
    Survival,
    PartsOfWhole,
    /// Each column is a variable; rows are observations.
    MultiVariable,
    Nested,
}

impl TableFormat {
    /// The token used in `.prism` sheet JSON.
    pub fn as_json(self) -> &'static str {
        match self {
            TableFormat::XY => "xy",
            TableFormat::Column => "column",
            TableFormat::Grouped => "twoway",
            TableFormat::Contingency => "contingency",
            TableFormat::Survival => "survival",
            TableFormat::PartsOfWhole => "partsofwhole",
            TableFormat::MultiVariable => "multivariable",
            TableFormat::Nested => "nested",
        }
    }

    pub fn from_json(s: &str) -> Option<Self> {
        Some(match s {
            "xy" => TableFormat::XY,
            "column" => TableFormat::Column,
            "twoway" | "grouped" => TableFormat::Grouped,
            "contingency" => TableFormat::Contingency,
            "survival" => TableFormat::Survival,
            "partsofwhole" => TableFormat::PartsOfWhole,
            "multivariable" => TableFormat::MultiVariable,
            "nested" => TableFormat::Nested,
            _ => return None,
        })
    }

    /// Whether this table kind has an X column.
    pub fn has_x(self) -> bool {
        matches!(self, TableFormat::XY | TableFormat::Survival)
    }
}

/// How the subcolumns of one Y dataset are interpreted.
///
/// This is the `YFormat` of the XML schema and the `dataFormat` of the JSON sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubcolumnFormat {
    /// A single value per row.
    #[default]
    Single,
    /// `replicates` side-by-side measurements per row.
    Replicates,
    /// Mean, SD, N.
    MeanSdN,
    /// Mean, SEM, N.
    MeanSemN,
    /// Mean, CV, N.
    MeanCvN,
    /// Mean and SD.
    MeanSd,
    /// Mean and SEM.
    MeanSem,
    /// Mean and CV.
    MeanCv,
    /// Low and high bounds of a range.
    LowHigh,
    /// Upper and lower confidence limits around a mean.
    UpperLowerLimits,
}

impl SubcolumnFormat {
    /// How many physical subcolumns one Y dataset occupies.
    pub fn subcolumn_count(self, replicates: usize) -> usize {
        match self {
            SubcolumnFormat::Single => 1,
            SubcolumnFormat::Replicates => replicates.max(1),
            SubcolumnFormat::MeanSdN | SubcolumnFormat::MeanSemN | SubcolumnFormat::MeanCvN => 3,
            SubcolumnFormat::MeanSd
            | SubcolumnFormat::MeanSem
            | SubcolumnFormat::MeanCv
            | SubcolumnFormat::LowHigh => 2,
            SubcolumnFormat::UpperLowerLimits => 3,
        }
    }

    /// Whether the second and third subcolumns describe spread around the first.
    pub fn has_error(self) -> bool {
        !matches!(self, SubcolumnFormat::Single | SubcolumnFormat::Replicates)
    }

    pub fn as_json(self) -> &'static str {
        match self {
            SubcolumnFormat::Single => "y_single",
            SubcolumnFormat::Replicates => "y_replicates",
            SubcolumnFormat::MeanSdN => "y_sd_n",
            SubcolumnFormat::MeanSemN => "y_sem_n",
            SubcolumnFormat::MeanCvN => "y_cv_n",
            SubcolumnFormat::MeanSd => "y_sd",
            SubcolumnFormat::MeanSem => "y_sem",
            SubcolumnFormat::MeanCv => "y_cv",
            SubcolumnFormat::LowHigh => "y_low_high",
            SubcolumnFormat::UpperLowerLimits => "y_upper_lower_limits",
        }
    }

    pub fn from_json(s: &str) -> Option<Self> {
        Some(match s {
            "y_single" => SubcolumnFormat::Single,
            "y_replicates" => SubcolumnFormat::Replicates,
            "y_sd_n" => SubcolumnFormat::MeanSdN,
            "y_sem_n" => SubcolumnFormat::MeanSemN,
            "y_cv_n" => SubcolumnFormat::MeanCvN,
            "y_sd" => SubcolumnFormat::MeanSd,
            "y_sem" => SubcolumnFormat::MeanSem,
            "y_cv" => SubcolumnFormat::MeanCv,
            "y_low_high" => SubcolumnFormat::LowHigh,
            "y_upper_lower_limits" => SubcolumnFormat::UpperLowerLimits,
            _ => return None,
        })
    }

    /// The token used by the `YFormat` attribute of the XML schema.
    pub fn as_xml(self) -> &'static str {
        match self {
            SubcolumnFormat::Single | SubcolumnFormat::Replicates => "replicates",
            SubcolumnFormat::MeanSdN => "SDN",
            SubcolumnFormat::MeanSemN => "SEN",
            SubcolumnFormat::MeanCvN => "CVN",
            SubcolumnFormat::MeanSd => "SD",
            SubcolumnFormat::MeanSem => "SE",
            SubcolumnFormat::MeanCv => "CV",
            SubcolumnFormat::LowHigh => "low-high",
            SubcolumnFormat::UpperLowerLimits => "upper-lower-limits",
        }
    }

    pub fn from_xml(s: &str, replicates: usize) -> Self {
        match s {
            "SD" => SubcolumnFormat::MeanSd,
            "SE" => SubcolumnFormat::MeanSem,
            "CV" => SubcolumnFormat::MeanCv,
            "SDN" => SubcolumnFormat::MeanSdN,
            "SEN" => SubcolumnFormat::MeanSemN,
            "CVN" => SubcolumnFormat::MeanCvN,
            "low-high" => SubcolumnFormat::LowHigh,
            "upper-lower-limits" => SubcolumnFormat::UpperLowerLimits,
            // "replicates" and "text" both store plain values; the replicate count
            // decides whether we treat the dataset as single or replicated.
            _ if replicates > 1 => SubcolumnFormat::Replicates,
            _ => SubcolumnFormat::Single,
        }
    }
}

/// How the X column is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XFormat {
    None,
    #[default]
    Numbers,
    Text,
    /// X with error values.
    Error,
    /// Evenly spaced values generated from `start_at` and `interval`.
    Series,
    Date,
    StartEndDate,
    Time,
}

impl XFormat {
    pub fn as_xml(self) -> &'static str {
        match self {
            XFormat::None => "none",
            XFormat::Numbers => "numbers",
            XFormat::Text => "text",
            XFormat::Error => "error",
            XFormat::Series => "series",
            XFormat::Date => "date",
            XFormat::StartEndDate => "startenddate",
            XFormat::Time => "time",
        }
    }

    pub fn from_xml(s: &str) -> Self {
        match s {
            "none" => XFormat::None,
            "text" => XFormat::Text,
            "error" => XFormat::Error,
            "series" => XFormat::Series,
            "date" => XFormat::Date,
            "startenddate" => XFormat::StartEndDate,
            "time" => XFormat::Time,
            _ => XFormat::Numbers,
        }
    }

    pub fn subcolumn_count(self) -> usize {
        match self {
            XFormat::None => 0,
            XFormat::Error | XFormat::StartEndDate => 2,
            _ => 1,
        }
    }
}

/// One table of values plus the column metadata describing it.
#[derive(Debug, Clone)]
pub struct DataTable {
    pub uid: Uid,
    pub format: TableFormat,
    pub data_format: SubcolumnFormat,
    pub x_format: XFormat,
    pub replicates: usize,
    /// First value of a generated X series, used when `x_format` is `Series`.
    pub series_start: Option<f64>,
    /// Step between generated X values, used when `x_format` is `Series`.
    pub series_interval: Option<f64>,
    /// Physical subcolumns the X column occupies. Date and time tables store two:
    /// an elapsed number in `XColumn` plus the formatted text in `XAdvancedColumn`.
    pub x_subcolumns: usize,
    /// True when the source wrote a separate `XAdvancedColumn` alongside `XColumn`.
    pub x_advanced: bool,
    /// Present when `format.has_x()`.
    pub x: Option<DataSet>,
    pub ys: Vec<DataSet>,
    pub row_titles: Vec<String>,
    /// Cell values, one physical subcolumn per grid column: the X subcolumns first,
    /// then each Y dataset's subcolumns in order.
    pub values: Grid,
}

impl DataTable {
    pub fn new(format: TableFormat) -> Self {
        DataTable {
            uid: new_uid(),
            format,
            data_format: SubcolumnFormat::Single,
            x_format: if format.has_x() { XFormat::Numbers } else { XFormat::None },
            replicates: 1,
            series_start: None,
            series_interval: None,
            x_subcolumns: if format.has_x() { 1 } else { 0 },
            x_advanced: false,
            x: format.has_x().then(|| DataSet::new("X")),
            ys: Vec::new(),
            row_titles: Vec::new(),
            values: Grid::default(),
        }
    }

    /// Physical subcolumns occupied by the X column.
    pub fn x_width(&self) -> usize {
        if self.format.has_x() {
            self.x_subcolumns
        } else {
            0
        }
    }

    /// Physical subcolumns occupied by each Y dataset.
    pub fn y_width(&self) -> usize {
        self.data_format.subcolumn_count(self.replicates)
    }

    /// Total physical subcolumns, which is the CSV field count.
    pub fn total_width(&self) -> usize {
        self.x_width() + self.ys.len() * self.y_width()
    }

    /// The grid column where dataset `index` begins.
    pub fn y_offset(&self, index: usize) -> usize {
        self.x_width() + index * self.y_width()
    }

    /// Rebuild the grid so its width matches the current column layout.
    pub fn sync_grid_width(&mut self) {
        let w = self.total_width();
        if self.values.cols() != w {
            let rows = self.values.rows();
            self.values.resize(rows, w);
        }
    }

    /// The X value of a row, falling back to the one-based row index for tables
    /// that have no X column (Prism plots those against position).
    pub fn x_at(&self, row: usize) -> Option<f64> {
        if self.format.has_x() {
            match self.x_format {
                XFormat::Series => {
                    let start = self.series_start.unwrap_or(0.0);
                    let step = self.series_interval.unwrap_or(1.0);
                    Some(start + step * row as f64)
                }
                _ => self.values.get(row, 0).as_number(),
            }
        } else {
            Some(row as f64 + 1.0)
        }
    }
}

/// One column of a table: its title and the metadata Prism keeps about it.
#[derive(Debug, Clone)]
pub struct DataSet {
    pub uid: Uid,
    pub title: String,
    /// `continuous`, `categorical`, `label`, ... as stored in `currentType`.
    pub kind: DataSetKind,
    pub decimals: Option<u32>,
    pub scientific: bool,
}

impl DataSet {
    pub fn new(title: impl Into<String>) -> Self {
        DataSet {
            uid: new_uid(),
            title: title.into(),
            kind: DataSetKind::Continuous,
            decimals: None,
            scientific: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataSetKind {
    #[default]
    Continuous,
    Categorical,
    Label,
}

impl DataSetKind {
    pub fn as_json(self) -> &'static str {
        match self {
            DataSetKind::Continuous => "continuous",
            DataSetKind::Categorical => "categorical",
            DataSetKind::Label => "label",
        }
    }

    pub fn from_json(s: &str) -> Self {
        match s {
            "categorical" => DataSetKind::Categorical,
            "label" => DataSetKind::Label,
            _ => DataSetKind::Continuous,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Font {
    pub family: String,
    pub size: f32,
}

impl Default for Font {
    fn default() -> Self {
        // Prism 11's default data-sheet font.
        Font { family: "Arial".into(), size: 10.0 }
    }
}

// ---------------------------------------------------------------------------
// Other sheet kinds
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InfoSheet {
    pub uid: Uid,
    pub title: String,
    pub notes: String,
    pub constants: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct AnalysisSheet {
    pub uid: Uid,
    pub title: String,
    /// e.g. `HIERARCHICAL_CLUSTERING`. We do not run analyses; the sheet is carried
    /// so that documents round-trip and so results tables stay linked to their graphs.
    pub analysis_class: String,
    pub parameters: serde_json::Value,
    pub results: Option<serde_json::Value>,
}

/// A graph sheet.
///
/// Prism 11 writes two different graph representations. Newer multivariable graphs
/// (`GraphSheet`/`MVGraph`) carry their full appearance as JSON. Classic graphs
/// (`FENGraphSheet`) keep only their identity in JSON and store appearance in an
/// undocumented `data.bin` blob; for those we record the dataset links we can read
/// and keep the blob so nothing is lost on write.
#[derive(Debug, Clone)]
pub struct GraphSheet {
    pub uid: Uid,
    pub title: String,
    pub kind: GraphKind,
    /// Datasets feeding the graph, in plot order.
    pub input_data_sets: Vec<Uid>,
    /// The data sheet the graph draws from, when the JSON names one.
    pub data_sheet: Option<Uid>,
    /// Parsed appearance. Defaults are applied when the source is an opaque blob.
    pub spec: crate::graph::GraphSpec,
    /// The original `data.bin`, preserved verbatim for classic graphs.
    pub legacy_blob: Option<Vec<u8>>,
    /// The original graph JSON, preserved so unmodelled settings survive a write.
    pub raw_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphKind {
    /// `FENGraphSheet`: appearance lives in the opaque `data.bin`.
    Classic,
    /// `GraphSheet` with an `MVGraph`: appearance is fully described in JSON.
    MultiVariable,
}
