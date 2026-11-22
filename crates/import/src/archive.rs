use exercise_domain::{Activity, Track};

#[derive(Clone, Debug)]
pub struct ParseOptions {
    pub max_archive_bytes: usize,
    pub max_entry_bytes: usize,
    pub max_total_uncompressed_bytes: usize,
    pub max_expansion_ratio: u64,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            max_archive_bytes: 0x100 * 0x400 * 0x400,
            max_entry_bytes: 0x40 * 0x400 * 0x400,
            max_total_uncompressed_bytes: 0x100 * 0x400 * 0x400,
            max_expansion_ratio: 200,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImportEntryOutcome {
    Reconciled { activity: Activity, track: Track },
    TrackOnly { activity: Activity, track: Track },
    MetadataOnly { activity: Activity, reason: String },
    Duplicate { path: String },
    Malformed { path: String, reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportWarning {
    pub path: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    pub entries: Vec<ImportEntryOutcome>,
    pub warnings: Vec<ImportWarning>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportError(pub String);

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ImportError {}

pub fn parse_strava_zip(bytes: &[u8]) -> Result<ImportReport, ImportError> {
    parse_zip(bytes, &ParseOptions::default())
}

pub fn parse_strava_zip_with_options(
    bytes: &[u8],
    options: &ParseOptions,
) -> Result<ImportReport, ImportError> {
    parse_zip(bytes, options)
}

pub fn parse_zip(bytes: &[u8], options: &ParseOptions) -> Result<ImportReport, ImportError> {
    let mut stream = crate::zip_stream::ZipImportStream::new(bytes, options.clone())?;
    let mut report = ImportReport::default();
    while let Some(event) = stream.next_event()? {
        match event {
            crate::zip_stream::ImportEvent::Entry(entry) => report.entries.push(entry),
            crate::zip_stream::ImportEvent::Warning(warning) => report.warnings.push(warning),
            crate::zip_stream::ImportEvent::Progress { .. }
            | crate::zip_stream::ImportEvent::Complete => {}
        }
    }
    Ok(report)
}
