use chrono::{DateTime, NaiveDateTime, Utc};
use exercise_domain::{ActivityId, ActivityType};

#[derive(Clone, Debug)]
pub(crate) struct ActivityMetadata {
    pub id: ActivityId,
    pub source_id: String,
    pub name: Option<String>,
    pub activity_type: ActivityType,
    pub raw_type: String,
    pub started_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<f64>,
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
    duration: Option<usize>,
    distance: Option<usize>,
    filename: Option<usize>,
}

pub(crate) fn parse_csv(
    bytes: &[u8],
) -> Result<Vec<Result<ActivityMetadata, MetadataRowError>>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "activity CSV is not UTF-8".to_string())?;
    let text = text.trim_start_matches('\u{feff}');
    let rows = parse_records(text)?;
    let Some(headers) = rows.first() else {
        return Err("activity CSV is empty".into());
    };
    let headers: Vec<String> = headers.iter().map(|h| normalize_header(h)).collect();
    let columns = metadata_columns(&headers)?;
    Ok(parse_metadata_rows(&rows, &headers, &columns))
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
        duration: find_last(
            headers,
            &[
                "verstreken tijd",
                "elapsed time",
                "elapsed_time",
                "duration",
            ],
        ),
        distance: find_last(headers, &["afstand", "distance"]),
        filename: find(&["bestandsnaam", "filename", "file name", "activity file"]),
    })
}

fn parse_metadata_rows(
    rows: &[Vec<String>],
    headers: &[String],
    columns: &MetadataColumns,
) -> Vec<Result<ActivityMetadata, MetadataRowError>> {
    rows.iter()
        .enumerate()
        .skip(1)
        .filter_map(|(index, row)| {
            if row.iter().all(|field| field.trim().is_empty()) {
                return None;
            }
            let row_number = index + 1;
            Some(
                parse_activity_row(row, headers.len(), columns).map_err(|message| {
                    MetadataRowError {
                        row_number,
                        message,
                    }
                }),
            )
        })
        .collect()
}

fn parse_activity_row(
    row: &[String],
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
        duration_seconds: cell(row, columns.duration).and_then(parse_duration),
        distance_meters: cell(row, columns.distance).and_then(parse_number),
        filename: cell(row, columns.filename).map(str::to_string),
    })
}

fn cell(row: &[String], index: Option<usize>) -> Option<&str> {
    index
        .and_then(|i| row.get(i))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

fn find_last(headers: &[String], names: &[&str]) -> Option<usize> {
    headers.iter().rposition(|h| names.iter().any(|n| h == n))
}

fn parse_records(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                row.push(std::mem::take(&mut field));
            }
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' if !quoted => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    if quoted {
        return Err("activity CSV has an unterminated quoted field".into());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
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
