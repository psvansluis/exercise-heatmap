use chrono::{DateTime, TimeZone, Utc};
use exercise_domain::{ActivityType, TrackPoint};

pub(crate) struct ParsedTrack {
    pub points: Vec<TrackPoint>,
    pub activity_type: Option<ActivityType>,
    pub started_at: Option<DateTime<Utc>>,
}

#[derive(Default)]
struct DataValues {
    latitude: Option<i32>,
    longitude: Option<i32>,
    timestamp: Option<u32>,
    activity_started_at: Option<u32>,
    sport: Option<u8>,
}

struct FitParser {
    definitions: [Option<Definition>; 16],
    points: Vec<TrackPoint>,
    activity_type: Option<ActivityType>,
    activity_started_at: Option<DateTime<Utc>>,
    last_timestamp: Option<u32>,
}

#[derive(Clone)]
struct Definition {
    global: u16,
    endian: bool,
    fields: Vec<(u8, usize, u8)>,
    developer_sizes: Vec<usize>,
}

pub(crate) fn parse(data: &[u8]) -> Result<ParsedTrack, String> {
    let (header_size, end) = validate_file(data)?;
    let mut parser = FitParser {
        definitions: std::array::from_fn(|_| None),
        points: Vec::new(),
        activity_type: None,
        activity_started_at: None,
        last_timestamp: None,
    };
    let mut pos = header_size;
    while pos < end {
        let header = data[pos];
        pos += 1;
        let compressed = header & 0x80 != 0;
        let local = message_type(header, compressed);
        pos = if header & 0x40 != 0 {
            parser.read_definition(data, pos, end, header, local)?
        } else {
            parser.read_data(data, pos, end, header, local)?
        };
    }
    let started_at = parser
        .activity_started_at
        .or_else(|| parser.points.iter().find_map(|point| point.timestamp));
    Ok(ParsedTrack {
        points: parser.points,
        activity_type: parser.activity_type,
        started_at,
    })
}

fn validate_file(data: &[u8]) -> Result<(usize, usize), String> {
    if data.len() < 14 || data.get(8..12) != Some(b".FIT") {
        return Err("invalid FIT header".into());
    }
    let header_size = data[0] as usize;
    if header_size < 12 || header_size > 64 || data.len() < header_size + 2 {
        return Err("invalid FIT header size".into());
    }
    let data_size = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let end = header_size
        .checked_add(data_size)
        .ok_or("FIT size overflow")?;
    if end + 2 > data.len() {
        return Err("truncated FIT data".into());
    }
    if data[header_size..end].len() != data_size {
        return Err("truncated FIT data".into());
    }
    if u16::from_le_bytes(data[end..end + 2].try_into().unwrap()) != crc(&data[..end]) {
        return Err("FIT checksum mismatch".into());
    }
    if header_size >= 14 && u16::from_le_bytes(data[12..14].try_into().unwrap()) != crc(&data[..12])
    {
        return Err("FIT header checksum mismatch".into());
    }
    Ok((header_size, end))
}

fn message_type(header: u8, compressed: bool) -> usize {
    if compressed {
        ((header >> 5) & 0x03) as usize
    } else {
        (header & 0x0f) as usize
    }
}

impl FitParser {
    fn read_definition(
        &mut self,
        data: &[u8],
        mut pos: usize,
        end: usize,
        header: u8,
        local: usize,
    ) -> Result<usize, String> {
        if pos + 5 > end {
            return Err("truncated FIT definition".into());
        }
        let reserved = data[pos];
        let big_endian = data[pos + 1] != 0;
        pos += 2;
        let global = read_u16(&data[pos..pos + 2], big_endian);
        pos += 2;
        let count = data[pos] as usize;
        pos += 1;
        if count > 128 || pos + count * 3 > end {
            return Err("invalid FIT field definition".into());
        }
        let mut fields = Vec::with_capacity(count);
        for _ in 0..count {
            fields.push((data[pos], data[pos + 1] as usize, data[pos + 2]));
            pos += 3;
        }
        let developer_sizes = read_developer_fields(data, &mut pos, end, header)?;
        self.definitions[local] = Some(Definition {
            global,
            endian: big_endian,
            fields,
            developer_sizes,
        });
        let _ = reserved;
        Ok(pos)
    }

    fn read_data(
        &mut self,
        data: &[u8],
        mut pos: usize,
        end: usize,
        header: u8,
        local: usize,
    ) -> Result<usize, String> {
        let Some(definition) = self.definitions[local].clone() else {
            return Err("FIT data message has no definition".into());
        };
        let total = definition.fields.iter().map(|field| field.1).sum::<usize>()
            + definition.developer_sizes.iter().sum::<usize>();
        if pos + total > end {
            return Err("truncated FIT message".into());
        }
        let mut values = DataValues::default();
        for (number, size, base) in &definition.fields {
            let field = &data[pos..pos + *size];
            read_field(
                &mut values,
                definition.global,
                *number,
                *size,
                *base,
                field,
                definition.endian,
            );
            pos += *size;
        }
        pos += definition.developer_sizes.iter().sum::<usize>();
        self.consume_values(definition.global, header, values);
        Ok(pos)
    }

