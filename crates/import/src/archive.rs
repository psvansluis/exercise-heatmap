use exercise_domain::{Activity, ActivitySource, ActivityType, BoundingBox, Track};
use flate2::read::GzDecoder;
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read},
};
use zip::ZipArchive;

use crate::{
    fit, gpx,
    metadata::{self, ActivityMetadata, map_activity_type},
};

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
    Imported {
        activity: Activity,
        track: Track,
    },
    MetadataOnly {
        activity: Activity,
        reason: String,
    },
    /// An activity intentionally excluded by an import policy or user choice.
    Skipped {
        path: String,
        reason: String,
    },
    Duplicate {
        path: String,
    },
    Malformed {
        path: String,
        reason: String,
    },
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

#[derive(Clone)]
struct ArchiveEntry {
    path: String,
    size: u64,
    compressed_size: u64,
}

struct ArchiveIndex {
    entries: HashMap<String, ArchiveEntry>,
    csv_path: String,
}

struct ImportContext<'a, 'zip> {
    archive: &'a mut ZipArchive<Cursor<&'zip [u8]>>,
    index: &'a ArchiveIndex,
    options: &'a ParseOptions,
    seen_source_ids: HashSet<String>,
    referenced_files: HashSet<String>,
    charged_files: HashSet<String>,
    relevant_total: u64,
}

/// Parse a Strava export ZIP from memory. The input is never written to disk.
pub fn parse_strava_zip(bytes: &[u8]) -> Result<ImportReport, ImportError> {
    parse_strava_zip_with_options(bytes, &ParseOptions::default())
}

pub fn parse_strava_zip_with_options(
    bytes: &[u8],
    options: &ParseOptions,
) -> Result<ImportReport, ImportError> {
    if bytes.len() > options.max_archive_bytes {
        return Err(ImportError(
            "archive exceeds the configured size limit".into(),
        ));
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| ImportError("invalid or unsupported ZIP archive".into()))?;
    let index = scan_archive(&mut archive)?;
    let activities = read_activity_metadata(&mut archive, &index, options)?;
    let mut context: ImportContext<'_, '_> = ImportContext {
        archive: &mut archive,
        index: &index,
        options,
        seen_source_ids: HashSet::new(),
        referenced_files: HashSet::new(),
        charged_files: HashSet::new(),
        relevant_total: 0,
    };
    let mut report = ImportReport::default();
    for row in activities {
        context.process_row(row, &mut report)?;
    }
    context.report_unreferenced_activity_files(&mut report);
    Ok(report)
}

fn scan_archive(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Result<ArchiveIndex, ImportError> {
    let mut entries = HashMap::new();
    let mut csv_path = None;
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|_| ImportError("could not read ZIP directory".into()))?;
        let name = file.name().to_string();
        if file.is_dir() {
            continue;
        }
        validate_path(&name).map_err(ImportError)?;
        let size = file.size();
        let compressed = file.compressed_size();
        let key = normalize_path(&name);
        if entries
            .insert(
                key.clone(),
                ArchiveEntry {
                    path: name.clone(),
                    size,
                    compressed_size: compressed,
                },
            )
            .is_some()
        {
            return Err(ImportError("ZIP contains duplicate entry paths".into()));
        }
        if key == "activities.csv" {
            if csv_path.replace(name).is_some() {
                return Err(ImportError(
                    "ZIP contains multiple activities.csv files".into(),
                ));
            }
        }
    }
    let csv_path = csv_path.ok_or_else(|| ImportError("ZIP has no root activities.csv".into()))?;
    Ok(ArchiveIndex { entries, csv_path })
}

fn read_activity_metadata(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    index: &ArchiveIndex,
    options: &ParseOptions,
) -> Result<Vec<Result<ActivityMetadata, metadata::MetadataRowError>>, ImportError> {
    let Some(csv_entry) = index.entries.get("activities.csv") else {
        return Err(ImportError("activity CSV entry metadata is missing".into()));
    };
    if csv_entry.size > options.max_entry_bytes as u64 {
        return Err(ImportError(
            "activities.csv exceeds the configured size limit".into(),
        ));
    }
    if csv_entry.size > 1024 * 1024
        && (csv_entry.compressed_size == 0
            || csv_entry.size / csv_entry.compressed_size > options.max_expansion_ratio)
    {
        return Err(ImportError(
            "activities.csv has an excessive expansion ratio".into(),
        ));
    }
    let csv_bytes = read_entry(archive, &index.csv_path, options.max_entry_bytes)?;
    metadata::parse_csv(&csv_bytes).map_err(ImportError)
}

