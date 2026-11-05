//! Reading and writing the `.prism` / `.prismt` container.
//!
//! The container is a ZIP with this layout, recovered by inspecting the files
//! shipped inside Prism 11 and documents written by Prism 10 and 11:
//!
//! ```text
//! document.json                      manifest: sheet order, titles, producers
//! data/sheets/<uid>/sheet.json       one data sheet: table shape + dataset links
//! data/sets/<uid>.json               one column: title, type, subcolumn layout
//! data/tables/<uid>/content.json     row and column counts
//! data/tables/<uid>/data.csv         the cell values, one field per subcolumn
//! data/tables/<uid>/data.dt          32 hex characters, purpose unconfirmed
//! graphs/<uid>/sheet.json            graph identity; appearance for MV graphs
//! graphs/<uid>/data.bin              classic graph appearance, "PCFFGRA4" blob
//! analyses/<uid>/{sheet,parameters,results}.json
//! info/<uid>/...                     info sheets
//! misc/*.bin                         fonts and other side-car blobs
//! ```
//!
//! Anything this module does not model is carried through untouched, so writing a
//! document we read preserves the parts Prism understands and we do not.

mod reader;
mod writer;

pub use reader::read_bytes;
pub use reader::read_file;
pub use writer::write_bytes;
pub use writer::write_file;

/// Entry paths we parse into the model rather than preserving verbatim.
pub(crate) fn is_modelled(path: &str) -> bool {
    path == "document.json"
        || path.starts_with("data/sheets/")
        || path.starts_with("data/sets/")
        || path.starts_with("data/tables/")
}
