use std::path::PathBuf;

/// Everything that can go wrong while reading or writing a Prism document.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("io error: {0}")]
    BareIo(#[from] std::io::Error),

    #[error("{0} is not a Prism document: {1}")]
    NotPrism(PathBuf, &'static str),

    #[error("zip container error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("malformed json in {entry}: {source}")]
    Json {
        entry: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("malformed xml: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("malformed xml attribute: {0}")]
    XmlAttr(#[from] quick_xml::events::attributes::AttrError),

    #[error("malformed csv in {entry}: {source}")]
    Csv {
        entry: String,
        #[source]
        source: csv::Error,
    },

    #[error("document references {kind} {uid}, which is not present in the container")]
    DanglingRef { kind: &'static str, uid: String },

    #[error("unsupported {what}: {value}")]
    Unsupported { what: &'static str, value: String },

    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn invalid(msg: impl Into<String>) -> Self {
        Error::Invalid(msg.into())
    }
}