impl ImportContext<'_, '_> {
    fn process_row(
        &mut self,
        row: Result<ActivityMetadata, metadata::MetadataRowError>,
        report: &mut ImportReport,
    ) -> Result<(), ImportError> {
        let meta = match row {
            Ok(meta) => meta,
            Err(error) => {
                report.entries.push(ImportEntryOutcome::Malformed {
                    path: format!("activities.csv#row-{}", error.row_number),
                    reason: error.message,
                });
                return Ok(());
            }
        };
        if let Some(filename) = &meta.filename {
            self.referenced_files.insert(normalize_path(filename));
        }
        if !self.seen_source_ids.insert(meta.source_id.clone()) {
            report.entries.push(ImportEntryOutcome::Duplicate {
                path: meta.filename.unwrap_or_default(),
            });
            return Ok(());
        }
        self.process_activity(meta, report)
    }

    fn process_activity(
        &mut self,
        meta: ActivityMetadata,
        report: &mut ImportReport,
    ) -> Result<(), ImportError> {
        let Some(filename) = meta.filename.as_deref() else {
            report.warnings.push(ImportWarning {
                path: None,
                message: format!("Activity {} has no matching track filename", meta.source_id),
            });
            self.emit_metadata_only(
                meta,
                None,
                "activity row has no track filename".into(),
                report,
            );
            return Ok(());
        };
        let path = normalize_path(filename);
        let Some(entry) = self.index.entries.get(&path).cloned() else {
            report.warnings.push(ImportWarning {
                path: meta.filename.clone(),
                message: "Activity track file is missing from the archive".into(),
            });
            self.emit_metadata_only(
                meta,
                Some(path),
                "referenced track file is missing from the archive".into(),
                report,
            );
            return Ok(());
        };
        if !path.ends_with(".gpx") && !path.ends_with(".fit") && !path.ends_with(".fit.gz") {
            self.emit_metadata_only(
                meta,
                Some(entry.path),
                format!("unsupported track format: .{}", ext_from_path(&path)),
                report,
            );
            return Ok(());
        }
        self.process_track(meta, path, entry, report)
    }

    fn process_track(
        &mut self,
        meta: ActivityMetadata,
        path: String,
        entry: ArchiveEntry,
        report: &mut ImportReport,
    ) -> Result<(), ImportError> {
        if let Err(reason) = self.validate_track_entry(&entry) {
            self.emit_metadata_only(meta, Some(entry.path), reason, report);
            return Ok(());
        }
        if self.charged_files.insert(path.clone()) {
            self.relevant_total = self.relevant_total.saturating_add(entry.size);
        }
        if self.relevant_total > self.options.max_total_uncompressed_bytes as u64 {
            return Err(ImportError(
                "selected activity files exceed the total expanded size limit".into(),
            ));
        }
        match self.parse_track(&path, &entry.path) {
            Ok((parsed, expanded_size)) => {
                self.relevant_total = self.relevant_total.saturating_add(expanded_size as u64);
                if self.relevant_total > self.options.max_total_uncompressed_bytes as u64 {
                    return Err(ImportError(
                        "selected activity files exceed the total expanded size limit".into(),
                    ));
                }
                self.finish_activity(meta, entry.path, parsed, report);
            }
            Err(reason) => self.emit_metadata_only(meta, Some(entry.path), reason, report),
        }
        Ok(())
    }

    fn validate_track_entry(&self, entry: &ArchiveEntry) -> Result<(), String> {
        if entry.size > self.options.max_entry_bytes as u64 {
            return Err("entry exceeds size limit".into());
        }
        if entry.size > 1024 * 1024
            && (entry.compressed_size == 0
                || entry.size / entry.compressed_size > self.options.max_expansion_ratio)
        {
            return Err("entry expansion ratio is too high".into());
        }
        Ok(())
    }

    fn parse_track(
        &mut self,
        path: &str,
        archive_path: &str,
    ) -> Result<(ParsedFile, usize), String> {
        let bytes = read_entry(self.archive, archive_path, self.options.max_entry_bytes)
            .map_err(|e| e.0)?;
        if path.ends_with(".gpx") {
            let track = gpx::parse(&bytes)?;
            return Ok((
                ParsedFile {
                    points: track.points,
                    activity_type: track.activity_type,
                    started_at: track.started_at,
                },
                0,
            ));
        }
        let fit_bytes = if path.ends_with(".fit.gz") {
            let remaining = self
                .options
                .max_total_uncompressed_bytes
                .saturating_sub(self.relevant_total as usize);
            decompress_gzip(&bytes, self.options.max_entry_bytes.min(remaining))?
        } else {
            bytes
        };
        let expanded_size = if path.ends_with(".fit.gz") {
            fit_bytes.len()
        } else {
            0
        };
        let track = fit::parse(&fit_bytes)?;
        Ok((
            ParsedFile {
                points: track.points,
                activity_type: track.activity_type,
                started_at: track.started_at,
            },
            expanded_size,
        ))
    }

