use chrono::{DateTime, NaiveDateTime, Utc};
use csv::{ReaderBuilder, StringRecord};
use exercise_domain::{ActivityId, ActivityType};

#[derive(Clone, Debug)]
pub(crate) struct ActivityMetadata {
    pub id: ActivityId,
    pub source_id: String,
    pub name: Option<String>,
    pub activity_type: ActivityType,
    pub raw_type: String,
    pub started_at: Option<DateTime<Utc>>,
    pub elapsed_duration_seconds: Option<f64>,
    pub timer_duration_seconds: Option<f64>,
    pub distance_meters: Option<f64>,
    pub filename: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct MetadataRowError {
    pub row_number: usize,
    pub message: String,
}

struct MetadataColumns {
    id: usize,
    activity_type: Option<usize>,
    date: Option<usize>,
    name: Option<usize>,
    elapsed_duration: Option<usize>,
    timer_duration: Option<usize>,
    distance: Option<usize>,
    filename: Option<usize>,
}

pub(crate) fn parse_csv(
    bytes: &[u8],
) -> Result<Vec<Result<ActivityMetadata, MetadataRowError>>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "activity CSV is not UTF-8".to_string())?;
    let text = text.trim_start_matches('\u{feff}');
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|error| format!("could not read activity CSV headers: {error}"))?;
    if headers.is_empty() {
        return Err("activity CSV is empty".into());
    }
    let headers: Vec<String> = headers.iter().map(normalize_header).collect();
    let columns = metadata_columns(&headers)?;
    Ok(reader
        .records()
        .enumerate()
        .filter_map(|(index, record)| {
            let row_number = index + 2;
            match record {
                Ok(row) if row.iter().all(|field| field.trim().is_empty()) => None,
                Ok(row) => Some(parse_activity_row(&row, headers.len(), &columns).map_err(
                    |message| MetadataRowError {
                        row_number,
                        message,
                    },
                )),
                Err(error) => Some(Err(MetadataRowError {
                    row_number,
                    message: error.to_string(),
                })),
            }
        })
        .collect())
}

fn metadata_columns(headers: &[String]) -> Result<MetadataColumns, String> {
    let find = |names: &[&str]| headers.iter().position(|h| names.iter().any(|n| h == n));
    let id = find(&["activiteits-id", "activity id", "activity_id", "id"])
        .ok_or_else(|| "activity CSV has no activity ID column".to_string())?;
    Ok(MetadataColumns {
        id,
        activity_type: find(&["activiteitstype", "activity type", "type"]),
        date: find(&[
            "datum van activiteit",
            "activity date",
            "start date",
            "start_date",
        ]),
        name: find(&["naam activiteit", "activity name", "name"]),
        elapsed_duration: find_last(
            headers,
            &[
                "verstreken tijd",
                "elapsed time",
                "elapsed_time",
                "duration",
            ],
        ),
        timer_duration: find_last(
            headers,
            &[
                "timer time",
                "timer_time",
                "moving time",
                "moving_time",
                "bewegingstijd",
            ],
        ),
        distance: find_last(headers, &["afstand", "distance"]),
        filename: find(&["bestandsnaam", "filename", "file name", "activity file"]),
    })
}

fn parse_activity_row(
    row: &StringRecord,
    expected_columns: usize,
    columns: &MetadataColumns,
) -> Result<ActivityMetadata, String> {
    if row.len() != expected_columns {
        return Err(format!(
            "expected {expected_columns} columns, found {}",
            row.len()
        ));
    }
    let Some(source_id) = cell(row, Some(columns.id)) else {
        return Err("activity ID is missing".into());
    };
    let raw_type = cell(row, columns.activity_type)
        .unwrap_or("Unknown")
        .to_string();
    let activity_type =
        map_activity_type(&raw_type).unwrap_or_else(|| ActivityType::Other(raw_type.clone()));
    Ok(ActivityMetadata {
        id: ActivityId(format!("strava:{source_id}")),
        source_id: source_id.to_string(),
        name: cell(row, columns.name).map(str::to_string),
        activity_type,
        raw_type,
        started_at: cell(row, columns.date).and_then(parse_datetime),
        elapsed_duration_seconds: cell(row, columns.elapsed_duration).and_then(parse_duration),
        timer_duration_seconds: cell(row, columns.timer_duration).and_then(parse_duration),
        distance_meters: cell(row, columns.distance).and_then(parse_number),
        filename: cell(row, columns.filename).map(str::to_string),
    })
}

