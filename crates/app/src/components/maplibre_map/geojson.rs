use exercise_domain::{ActivityEntry, ActivityType};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct GeoJsonFeatureCollection {
    r#type: &'static str,
    pub features: Vec<GeoJsonFeature>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct GeoJsonFeature {
    r#type: &'static str,
    pub geometry: GeoJsonLineString,
    pub properties: GeoJsonProperties,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct GeoJsonLineString {
    r#type: &'static str,
    pub coordinates: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeoJsonProperties {
    pub activity_id: String,
    pub activity_type: String,
}

fn activity_type_label(t: &ActivityType) -> String {
    match t {
        ActivityType::Run => "Run".into(),
        ActivityType::Walk => "Walk".into(),
        ActivityType::Hike => "Hike".into(),
        ActivityType::Ride => "Ride".into(),
        ActivityType::Swim => "Swim".into(),
        ActivityType::Other(s) => s.clone(),
    }
}

/// Converts entries with `include_on_map == true` and a valid track into a
/// GeoJSON `FeatureCollection` suitable for MapLibre's `geojson` source type.
pub(crate) fn entries_to_geojson(entries: &[ActivityEntry]) -> GeoJsonFeatureCollection {
    let features = entries
        .iter()
        .filter_map(|entry| {
            let track = entry.track.as_ref()?;
            let coordinates: Vec<[f64; 2]> = track
                .points
                .iter()
                .filter(|p| p.has_valid_coordinates())
                .map(|p| [p.longitude, p.latitude])
                .collect();

            if coordinates.len() < 2 {
                return None;
            }

            Some(GeoJsonFeature {
                r#type: "Feature",
                geometry: GeoJsonLineString {
                    r#type: "LineString",
                    coordinates,
                },
                properties: GeoJsonProperties {
                    activity_id: entry.activity.id.0.clone(),
                    activity_type: activity_type_label(&entry.activity.activity_type),
                },
            })
        })
        .collect();

    GeoJsonFeatureCollection {
        r#type: "FeatureCollection",
        features,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exercise_domain::*;

    fn sample_activity(id: &str, activity_type: ActivityType) -> Activity {
        Activity {
            id: ActivityId(id.into()),
            source: ActivitySource::ImportedFile,
            source_id: None,
            name: Some("Test".into()),
            activity_type,
            started_at: None,
            elapsed_duration_seconds: Some(1200.0),
            timer_duration_seconds: Some(1100.0),
            distance_meters: Some(5000.0),
            bounds: None,
        }
    }

    fn sample_track(id: &str, points: Vec<TrackPoint>) -> Track {
        Track {
            activity_id: ActivityId(id.into()),
            points,
        }
    }

    fn point(lat: f64, lon: f64) -> TrackPoint {
        TrackPoint {
            latitude: lat,
            longitude: lon,
            timestamp: None,
        }
    }

    #[test]
    fn empty_input_produces_empty_collection() {
        let result = entries_to_geojson(&[]);
        assert_eq!(result.features.len(), 0);
        assert_eq!(result.r#type, "FeatureCollection");
    }

    #[test]
    fn entries_without_track_are_skipped() {
        let entry = ActivityEntry::new(sample_activity("1", ActivityType::Run), None);
        let result = entries_to_geojson(&[entry]);
        assert_eq!(result.features.len(), 0);
    }

    #[test]
    fn coordinates_are_lon_lat_order() {
        let track = sample_track(
            "1",
            vec![point(52.37, 4.89), point(52.38, 4.90)],
        );
        let entry = ActivityEntry::new(sample_activity("1", ActivityType::Run), Some(track));
        let result = entries_to_geojson(&[entry]);

        assert_eq!(result.features.len(), 1);
        let coords = &result.features[0].geometry.coordinates;
        assert_eq!(coords[0], [4.89, 52.37]); // [lon, lat]
        assert_eq!(coords[1], [4.90, 52.38]);
    }

    #[test]
    fn invalid_points_are_filtered() {
        let track = sample_track(
            "1",
            vec![
                point(52.37, 4.89),
                point(f64::NAN, 4.90),
                point(52.39, 4.91),
            ],
        );
        let entry = ActivityEntry::new(sample_activity("1", ActivityType::Run), Some(track));
        let result = entries_to_geojson(&[entry]);

        assert_eq!(result.features.len(), 1);
        assert_eq!(result.features[0].geometry.coordinates.len(), 2);
    }

    #[test]
    fn track_with_fewer_than_two_valid_points_is_skipped() {
        let track = sample_track("1", vec![point(52.37, 4.89)]);
        let entry = ActivityEntry::new(sample_activity("1", ActivityType::Run), Some(track));
        let result = entries_to_geojson(&[entry]);
        assert_eq!(result.features.len(), 0);
    }

    #[test]
    fn activity_type_strings() {
        assert_eq!(activity_type_label(&ActivityType::Run), "Run");
        assert_eq!(activity_type_label(&ActivityType::Ride), "Ride");
        assert_eq!(activity_type_label(&ActivityType::Walk), "Walk");
        assert_eq!(activity_type_label(&ActivityType::Hike), "Hike");
        assert_eq!(activity_type_label(&ActivityType::Swim), "Swim");
        assert_eq!(
            activity_type_label(&ActivityType::Other("Yoga".into())),
            "Yoga"
        );
    }

    #[test]
    fn properties_contain_correct_activity_id_and_type() {
        let track = sample_track(
            "run-42",
            vec![point(52.37, 4.89), point(52.38, 4.90)],
        );
        let entry = ActivityEntry::new(
            sample_activity("run-42", ActivityType::Ride),
            Some(track),
        );
        let result = entries_to_geojson(&[entry]);

        let props = &result.features[0].properties;
        assert_eq!(props.activity_id, "run-42");
        assert_eq!(props.activity_type, "Ride");
    }
}
