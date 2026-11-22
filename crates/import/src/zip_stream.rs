use crate::{
    archive::{ImportEntryOutcome, ImportError, ImportWarning, ParseOptions},
    batch::{decompress_gzip, failed_file, metadata_only, parse_activity_bytes},
    metadata::{self, ActivityMetadata},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::{Cursor, Read},
};
use zip::{ZipArchive, read::ZipFile};

#[derive(Clone, Debug, PartialEq)]
pub enum ImportEvent {
    Entry(ImportEntryOutcome),
    Warning(ImportWarning),
    Progress { completed: usize, total: usize },
    Complete,
}

#[derive(Clone)]
struct Candidate {
    index: usize,
    path: String,
    size: u64,
    compressed_size: u64,
}

/// Incrementally parses supported files from one ZIP. Each call to `next_event`
/// processes at most one activity file, allowing callers to yield between files.
pub struct ZipImportStream<'a> {
    archive: ZipArchive<Cursor<&'a [u8]>>,
    options: ParseOptions,
    candidates: Vec<Candidate>,
    archive_paths: HashSet<String>,
    cursor: usize,
    metadata_by_path: HashMap<String, VecDeque<ActivityMetadata>>,
    unmatched_metadata: VecDeque<ActivityMetadata>,
    pending_events: VecDeque<ImportEvent>,
    seen_source_ids: HashSet<String>,
    completed: usize,
    total_uncompressed: usize,
    finished: bool,
}

impl<'a> ZipImportStream<'a> {
    pub fn new(bytes: &'a [u8], options: ParseOptions) -> Result<Self, ImportError> {
        if bytes.len() > options.max_archive_bytes {
            return Err(ImportError(
                "archive exceeds the configured size limit".into(),
            ));
        }
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| ImportError("invalid or unsupported ZIP archive".into()))?;
        let mut candidates = Vec::new();
        let mut all_paths = HashSet::new();
        let mut csv_indices = Vec::new();

        for index in 0..archive.len() {
            let file = archive
                .by_index(index)
                .map_err(|_| ImportError("could not read ZIP directory".into()))?;
            if file.is_dir() {
                continue;
            }
            let path = file.name().to_string();
            validate_path(&path).map_err(ImportError)?;
            if !all_paths.insert(path.clone()) {
                return Err(ImportError("ZIP contains duplicate entry paths".into()));
            }
            if path.rsplit('/').next() == Some("activities.csv") {
                csv_indices.push(index);
            }
            if is_activity_file(&path) {
                candidates.push(Candidate {
                    index,
                    path,
                    size: file.size(),
                    compressed_size: file.compressed_size(),
                });
            }
        }

        let mut stream = Self {
            archive,
            options,
            candidates,
            archive_paths: all_paths,
            cursor: 0,
            metadata_by_path: HashMap::new(),
            unmatched_metadata: VecDeque::new(),
            pending_events: VecDeque::new(),
            seen_source_ids: HashSet::new(),
            completed: 0,
            total_uncompressed: 0,
            finished: false,
        };

