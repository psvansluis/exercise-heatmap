//! Import adapters that normalize external activity archives into domain types.
//!
//! Archive and format dependencies will be selected after the ZIP, FIT, and
//! `.fit.gz` compatibility spike described in `DESIGN.md`.

mod archive;
mod fit;
mod gpx;
mod metadata;

pub use archive::{ImportEntryOutcome, ImportWarning};
