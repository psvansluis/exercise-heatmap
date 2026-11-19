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
    Backup,
}

/// A normalized activity category, preserving provider values not yet mapped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityType {
    Run,
    Walk,
    Hike,
    Ride,
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
    pub duration_seconds: Option<f64>,
    pub distance_meters: Option<f64>,
    pub bounds: Option<BoundingBox>,
}

/// Ordered GPS track associated with an activity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub activity_id: ActivityId,
    pub points: Vec<TrackPoint>,
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