        if csv_indices.len() > 1 {
            stream
                .pending_events
                .push_back(ImportEvent::Warning(ImportWarning {
                    path: None,
                    message: "ZIP contains multiple activities.csv files; no CSV reconciliation was performed".into(),
                }));
        } else if let Some(index) = csv_indices.first() {
            stream.read_metadata(*index)?;
        }
        Ok(stream)
    }

    pub fn next_event(&mut self) -> Result<Option<ImportEvent>, ImportError> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(Some(event));
        }
        if self.cursor < self.candidates.len() {
            let candidate = self.candidates[self.cursor].clone();
            self.cursor += 1;
            self.completed += 1;
            let outcome = self.process_candidate(candidate)?;
            self.pending_events.push_back(ImportEvent::Progress {
                completed: self.completed,
                total: self.candidates.len(),
            });
            return Ok(Some(ImportEvent::Entry(outcome)));
        }
        if let Some(meta) = self.pop_unmatched_metadata() {
            let path = meta.filename.clone();
            let reason = match path.as_deref() {
                None => "activity CSV row has no activity file path".to_string(),
                Some(path) if self.archive_paths.contains(path) => {
                    "CSV path exists in the ZIP but is not a supported activity file".to_string()
                }
                Some(_) => {
                    "activity file path from CSV does not exactly match a ZIP entry".to_string()
                }
            };
            self.pending_events
                .push_back(ImportEvent::Warning(ImportWarning {
                    path,
                    message: reason.clone(),
                }));
            return Ok(Some(ImportEvent::Entry(metadata_only(meta, reason))));
        }
        if !self.finished {
            self.finished = true;
            return Ok(Some(ImportEvent::Complete));
        }
        Ok(None)
    }

    fn read_metadata(&mut self, index: usize) -> Result<(), ImportError> {
        let file = self
            .archive
            .by_index(index)
            .map_err(|_| ImportError("could not open activities.csv".into()))?;
        validate_entry_size(
            file.size(),
            file.compressed_size(),
            &self.options,
            "activities.csv",
        )?;
        let bytes = read_zip_file(file, self.options.max_entry_bytes, "activities.csv")?;
        self.total_uncompressed = self.total_uncompressed.saturating_add(bytes.len());
        if self.total_uncompressed > self.options.max_total_uncompressed_bytes {
            return Err(ImportError(
                "archive entries exceed the total expanded size limit".into(),
            ));
        }
        let rows = match metadata::parse_csv(&bytes) {
            Ok(rows) => rows,
            Err(message) => {
                self.pending_events
                    .push_back(ImportEvent::Warning(ImportWarning {
                        path: Some("activities.csv".into()),
                        message: format!("activity CSV could not be used: {message}"),
                    }));
                return Ok(());
            }
        };
        for row in rows {
            match row {
                Ok(meta) => {
                    if let Some(path) = &meta.filename {
                        self.metadata_by_path
                            .entry(path.clone())
                            .or_default()
                            .push_back(meta);
                    } else {
                        self.unmatched_metadata.push_back(meta);
                    }
                }
                Err(error) => self.pending_events.push_back(ImportEvent::Entry(
                    ImportEntryOutcome::Malformed {
                        path: format!("activities.csv#row-{}", error.row_number),
                        reason: error.message,
                    },
                )),
            }
        }
        Ok(())
    }

    fn process_candidate(
        &mut self,
        candidate: Candidate,
    ) -> Result<ImportEntryOutcome, ImportError> {
        let metadata = self
            .metadata_by_path
            .get_mut(&candidate.path)
            .and_then(VecDeque::pop_front);
        let source_id = metadata.as_ref().map(|meta| meta.source_id.clone());
        if let Some(source_id) = source_id {
            if !self.seen_source_ids.insert(source_id) {
                return Ok(ImportEntryOutcome::Duplicate {
                    path: candidate.path,
                });
            }
        }

        if let Err(reason) = validate_entry_size(
            candidate.size,
            candidate.compressed_size,
            &self.options,
            &candidate.path,
        ) {
            return Ok(failed_file(metadata, &candidate.path, reason.0));
        }
        self.total_uncompressed = self
            .total_uncompressed
            .saturating_add(candidate.size as usize);
        if self.total_uncompressed > self.options.max_total_uncompressed_bytes {
            return Ok(failed_file(
                metadata,
                &candidate.path,
                "selected activity files exceed the total expanded size limit".into(),
            ));
        }
        let file = self
            .archive
            .by_index(candidate.index)
            .map_err(|_| ImportError(format!("could not open ZIP entry: {}", candidate.path)))?;
        let bytes = match read_zip_file(file, self.options.max_entry_bytes, &candidate.path) {
            Ok(bytes) => bytes,
            Err(error) => return Ok(failed_file(metadata, &candidate.path, error.0)),
        };
        let parsed_bytes = if candidate.path.to_lowercase().ends_with(".fit.gz") {
            let remaining = self
                .options
                .max_total_uncompressed_bytes
                .saturating_sub(self.total_uncompressed);
            match decompress_gzip(&bytes, self.options.max_entry_bytes.min(remaining)) {
                Ok(bytes) => bytes,
                Err(reason) => return Ok(failed_file(metadata, &candidate.path, reason)),
            }
        } else {
            bytes
        };
        if candidate.path.to_lowercase().ends_with(".fit.gz") {
            self.total_uncompressed = self.total_uncompressed.saturating_add(parsed_bytes.len());
        }
        if self.total_uncompressed > self.options.max_total_uncompressed_bytes {
            return Ok(failed_file(
                metadata,
                &candidate.path,
                "selected activity files exceed the total expanded size limit".into(),
            ));
        }
        match parse_activity_bytes(&candidate.path, &parsed_bytes) {
            Ok(parsed) => Ok(crate::batch::finish_activity(
                metadata,
                &candidate.path,
                &parsed_bytes,
                parsed,
            )),
            Err(reason) => Ok(failed_file(metadata, &candidate.path, reason)),
        }
    }

    fn pop_unmatched_metadata(&mut self) -> Option<ActivityMetadata> {
        if let Some(metadata) = self.unmatched_metadata.pop_front() {
            return Some(metadata);
        }
        let mut paths = self.metadata_by_path.keys().cloned().collect::<Vec<_>>();
        paths.sort();
        paths.into_iter().find_map(|path| {
            self.metadata_by_path
                .get_mut(&path)
                .and_then(VecDeque::pop_front)
        })
    }
}

fn is_activity_file(path: &str) -> bool {
    let path = path.to_lowercase();
    path.ends_with(".gpx") || path.ends_with(".fit") || path.ends_with(".fit.gz")
}