    fn consume_values(&mut self, global: u16, header: u8, mut values: DataValues) {
        if values.timestamp == Some(u32::MAX) {
            values.timestamp = None;
        } else if let Some(timestamp) = values.timestamp {
            self.last_timestamp = Some(timestamp);
        }
        if global == 18 && self.activity_type.is_none() {
            self.activity_type = values.sport.and_then(map_fit_sport);
        }
        if global == 18 && self.activity_started_at.is_none() {
            self.activity_started_at = values
                .activity_started_at
                .filter(|timestamp| *timestamp != u32::MAX)
                .and_then(fit_time);
        }
        if global != 20 {
            return;
        }
        if values.timestamp.is_none() && header & 0x80 != 0 {
            values.timestamp = compressed_timestamp(self.last_timestamp, header & 0x1f);
            if let Some(timestamp) = values.timestamp {
                self.last_timestamp = Some(timestamp);
            }
        }
        if let (Some(lat), Some(lon)) = (
            values.latitude.filter(|value| *value != i32::MAX),
            values.longitude.filter(|value| *value != i32::MAX),
        ) {
            let point = TrackPoint {
                latitude: lat as f64 * 180.0 / 2_147_483_648.0,
                longitude: lon as f64 * 180.0 / 2_147_483_648.0,
                timestamp: values.timestamp.and_then(fit_time),
            };
            if point.has_valid_coordinates() {
                self.points.push(point);
            }
        }
    }
}

fn read_developer_fields(
    data: &[u8],
    pos: &mut usize,
    end: usize,
    header: u8,
) -> Result<Vec<usize>, String> {
    if header & 0x20 == 0 {
        return Ok(Vec::new());
    }
    if *pos >= end {
        return Err("truncated FIT developer fields".into());
    }
    let count = data[*pos] as usize;
    *pos += 1;
    let mut sizes = Vec::with_capacity(count);
    for _ in 0..count {
        if *pos + 3 > end {
            return Err("truncated FIT developer field".into());
        }
        sizes.push(data[*pos + 1] as usize);
        *pos += 3;
    }
    Ok(sizes)
}

fn read_field(
    values: &mut DataValues,
    global: u16,
    number: u8,
    size: usize,
    base: u8,
    field: &[u8],
    big_endian: bool,
) {
    match (global, number) {
        (20, 0) if size == 4 && base & 0x1f == 5 => {
            values.latitude = Some(read_i32(field, big_endian))
        }
        (20, 1) if size == 4 && base & 0x1f == 5 => {
            values.longitude = Some(read_i32(field, big_endian))
        }
        (_, 253) if size == 4 => values.timestamp = Some(read_u32(field, big_endian)),
        (18, 2) if size == 4 => values.activity_started_at = Some(read_u32(field, big_endian)),
        (18, 5) if size == 1 => values.sport = Some(field[0]),
        _ => {}
    }
}

fn compressed_timestamp(last: Option<u32>, offset: u8) -> Option<u32> {
    last.map(|previous| {
        let mut timestamp = (previous & !0x1f) | u32::from(offset);
        if timestamp < previous {
            timestamp += 0x20;
        }
        timestamp
    })
}

fn read_u16(bytes: &[u8], big: bool) -> u16 {
    if big {
        u16::from_be_bytes(bytes.try_into().unwrap())
    } else {
        u16::from_le_bytes(bytes.try_into().unwrap())
    }
}
fn read_i32(bytes: &[u8], big: bool) -> i32 {
    if big {
        i32::from_be_bytes(bytes.try_into().unwrap())
    } else {
        i32::from_le_bytes(bytes.try_into().unwrap())
    }
}
fn read_u32(bytes: &[u8], big: bool) -> u32 {
    if big {
        u32::from_be_bytes(bytes.try_into().unwrap())
    } else {
        u32::from_le_bytes(bytes.try_into().unwrap())
    }
}

fn fit_time(seconds: u32) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(seconds as i64 + 631_065_600, 0).single()
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

fn crc(data: &[u8]) -> u16 {
    const TABLE: [u16; 16] = [
        0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401, 0xA001, 0x6C00, 0x7800,
        0xB401, 0x5000, 0x9C01, 0x8801, 0x4400,
    ];
    let mut crc = 0u16;
    for b in data {
        let mut tmp = TABLE[(crc & 0xf) as usize];
        crc = (crc >> 4) & 0x0fff;
        crc ^= tmp ^ TABLE[(b & 0xf) as usize];
        tmp = TABLE[(crc & 0xf) as usize];
        crc = (crc >> 4) & 0x0fff;
        crc ^= tmp ^ TABLE[(b >> 4) as usize];
    }
    crc
}
