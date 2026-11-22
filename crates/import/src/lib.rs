//! Import adapters that normalize activity files and ZIP archives into domain types.

mod archive;
mod batch;
mod fit;
mod gpx;
mod metadata;
mod zip_stream;

pub use archive::{
    ImportEntryOutcome, ImportError, ImportReport, ImportWarning, ParseOptions, parse_strava_zip,
    parse_strava_zip_with_options, parse_zip,
};
pub use batch::parse_activity_file;
pub use zip_stream::{ImportEvent, ZipImportStream};
