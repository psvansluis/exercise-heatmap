use crate::{
    archive::{ImportEntryOutcome, ParseOptions},
    fit, gpx,
    metadata::{ActivityMetadata, map_activity_type},
};
use chrono::{DateTime, Utc};
use exercise_domain::{
    Activity, ActivityId, ActivitySource, ActivityType, BoundingBox, Track, TrackPoint,
};
use flate2::read::GzDecoder;
use std::io::Read;

pub fn parse_activity_file(path: &str, bytes: &[u8], options: &ParseOptions) -> ImportEntryOutcome {
    if bytes.len() > options.max_entry_bytes {
        return ImportEntryOutcome::Malformed {
            path: path.into(),
            reason: "entry exceeds size limit".into(),
        };
    }
    let parsed_bytes = if path.to_lowercase().ends_with(".fit.gz") {
        match decompress_gzip(bytes, options.max_entry_bytes) {
            Ok(bytes) => bytes,
            Err(reason) => {
                return ImportEntryOutcome::Malformed {
                    path: path.into(),
                    reason,
                };
            }
        }
    } else {
        bytes.to_vec()
    };
    if parsed_bytes.len() > options.max_total_uncompressed_bytes {
        return ImportEntryOutcome::Malformed {
            path: path.into(),
            reason: "decompressed activity exceeds the configured total size limit".into(),
        };
    }
    match parse_activity_bytes(path, &parsed_bytes) {
        Ok(parsed) => finish_activity(None, path, &parsed_bytes, parsed),
        Err(reason) => ImportEntryOutcome::Malformed {
            path: path.into(),
            reason,
        },
    }
}

pub(super) fn parse_activity_bytes(path: &str, bytes: &[u8]) -> Result<ParsedFile, String> {
    let normalized = path.to_lowercase();
    if normalized.ends_with(".gpx") {
        let parsed = gpx::parse(bytes)?;
        Ok(ParsedFile {
            points: parsed.points,
            activity_type: parsed.activity_type,
            started_at: parsed.started_at,
            elapsed_duration_seconds: parsed.elapsed_duration_seconds,
            timer_duration_seconds: None,
        })
    } else if normalized.ends_with(".fit") || normalized.ends_with(".fit.gz") {
        let parsed = fit::parse(bytes)?;
        Ok(ParsedFile {
            points: parsed.points,
            activity_type: parsed.activity_type,
            started_at: parsed.started_at,
            elapsed_duration_seconds: parsed.elapsed_duration_seconds,
            timer_duration_seconds: parsed.timer_duration_seconds,
        })
    } else {
        Err("unsupported activity file format".into())
    }
}

pub(super) struct ParsedFile {
    pub(super) points: Vec<TrackPoint>,
    pub(super) activity_type: Option<ActivityType>,
    pub(super) started_at: Option<DateTime<Utc>>,
    pub(super) elapsed_duration_seconds: Option<f64>,
    pub(super) timer_duration_seconds: Option<f64>,
}

pub(super) fn finish_activity(
    metadata: Option<ActivityMetadata>,
    path: &str,
    identity_bytes: &[u8],
    parsed: ParsedFile,
) -> ImportEntryOutcome {
    let reconciled = metadata.is_some();
    let file_type = parsed.activity_type.clone();
    let activity_type = file_type
        .or_else(|| {
            metadata
                .as_ref()
                .and_then(|meta| map_activity_type(&meta.raw_type))
        })
        .or_else(|| metadata.as_ref().map(|meta| meta.activity_type.clone()))
        .unwrap_or_else(|| ActivityType::Other("Unknown".into()));
    let id = metadata
        .as_ref()
        .map(|meta| meta.id.clone())
        .unwrap_or_else(|| ActivityId(format!("file:{:016x}", stable_hash(path, identity_bytes))));
    let bounds = bounds_for(&parsed.points);
    let activity = Activity {
        id: id.clone(),
        source: if metadata.is_some() {
            ActivitySource::StravaArchive
        } else {
            ActivitySource::ImportedFile
        },
        source_id: metadata.as_ref().map(|meta| meta.source_id.clone()),
        name: metadata
            .as_ref()
            .and_then(|meta| meta.name.clone())
            .or_else(|| Some(file_name(path))),
        activity_type,
        started_at: parsed
            .started_at
            .or_else(|| metadata.as_ref().and_then(|meta| meta.started_at)),
        elapsed_duration_seconds: parsed.elapsed_duration_seconds.or_else(|| {
            metadata
                .as_ref()
                .and_then(|meta| meta.elapsed_duration_seconds)
        }),
        timer_duration_seconds: parsed.timer_duration_seconds.or_else(|| {
            metadata
                .as_ref()
                .and_then(|meta| meta.timer_duration_seconds)
        }),
        distance_meters: metadata.as_ref().and_then(|meta| meta.distance_meters),
        bounds,
    };
    if parsed.points.is_empty() {
        ImportEntryOutcome::MetadataOnly {
            activity,
            reason: "activity file contains no usable GPS points".into(),
        }
    } else {
        let track = Track {
            activity_id: id,
            points: parsed.points,
        };
        if reconciled {
            ImportEntryOutcome::Reconciled { activity, track }
        } else {
            ImportEntryOutcome::TrackOnly { activity, track }
        }
    }
}