    fn finish_activity(
        &self,
        meta: ActivityMetadata,
        path: String,
        parsed: ParsedFile,
        report: &mut ImportReport,
    ) {
        let activity_type = parsed
            .activity_type
            .or_else(|| map_activity_type(&meta.raw_type))
            .unwrap_or_else(|| meta.activity_type.clone());
        if parsed.points.is_empty() {
            report.warnings.push(ImportWarning {
                path: Some(path.clone()),
                message: "Activity metadata was imported without a usable GPS track".into(),
            });
            let mut activity = to_activity(&meta, Some(activity_type), None);
            activity.started_at = parsed.started_at.or(activity.started_at);
            report.entries.push(ImportEntryOutcome::MetadataOnly {
                activity,
                reason: "track file contains no usable GPS points".into(),
            });
            return;
        }
        let bounds = bounds_for(&parsed.points);
        let mut activity = to_activity(&meta, Some(activity_type), bounds);
        activity.started_at = parsed.started_at.or(activity.started_at);
        let track = Track {
            activity_id: activity.id.clone(),
            points: parsed.points,
        };
        report
            .entries
            .push(ImportEntryOutcome::Imported { activity, track });
    }

    fn emit_metadata_only(
        &self,
        meta: ActivityMetadata,
        path: Option<String>,
        reason: String,
        report: &mut ImportReport,
    ) {
        report.warnings.push(ImportWarning {
            path,
            message: reason.clone(),
        });
        report.entries.push(ImportEntryOutcome::MetadataOnly {
            activity: to_activity(&meta, None, None),
            reason,
        });
    }

    fn report_unreferenced_activity_files(&self, report: &mut ImportReport) {
        for (normalized_path, entry) in &self.index.entries {
            let is_activity_track = normalized_path.starts_with("activities/")
                && (normalized_path.ends_with(".gpx")
                    || normalized_path.ends_with(".fit")
                    || normalized_path.ends_with(".fit.gz"));
            if is_activity_track && !self.referenced_files.contains(normalized_path) {
                report.entries.push(ImportEntryOutcome::Skipped {
                    path: entry.path.clone(),
                    reason: "no activity metadata row references this track file".into(),
                });
            }
        }
    }
}

struct ParsedFile {
    points: Vec<exercise_domain::TrackPoint>,
    activity_type: Option<ActivityType>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn ext_from_path(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or("")
}

fn to_activity(
    meta: &ActivityMetadata,
    file_type: Option<ActivityType>,
    bounds: Option<BoundingBox>,
) -> Activity {
    Activity {
        id: meta.id.clone(),
        source: ActivitySource::StravaArchive,
        source_id: Some(meta.source_id.clone()),
        name: meta.name.clone(),
        activity_type: file_type.unwrap_or_else(|| meta.activity_type.clone()),
        started_at: meta.started_at,
        duration_seconds: meta.duration_seconds,
        distance_meters: meta.distance_meters,
        bounds,
    }
}

fn bounds_for(points: &[exercise_domain::TrackPoint]) -> Option<BoundingBox> {
    let first = points.first()?;
    let (mut min_lat, mut max_lat) = (first.latitude, first.latitude);
    let (mut min_lon, mut max_lon) = (first.longitude, first.longitude);
    for p in &points[1..] {
        min_lat = min_lat.min(p.latitude);
        max_lat = max_lat.max(p.latitude);
        min_lon = min_lon.min(p.longitude);
        max_lon = max_lon.max(p.longitude);
    }
    Some(BoundingBox {
        min_latitude: min_lat,
        min_longitude: min_lon,
        max_latitude: max_lat,
        max_longitude: max_lon,
    })
}

fn read_entry(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
    limit: usize,
) -> Result<Vec<u8>, ImportError> {
    let file = archive
        .by_name(name)
        .map_err(|_| ImportError(format!("could not open ZIP entry: {name}")))?;
    let mut bytes = Vec::with_capacity(file.size().min(limit as u64) as usize);
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ImportError(format!("could not decompress ZIP entry: {name}")))?;
    if bytes.len() > limit {
        return Err(ImportError(format!("ZIP entry exceeds size limit: {name}")));
    }
    Ok(bytes)
}

fn decompress_gzip(bytes: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let mut decoder = GzDecoder::new(bytes);
    let mut out = Vec::new();
    decoder
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|_| "invalid gzip stream".to_string())?;
    if out.len() > limit {
        return Err("decompressed FIT exceeds size limit".into());
    }
    Ok(out)
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .to_lowercase()
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
