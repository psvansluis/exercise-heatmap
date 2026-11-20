use chrono::{DateTime, Utc};
use exercise_domain::{ActivityType, TrackPoint};
use fitparser::{FitDataRecord, Value, from_bytes, profile::MesgNum};

pub(crate) struct ParsedTrack {
    pub points: Vec<TrackPoint>,
    pub activity_type: Option<ActivityType>,
    pub started_at: Option<DateTime<Utc>>,
    pub elapsed_duration_seconds: Option<f64>,
    pub timer_duration_seconds: Option<f64>,
}

pub(crate) fn parse(data: &[u8]) -> Result<ParsedTrack, String> {
    let records = from_bytes(data).map_err(|error| error.to_string())?;
    let mut points = Vec::new();
    let mut activity_type = None;
    let mut started_at = None;
    let mut elapsed_duration_seconds = None;
    let mut timer_duration_seconds = None;

    for record in &records {
        match record.kind() {
            MesgNum::Record => {
                if let (Some(latitude), Some(longitude)) = (
                    field_coordinate(record, "position_lat"),
                    field_coordinate(record, "position_long"),
                ) {
                    let point = TrackPoint {
                        latitude,
                        longitude,
                        timestamp: field_timestamp(record, "timestamp"),
                    };
                    if point.has_valid_coordinates() {
                        points.push(point);
                    }
                }
            }
            MesgNum::Session => {
                activity_type = activity_type.or_else(|| field_sport(record));
                started_at = started_at.or_else(|| field_timestamp(record, "start_time"));
                elapsed_duration_seconds =
                    elapsed_duration_seconds.or_else(|| field_number(record, "total_elapsed_time"));
                timer_duration_seconds =
                    timer_duration_seconds.or_else(|| field_number(record, "total_timer_time"));
            }
            MesgNum::Activity => {
                timer_duration_seconds =
                    field_number(record, "total_timer_time").or(timer_duration_seconds);
            }
            _ => {}
        }
    }

    started_at = started_at.or_else(|| points.iter().find_map(|point| point.timestamp));
    Ok(ParsedTrack {
        points,
        activity_type,
        started_at,
        elapsed_duration_seconds,
        timer_duration_seconds,
    })
}

fn field_coordinate(record: &FitDataRecord, name: &str) -> Option<f64> {
    let field = record.fields().iter().find(|field| field.name() == name)?;
    let Value::SInt32(semicircles) = field.value() else {
        return None;
    };
    (*semicircles != i32::MAX).then_some(*semicircles as f64 * 180.0 / 2_147_483_648.0)
}

fn field_sport(record: &FitDataRecord) -> Option<ActivityType> {
    let field = record
        .fields()
        .iter()
        .find(|field| field.name() == "sport")?;
    match field.value() {
        Value::Enum(sport) | Value::Byte(sport) | Value::UInt8(sport) => map_fit_sport(*sport),
        Value::String(sport) => map_fit_sport_name(sport),
        _ => None,
    }
}

fn map_fit_sport_name(sport: &str) -> Option<ActivityType> {
    match sport {
        "running" => Some(ActivityType::Run),
        "cycling" => Some(ActivityType::Ride),
        "swimming" => Some(ActivityType::Swim),
        "walking" => Some(ActivityType::Walk),
        "hiking" => Some(ActivityType::Hike),
        _ => None,
    }
}

fn field_number(record: &FitDataRecord, name: &str) -> Option<f64> {
    record
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .and_then(|field| match field.value() {
            Value::Invalid => None,
            value => value.clone().try_into().ok(),
        })
}

fn field_timestamp(record: &FitDataRecord, name: &str) -> Option<DateTime<Utc>> {
    record
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .and_then(|field| match field.value() {
            Value::Timestamp(timestamp) => Some(timestamp.with_timezone(&Utc)),
            _ => None,
        })
}

fn map_fit_sport(sport: u8) -> Option<ActivityType> {
    match sport {
        1 => Some(ActivityType::Run),
        2 => Some(ActivityType::Ride),
        5 => Some(ActivityType::Swim),
        11 => Some(ActivityType::Walk),
        17 => Some(ActivityType::Hike),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{map_fit_sport, map_fit_sport_name};
    use exercise_domain::ActivityType;

    #[test]
    fn maps_supported_fit_sports() {
        assert_eq!(map_fit_sport(1), Some(ActivityType::Run));
        assert_eq!(map_fit_sport(2), Some(ActivityType::Ride));
        assert_eq!(map_fit_sport(5), Some(ActivityType::Swim));
        assert_eq!(map_fit_sport(11), Some(ActivityType::Walk));
        assert_eq!(map_fit_sport(17), Some(ActivityType::Hike));
        assert_eq!(map_fit_sport(255), None);
        assert_eq!(map_fit_sport_name("walking"), Some(ActivityType::Walk));
        assert_eq!(map_fit_sport_name("unknown"), None);
    }
}
