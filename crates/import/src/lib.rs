//! Import adapters that normalize Strava archives into canonical domain types.

mod archive;
mod fit;
mod gpx;
mod metadata;

pub use archive::{
    ImportEntryOutcome, ImportError, ImportReport, ImportWarning, ParseOptions, parse_strava_zip,
    parse_strava_zip_with_options,
};
