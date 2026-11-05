//! Reading and writing `.pzfx` (and the equivalent `.xml`) documents.
//!
//! The format is specified by `PrismXMLSchema.xsd`, which Prism ships inside its
//! own bundle. It carries **data and info tables only**: a document's graphs live
//! in an opaque, zlib-compressed `<Template>` blob whose inflated payload starts
//! with the magic `PCFFGRA4`. We read and write the tables, and carry the template
//! blob through untouched so that saving a file we opened does not drop its graphs.

mod reader;
mod writer;

pub use reader::read_bytes;
pub use reader::read_file;
pub use writer::write_bytes;
pub use writer::write_file;

/// The namespace every element lives in.
pub(crate) const NS: &str = "http://graphpad.com/prism/Prism.htm";

/// Key under which the untouched `<Template>` payload is kept in
/// [`crate::model::Preserved::entries`].
pub const TEMPLATE_KEY: &str = "pzfx:Template";
/// Key for the `<TemplateDescription>` text.
pub const TEMPLATE_DESCRIPTION_KEY: &str = "pzfx:TemplateDescription";
