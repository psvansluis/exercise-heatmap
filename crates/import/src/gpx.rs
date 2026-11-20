use crate::metadata::map_activity_type;
use chrono::{DateTime, Utc};
use exercise_domain::{ActivityType, TrackPoint};
use gpx::Gpx;
use std::io::Cursor;

#[derive(Debug)]
pub(crate) struct ParsedTrack {
    pub points: Vec<TrackPoint>,
    pub activity_type: Option<ActivityType>,
    pub started_at: Option<DateTime<Utc>>,
    pub elapsed_duration_seconds: Option<f64>,
}

pub(crate) fn parse(bytes: &[u8]) -> Result<ParsedTrack, String> {
    let document = gpx::read(Cursor::new(bytes)).map_err(|error| error.to_string())?;

    let activity_type = parse_activity_type(&document);
    let points = parse_points(&document);

    let timestamps: Vec<_> = points.iter().filter_map(|point| point.timestamp).collect();
    let elapsed_duration_seconds = timestamps
        .first()
        .zip(timestamps.last())
        .map(|(first, last)| (*last - *first).num_milliseconds() as f64 / 1000.0)
        .filter(|duration| *duration >= 0.0);

    Ok(ParsedTrack {
        started_at: timestamps.first().copied(),
        elapsed_duration_seconds,
        activity_type,
        points,
    })
}

fn parse_activity_type(document: &Gpx) -> Option<ActivityType> {
    document
        .routes
        .iter()
        .filter_map(|route| route.type_.as_deref())
        .chain(
            document
                .tracks
                .iter()
                .filter_map(|track| track.type_.as_deref()),
        )
        .find_map(map_activity_type)
}

fn parse_points(document: &Gpx) -> Vec<TrackPoint> {
    document
        .routes
        .iter()
        .flat_map(|route| route.points.iter())
        .chain(
            document
                .tracks
                .iter()
                .flat_map(|track| track.segments.iter())
                .flat_map(|segment| segment.points.iter()),
        )
        .filter_map(|waypoint| {
            let point = waypoint.point();
            let track_point = TrackPoint {
                latitude: point.y(),
                longitude: point.x(),
                timestamp: waypoint
                    .time
                    .and_then(|time| time.format().ok())
                    .and_then(|time| DateTime::parse_from_rfc3339(&time).ok())
                    .map(|time| time.with_timezone(&Utc)),
            };
            track_point.has_valid_coordinates().then_some(track_point)
        })
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::parse;
    use exercise_domain::ActivityType;

    #[test]
    fn parses_track_type_points_timestamps_and_elapsed_time() {
        let gpx = br#"<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
            <trk><type>Walk</type><trkseg>
                <trkpt lat="51.5" lon="-0.1"><time>2024-08-20T10:00:00Z</time></trkpt>
                <trkpt lat="51.6" lon="-0.2"><time>2024-08-20T10:01:15Z</time></trkpt>
            </trkseg></trk>
        </gpx>"#;

        let parsed = parse(gpx).unwrap();
        assert_eq!(parsed.points.len(), 2);
        assert_eq!(parsed.activity_type, Some(ActivityType::Walk));
        assert_eq!(
            parsed.started_at.unwrap().to_rfc3339(),
            "2024-08-20T10:00:00+00:00"
        );
        assert_eq!(parsed.elapsed_duration_seconds, Some(75.0));
    }

    #[test]
    fn includes_route_points_but_not_standalone_waypoints() {
        let gpx = br#"<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
            <wpt lat="50" lon="-1" />
            <rte><type>Run</type>
                <rtept lat="51.5" lon="-0.1"><time>2024-08-20T10:00:00Z</time></rtept>
                <rtept lat="51.6" lon="-0.2"><time>2024-08-20T10:00:30Z</time></rtept>
            </rte>
        </gpx>"#;

        let parsed = parse(gpx).unwrap();
        assert_eq!(parsed.points.len(), 2);
        assert_eq!(parsed.activity_type, Some(ActivityType::Run));
        assert_eq!(parsed.elapsed_duration_seconds, Some(30.0));
    }
}