pub(super) fn failed_file(
    metadata: Option<ActivityMetadata>,
    path: &str,
    reason: String,
) -> ImportEntryOutcome {
    if let Some(meta) = metadata {
        let activity_type =
            map_activity_type(&meta.raw_type).unwrap_or_else(|| meta.activity_type.clone());
        ImportEntryOutcome::MetadataOnly {
            activity: Activity {
                id: meta.id,
                source: ActivitySource::StravaArchive,
                source_id: Some(meta.source_id),
                name: meta.name.or_else(|| Some(file_name(path))),
                activity_type,
                started_at: meta.started_at,
                elapsed_duration_seconds: meta.elapsed_duration_seconds,
                timer_duration_seconds: meta.timer_duration_seconds,
                distance_meters: meta.distance_meters,
                bounds: None,
            },
            reason,
        }
    } else {
        ImportEntryOutcome::Malformed {
            path: path.into(),
            reason,
        }
    }
}

pub(super) fn metadata_only(meta: ActivityMetadata, reason: String) -> ImportEntryOutcome {
    ImportEntryOutcome::MetadataOnly {
        activity: Activity {
            id: meta.id,
            source: ActivitySource::StravaArchive,
            source_id: Some(meta.source_id),
            name: meta
                .name
                .or_else(|| meta.filename.as_deref().map(file_name)),
            activity_type: map_activity_type(&meta.raw_type).unwrap_or(meta.activity_type),
            started_at: meta.started_at,
            elapsed_duration_seconds: meta.elapsed_duration_seconds,
            timer_duration_seconds: meta.timer_duration_seconds,
            distance_meters: meta.distance_meters,
            bounds: None,
        },
        reason,
    }
}

pub(super) fn decompress_gzip(bytes: &[u8], limit: usize) -> Result<Vec<u8>, String> {
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

fn bounds_for(points: &[TrackPoint]) -> Option<BoundingBox> {
    let first = points.first()?;
    let (mut min_lat, mut max_lat) = (first.latitude, first.latitude);
    let (mut min_lon, mut max_lon) = (first.longitude, first.longitude);
    for point in &points[1..] {
        min_lat = min_lat.min(point.latitude);
        max_lat = max_lat.max(point.latitude);
        min_lon = min_lon.min(point.longitude);
        max_lon = max_lon.max(point.longitude);
    }
    Some(BoundingBox {
        min_latitude: min_lat,
        min_longitude: min_lon,
        max_latitude: max_lat,
        max_longitude: max_lon,
    })
}

fn stable_hash(path: &str, bytes: &[u8]) -> u64 {
    path.bytes()
        .chain(bytes.iter().copied())
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}

fn file_name(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let lowercase = name.to_lowercase();
    let extension_length = [".fit.gz", ".gpx", ".fit"]
        .iter()
        .find(|extension| lowercase.ends_with(**extension))
        .map_or(0, |extension| extension.len());
    name[..name.len().saturating_sub(extension_length)].to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_activity_file;
    use crate::archive::{ImportEntryOutcome, ParseOptions};
    use exercise_domain::{ActivitySource, ActivityType};

    const GPX: &[u8] = br#"<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1"><trk><type>Walk</type><trkseg><trkpt lat="51.5" lon="-0.1"><time>2024-08-20T10:00:00Z</time></trkpt><trkpt lat="51.6" lon="-0.2"><time>2024-08-20T10:01:00Z</time></trkpt></trkseg></trk></gpx>"#;

    #[test]
    fn standalone_gpx_import_does_not_require_csv_metadata() {
        let outcome = parse_activity_file("walk.gpx", GPX, &ParseOptions::default());
        assert!(matches!(
            outcome,
            ImportEntryOutcome::TrackOnly { activity, .. }
                if activity.name.as_deref() == Some("walk")
                    && activity.source == ActivitySource::ImportedFile
                    && activity.activity_type == ActivityType::Walk
        ));
    }
}
