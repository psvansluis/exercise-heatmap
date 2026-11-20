use crate::metadata::map_activity_type;
use chrono::{DateTime, Utc};
use exercise_domain::{ActivityType, TrackPoint};

pub(crate) struct ParsedTrack {
    pub points: Vec<TrackPoint>,
    pub activity_type: Option<ActivityType>,
    pub started_at: Option<DateTime<Utc>>,
}

#[derive(Default)]
struct ParserState {
    points: Vec<TrackPoint>,
    sport: Option<ActivityType>,
    point_index: Option<usize>,
}

pub(crate) fn parse(bytes: &[u8]) -> Result<ParsedTrack, String> {
    let xml = std::str::from_utf8(bytes).map_err(|_| "GPX is not UTF-8".to_string())?;
    if xml.to_ascii_uppercase().contains("<!DOCTYPE") {
        return Err("GPX document types are not accepted".into());
    }
    let mut state = ParserState::default();
    let mut i = 0;
    while let Some(rel) = xml[i..].find('<') {
        let start = i + rel;
        let preceding_text = decode_entities(xml[i..start].trim());
        let Some(close_rel) = xml[start..].find('>') else {
            return Err("unterminated GPX tag".into());
        };
        let end = start + close_rel;
        let token = xml[start + 1..end].trim();
        i = end + 1;
        if token.starts_with("!--") {
            if let Some(c) = xml[i..].find("-->") {
                i += c + 3;
                continue;
            } else {
                return Err("unterminated GPX comment".into());
            }
        }
        state.consume_token(token, &preceding_text)?;
    }
    Ok(ParsedTrack {
        started_at: state.points.iter().find_map(|point| point.timestamp),
        points: state.points,
        activity_type: state.sport,
    })
}

impl ParserState {
    fn consume_token(&mut self, token: &str, text: &str) -> Result<(), String> {
        if token.starts_with('?') || token.starts_with('!') {
            return Ok(());
        }
        if let Some(closing) = token.strip_prefix('/') {
            self.consume_close_tag(local_name(closing.trim()), text);
            return Ok(());
        }
        let self_closing = token.ends_with('/');
        let token = if self_closing {
            token[..token.len() - 1].trim_end()
        } else {
            token
        };
        let split = token.find(char::is_whitespace).unwrap_or(token.len());
        let name = local_name(&token[..split]);
        if name == "trkpt" || name == "rtept" {
            self.add_point(&token[split..])?;
        }
        if name.is_empty() {
            return Err("invalid GPX element".into());
        }
        if self_closing {
            self.consume_close_tag(name, "");
        }
        Ok(())
    }

    fn consume_close_tag(&mut self, name: &str, text: &str) {
        match name {
            "time" => {
                if let Some(index) = self.point_index {
                    self.points[index].timestamp = parse_gpx_time(text);
                }
            }
            "type" if self.sport.is_none() => self.sport = map_activity_type(text.trim()),
            "trkpt" | "rtept" => self.point_index = None,
            _ => {}
        }
    }

    fn add_point(&mut self, attributes: &str) -> Result<(), String> {
        let attrs = parse_attributes(attributes)?;
        let lat = attrs.get("lat").and_then(|s| s.parse::<f64>().ok());
        let lon = attrs.get("lon").and_then(|s| s.parse::<f64>().ok());
        if let (Some(latitude), Some(longitude)) = (lat, lon) {
            let point = TrackPoint {
                latitude,
                longitude,
                timestamp: None,
            };
            if point.has_valid_coordinates() {
                self.points.push(point);
                self.point_index = Some(self.points.len() - 1);
            }
        }
        Ok(())
    }
}

fn local_name(s: &str) -> &str {
    s.rsplit(':').next().unwrap_or(s).trim()
}

fn parse_attributes(text: &str) -> Result<std::collections::HashMap<String, String>, String> {
    let mut result = std::collections::HashMap::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else { break };
        let key = rest[..eq].trim().trim_end_matches('/').to_string();
        rest = rest[eq + 1..].trim_start();
        let Some(quote) = rest.chars().next() else {
            break;
        };
        if quote != '\'' && quote != '"' {
            return Err("invalid GPX attribute".into());
        }
        rest = &rest[1..];
        let Some(end) = rest.find(quote) else {
            return Err("unterminated GPX attribute".into());
        };
        result.insert(local_name(&key).to_string(), decode_entities(&rest[..end]));
        rest = rest[end + 1..].trim_start();
    }
    Ok(result)
}

fn decode_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn parse_gpx_time(value: &str) -> Option<DateTime<Utc>> {
    let value = decode_entities(value.trim());
    DateTime::parse_from_rfc3339(&value)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}
