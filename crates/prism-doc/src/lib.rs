//! A reader, writer and document model for GraphPad Prism files.
//!
//! Two container formats are supported:
//!
//! * **`.prism` / `.prismt`** — the native format of Prism 10 and 11. A ZIP holding
//!   `document.json`, per-sheet JSON and per-table CSV. Data round-trips in full.
//! * **`.pzfx` / `.xml`** — the XML format specified by the `PrismXMLSchema.xsd`
//!   that ships inside Prism. It carries data and info tables only; graphs live in
//!   an opaque `<Template>` blob which is preserved but not interpreted.
//!
//! ```no_run
//! let doc = prism_doc::open("experiment.prism")?;
//! for sheet in &doc.data_sheets {
//!     println!("{}: {} x {}", sheet.title, sheet.table.values.rows(), sheet.table.ys.len());
//! }
//! prism_doc::save(&doc, "copy.prism")?;
//! # Ok::<(), prism_doc::Error>(())
//! ```

pub mod csvgrid;
pub mod error;
pub mod graph;
pub mod model;
pub mod prism;
pub mod pzfx;
pub mod value;

pub use error::{Error, Result};
pub use graph::GraphSpec;
pub use model::{DataSet, DataSheet, DataTable, Document, GraphSheet, TableFormat};
pub use value::{Cell, Grid};

use std::path::Path;

/// Which container a path or byte slice holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// ZIP-based `.prism` / `.prismt`.
    Prism,
    /// XML-based `.pzfx` / `.xml`.
    Pzfx,
}

impl Format {
    /// Detect from the leading bytes: a ZIP local-file header means `.prism`.
    pub fn sniff(bytes: &[u8]) -> Option<Format> {
        if bytes.starts_with(b"PK\x03\x04") {
            Some(Format::Prism)
        } else if bytes.trim_ascii_start().starts_with(b"<") {
            Some(Format::Pzfx)
        } else {
            None
        }
    }

    pub fn from_extension(path: &Path) -> Option<Format> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "prism" | "prismt" | "pzt" => Some(Format::Prism),
            "pzfx" | "xml" => Some(Format::Pzfx),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Prism => "prism",
            Format::Pzfx => "pzfx",
        }
    }
}

/// Open a Prism document, choosing the reader from the file's contents.
pub fn open(path: impl AsRef<Path>) -> Result<Document> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| Error::Io { path: path.into(), source })?;
    match Format::sniff(&bytes).or_else(|| Format::from_extension(path)) {
        Some(Format::Prism) => prism::read_bytes(&bytes),
        Some(Format::Pzfx) => pzfx::read_bytes(&bytes),
        None => Err(Error::NotPrism(path.into(), "not a ZIP container or XML document")),
    }
}

/// Write a document, choosing the writer from the path's extension.
pub fn save(doc: &Document, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    match Format::from_extension(path).unwrap_or(Format::Prism) {
        Format::Prism => prism::write_file(doc, path),
        Format::Pzfx => pzfx::write_file(doc, path),
    }
}

/// Write a document in an explicitly chosen format.
pub fn save_as(doc: &Document, path: impl AsRef<Path>, format: Format) -> Result<()> {
    match format {
        Format::Prism => prism::write_file(doc, path),
        Format::Pzfx => pzfx::write_file(doc, path),
    }
}