fn cell(row: &StringRecord, index: Option<usize>) -> Option<&str> {
    index
        .and_then(|i| row.get(i))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

fn find_last(headers: &[String], names: &[&str]) -> Option<usize> {
    headers.iter().rposition(|h| names.iter().any(|n| h == n))
}

fn normalize_header(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .replace('ï', "i")
        .replace('é', "e")
        .replace('ë', "e")
        .replace('á', "a")
}

pub(crate) fn map_activity_type(raw: &str) -> Option<ActivityType> {
    let value = raw.trim().to_lowercase().replace(['_', '-'], " ");
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    match value.as_str() {
        "run" | "running" | "hardloopsessie" => Some(ActivityType::Run),
        "walk" | "walking" | "wandeling" => Some(ActivityType::Walk),
        "hike" | "hiking" | "wandelen" => Some(ActivityType::Hike),
        "ride" | "cycling" | "fietsrit" | "e bike ride" => Some(ActivityType::Ride),
        "swim" | "swimming" | "zwemmen" => Some(ActivityType::Swim),
        _ => None,
    }
}

fn parse_number(value: &str) -> Option<f64> {
    let value = value.trim().replace(' ', "");
    let normalized = if value.contains(',') && value.contains('.') {
        value.replace('.', "").replace(',', ".")
    } else {
        value.replace(',', ".")
    };
    normalized
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.0)
}

fn parse_duration(value: &str) -> Option<f64> {
    if let Some(seconds) = parse_number(value) {
        return Some(seconds);
    }
    let parts: Vec<_> = value.trim().split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let nums: Option<Vec<u64>> = parts.iter().map(|p| p.parse().ok()).collect();
    let nums = nums?;
    let seconds = match nums.as_slice() {
        [m, s] => m * 60 + s,
        [h, m, s] => h * 3600 + m * 60 + s,
        _ => return None,
    };
    Some(seconds as f64)
}

pub(crate) fn parse_datetime(value: &str) -> Option<DateTime<Utc>> {
    let value = value.trim();
    if let Ok(date) = DateTime::parse_from_rfc3339(value) {
        return Some(date.with_timezone(&Utc));
    }
    let value = [
        ("okt", "Oct"),
        ("mrt", "Mar"),
        ("mei", "May"),
        ("juni", "Jun"),
        ("juli", "Jul"),
        ("jan", "Jan"),
        ("feb", "Feb"),
        ("apr", "Apr"),
        ("jun", "Jun"),
        ("aug", "Aug"),
        ("sep", "Sep"),
        ("okt", "Oct"),
        ("nov", "Nov"),
        ("dec", "Dec"),
    ]
    .iter()
    .fold(value.to_string(), |s, (a, b)| {
        s.replace(a, b).replace(&a.to_uppercase(), b)
    });
    for fmt in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%d %b %Y, %H:%M:%S",
        "%b %d, %Y, %I:%M:%S %p",
        "%B %d, %Y, %I:%M:%S %p",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&value, fmt) {
            return Some(dt.and_utc());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{map_activity_type, parse_csv, parse_datetime, parse_duration, parse_number};
    use exercise_domain::ActivityType;

    #[test]
    fn maps_common_english_and_dutch_activity_types() {
        assert_eq!(map_activity_type("Run"), Some(ActivityType::Run));
        assert_eq!(map_activity_type("wandeling"), Some(ActivityType::Walk));
        assert_eq!(map_activity_type("Fietsrit"), Some(ActivityType::Ride));
        assert_eq!(map_activity_type("Zwemmen"), Some(ActivityType::Swim));
        assert_eq!(map_activity_type("Yoga"), None);
    }

    #[test]
    fn parses_localized_numbers_durations_and_dates() {
        assert_eq!(parse_number("1.234,5"), Some(1234.5));
        assert_eq!(parse_duration("1:02:03"), Some(3723.0));
        assert_eq!(
            parse_datetime("2024-08-20T09:41:13Z").unwrap().to_rfc3339(),
            "2024-08-20T09:41:13+00:00"
        );
    }

    #[test]
    fn parses_quoted_csv_fields_and_keeps_bad_rows_local() {
        let csv = concat!(
            "Activiteits-ID,Activiteitstype,Naam activiteit,Bestandsnaam\n",
            "42,Run,\"Morning, easy run\",activities/run.gpx\n",
            "43,Walk,too,few,columns\n",
        );
        let rows = parse_csv(csv.as_bytes()).unwrap();
        let activity = rows[0].as_ref().unwrap();
        assert_eq!(activity.source_id, "42");
        assert_eq!(activity.name.as_deref(), Some("Morning, easy run"));
        assert_eq!(rows[1].as_ref().unwrap_err().row_number, 3);
    }

    #[test]
    fn maps_csv_elapsed_and_timer_columns_separately() {
        let csv = concat!(
            "Activity ID,Elapsed Time,Moving Time\n",
            "42,1:02:03,58:10\n",
        );
        let rows = parse_csv(csv.as_bytes()).unwrap();
        let activity = rows[0].as_ref().unwrap();
        assert_eq!(activity.elapsed_duration_seconds, Some(3723.0));
        assert_eq!(activity.timer_duration_seconds, Some(3490.0));
    }

    #[test]
    fn parses_newlines_inside_quoted_fields() {
        let csv = concat!("Activity ID,Activity Name\n", "42,\"Long\n", "run\"\n",);
        let rows = parse_csv(csv.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].as_ref().unwrap().name.as_deref(), Some("Long\nrun"));
    }
}