fn validate_entry_size(
    size: u64,
    compressed_size: u64,
    options: &ParseOptions,
    path: &str,
) -> Result<(), ImportError> {
    if size > options.max_entry_bytes as u64 {
        return Err(ImportError(format!("entry exceeds size limit: {path}")));
    }
    if size > 1024 * 1024
        && (compressed_size == 0 || size / compressed_size > options.max_expansion_ratio)
    {
        return Err(ImportError(format!(
            "entry expansion ratio is too high: {path}"
        )));
    }
    Ok(())
}

fn read_zip_file<R: Read>(
    mut file: ZipFile<'_, R>,
    limit: usize,
    path: &str,
) -> Result<Vec<u8>, ImportError> {
    let mut bytes = Vec::with_capacity(file.size().min(limit as u64) as usize);
    file.by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ImportError(format!("could not decompress ZIP entry: {path}")))?;
    if bytes.len() > limit {
        return Err(ImportError(format!("ZIP entry exceeds size limit: {path}")));
    }
    Ok(bytes)
}

fn validate_path(path: &str) -> Result<(), String> {
    if path.starts_with('/') || path.starts_with('\\') || path.contains(':') {
        return Err("ZIP contains an absolute or drive-qualified path".into());
    }
    for component in path.replace('\\', "/").split('/') {
        if component == ".." || component.is_empty() {
            return Err("ZIP contains an unsafe path".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ImportEvent, ZipImportStream};
    use crate::archive::{ImportEntryOutcome, ParseOptions, parse_zip};
    use exercise_domain::{ActivitySource, ActivityType};
    use std::io::{Cursor, Write};
    use zip::{ZipWriter, write::SimpleFileOptions};

    const GPX: &[u8] = br#"<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1"><trk><type>Walk</type><trkseg><trkpt lat="51.5" lon="-0.1"><time>2024-08-20T10:00:00Z</time></trkpt><trkpt lat="51.6" lon="-0.2"><time>2024-08-20T10:01:00Z</time></trkpt></trkseg></trk></gpx>"#;

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (path, contents) in entries {
            writer
                .start_file(*path, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn reconciles_only_exact_paths_and_imports_unlisted_nested_files() {
        let csv = concat!(
            "Activity ID,Activity Type,Activity Date,Activity Name,Elapsed Time,Distance,Filename\n",
            "1,Run,2024-08-19 09:00:00,Matched title,999,1500,nested/one.gpx\n",
            "2,Run,2024-08-19 09:00:00,Wrong case path,999,1500,Nested/two.gpx\n",
        );
        let zip = archive(&[
            ("metadata/activities.csv", csv.as_bytes()),
            ("nested/one.gpx", GPX),
            ("nested/two.gpx", GPX),
        ]);

        let report = parse_zip(&zip, &ParseOptions::default()).unwrap();
        assert_eq!(report.entries.len(), 3);
        assert!(matches!(
            &report.entries[0],
            ImportEntryOutcome::Reconciled { activity, .. }
                if activity.name.as_deref() == Some("Matched title")
                    && activity.activity_type == ActivityType::Walk
        ));
        assert!(matches!(
            &report.entries[1],
            ImportEntryOutcome::TrackOnly { activity, .. }
                if activity.name.as_deref() == Some("two")
                    && activity.source == ActivitySource::ImportedFile
        ));
        assert!(matches!(
            &report.entries[2],
            ImportEntryOutcome::MetadataOnly { activity, reason }
                if activity.name.as_deref() == Some("Wrong case path")
                    && reason.contains("does not exactly match")
        ));
    }

    #[test]
    fn zip_stream_yields_an_entry_before_it_finishes() {
        let zip = archive(&[("deep/path/one.gpx", GPX), ("deep/path/two.gpx", GPX)]);
        let mut stream = ZipImportStream::new(&zip, ParseOptions::default()).unwrap();
        assert!(matches!(
            stream.next_event().unwrap(),
            Some(ImportEvent::Entry(ImportEntryOutcome::TrackOnly { .. }))
        ));
        assert!(matches!(
            stream.next_event().unwrap(),
            Some(ImportEvent::Progress {
                completed: 1,
                total: 2
            })
        ));
    }

    #[test]
    fn malformed_csv_rows_do_not_prevent_activity_file_import() {
        let csv = b"Activity ID,Activity Type,Filename\n42,Run\n";
        let zip = archive(&[("activities.csv", csv), ("elsewhere/walk.gpx", GPX)]);
        let report = parse_zip(&zip, &ParseOptions::default()).unwrap();
        assert_eq!(report.entries.len(), 2);
        assert!(matches!(
            &report.entries[0],
            ImportEntryOutcome::Malformed { path, .. }
                if path == "activities.csv#row-2"
        ));
        assert!(matches!(
            &report.entries[1],
            ImportEntryOutcome::TrackOnly { activity, .. }
                if activity.activity_type == ActivityType::Walk
        ));
    }

    #[test]
    fn rejects_non_zip_input() {
        assert!(ZipImportStream::new(b"not a zip", ParseOptions::default()).is_err());
    }
}
