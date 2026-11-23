//! Provider-independent types shared by import, storage, and analysis.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Application-owned identity for an activity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActivityId(pub String);

/// Origin of a canonical activity record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivitySource {
    StravaArchive,
    ImportedFile,
    Backup,
}

/// A normalized activity category, preserving provider values not yet mapped.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActivityType {
    Run,
    Walk,
    Hike,
    Ride,
    Swim,
    Other(String),
}

/// Geographic bounds in latitude/longitude degrees.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min_latitude: f64,
    pub min_longitude: f64,
    pub max_latitude: f64,
    pub max_longitude: f64,
}

/// Canonical activity metadata. Distances are metres and durations are seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    pub id: ActivityId,
    pub source: ActivitySource,
    pub source_id: Option<String>,
    pub name: Option<String>,
    pub activity_type: ActivityType,
    pub started_at: Option<DateTime<Utc>>,
    /// Wall-clock time including pauses, when available.
    pub elapsed_duration_seconds: Option<f64>,
    /// Timer-active time excluding pauses, when available.
    pub timer_duration_seconds: Option<f64>,
    pub distance_meters: Option<f64>,
    pub bounds: Option<BoundingBox>,
}

/// Ordered GPS track associated with an activity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub activity_id: ActivityId,
    pub points: Vec<TrackPoint>,
}

/// An activity record together with its optional GPS track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub activity: Activity,
    pub track: Option<Track>,
    pub include_on_map: bool,
}

impl ActivityEntry {
    pub fn new(activity: Activity, track: Option<Track>) -> Self {
        let has_track = track.is_some();
        let has_duration = activity.elapsed_duration_seconds.is_some()
            || activity.timer_duration_seconds.is_some();
        let include_on_map = has_track && has_duration;

        Self {
            activity,
            track,
            include_on_map,
        }
    }

    /// Whether this entry has a GPS track and can be included on a map.
    pub fn can_include_on_map(&self) -> bool {
        self.track.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackPoint {
    pub latitude: f64,
    pub longitude: f64,
    pub timestamp: Option<DateTime<Utc>>,
}

impl TrackPoint {
    /// Whether coordinates are finite and within the legal geographic ranges.
    pub fn has_valid_coordinates(&self) -> bool {
        self.latitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && self.longitude.is_finite()
            && (-180.0..=180.0).contains(&self.longitude)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_activity(elapsed: Option<f64>, timer: Option<f64>) -> Activity {
        Activity {
            id: ActivityId("test-1".into()),
            source: ActivitySource::ImportedFile,
            source_id: None,
            name: Some("Test Run".into()),
            activity_type: ActivityType::Run,
            started_at: None,
            elapsed_duration_seconds: elapsed,
            timer_duration_seconds: timer,
            distance_meters: Some(5000.0),
            bounds: None,
        }
    }

    fn sample_track() -> Track {
        Track {
            activity_id: ActivityId("test-1".into()),
            points: vec![TrackPoint {
                latitude: 52.37,
                longitude: 4.89,
                timestamp: None,
            }],
        }
    }

    #[test]
    fn activity_without_track_defaults_to_off_and_cannot_be_included() {
        let activity = sample_activity(Some(1200.0), Some(1100.0));
        let entry = ActivityEntry::new(activity, None);

        assert!(!entry.include_on_map);
        assert!(!entry.can_include_on_map());
    }

    #[test]
    fn activity_with_track_and_durations_defaults_to_on() {
        let activity_with_both = sample_activity(Some(1200.0), Some(1100.0));
        let entry_both = ActivityEntry::new(activity_with_both, Some(sample_track()));
        assert!(entry_both.include_on_map);
        assert!(entry_both.can_include_on_map());

        let activity_with_elapsed = sample_activity(Some(1200.0), None);
        let entry_elapsed = ActivityEntry::new(activity_with_elapsed, Some(sample_track()));
        assert!(entry_elapsed.include_on_map);

        let activity_with_timer = sample_activity(None, Some(1100.0));
        let entry_timer = ActivityEntry::new(activity_with_timer, Some(sample_track()));
        assert!(entry_timer.include_on_map);
    }

    #[test]
    fn activity_with_track_lacking_both_durations_defaults_to_off() {
        let activity = sample_activity(None, None);
        let entry = ActivityEntry::new(activity, Some(sample_track()));

        assert!(!entry.include_on_map);
        assert!(entry.can_include_on_map());
    }
}
