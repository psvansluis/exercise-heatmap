use std::collections::{BTreeMap, HashMap};

use color::{AlphaColor, Srgb};
use exercise_domain::ActivityType;
use serde::{Serialize, Serializer};

use super::geojson::activity_type_label;

/// Opaque sRGB colour used for one track line on the map.
pub type TrackColor = AlphaColor<Srgb>;

/// Shape sent to the JS adapter. Keys are the same labels written into the
/// GeoJSON `activityType` property by [`super::geojson`].
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivityColorsJs {
    colors: BTreeMap<String, String>,
    default: String,
}

/// Palette for colouring tracks by activity type. Unlisted types (including
/// any `Other` variant) use `default`.
#[derive(Clone, Debug, PartialEq)]
pub struct ActivityColors {
    by_type: HashMap<ActivityType, TrackColor>,
    default: TrackColor,
}

impl ActivityColors {
    /// The palette currently used by the map layer.
    pub fn default_palette() -> Self {
        Self::new(TrackColor::from_rgb8(0x7c, 0x3a, 0xed))
            .with(ActivityType::Run, TrackColor::from_rgb8(0xe0, 0x52, 0x3c))
            .with(ActivityType::Ride, TrackColor::from_rgb8(0x25, 0x64, 0xeb))
            .with(ActivityType::Walk, TrackColor::from_rgb8(0x0d, 0x94, 0x89))
            .with(ActivityType::Hike, TrackColor::from_rgb8(0xd9, 0x77, 0x06))
            .with(ActivityType::Swim, TrackColor::from_rgb8(0x02, 0x84, 0xc7))
    }

    pub fn new(default: TrackColor) -> Self {
        Self {
            by_type: HashMap::new(),
            default,
        }
    }

    /// Sets the colour for `activity_type`, consuming and returning `self` so
    /// construction can be chained.
    pub fn with(mut self, activity_type: ActivityType, color: TrackColor) -> Self {
        self.set(activity_type, color);
        self
    }

    pub fn set(&mut self, activity_type: ActivityType, color: TrackColor) {
        self.by_type.insert(activity_type, color);
    }
}

impl Default for ActivityColors {
    fn default() -> Self {
        Self::default_palette()
    }
}

impl Serialize for ActivityColors {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let colors = self
            .by_type
            .iter()
            .map(|(activity_type, color)| (activity_type_label(activity_type), to_hex(color)))
            .collect::<BTreeMap<_, _>>();
        ActivityColorsJs {
            colors,
            default: to_hex(&self.default),
        }
        .serialize(serializer)
    }
}

/// Formats an opaque sRGB colour as `#rrggbb`, the form MapLibre expects.
fn to_hex(color: &TrackColor) -> String {
    let rgba = color.to_rgba8();
    format!("#{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(color: &TrackColor) -> String {
        to_hex(color)
    }

    #[test]
    fn default_palette_covers_the_known_types() {
        let palette = ActivityColors::default_palette();
        for activity_type in [
            ActivityType::Run,
            ActivityType::Ride,
            ActivityType::Walk,
            ActivityType::Hike,
            ActivityType::Swim,
        ] {
            assert!(
                palette.by_type.contains_key(&activity_type),
                "missing colour for {activity_type:?}"
            );
        }
    }

    #[test]
    fn serializes_to_labels_and_hex() {
        let palette = ActivityColors::new(TrackColor::from_rgb8(0x00, 0x00, 0x00))
            .with(ActivityType::Run, TrackColor::from_rgb8(0xe0, 0x52, 0x3c))
            .with(
                ActivityType::Other("Yoga".into()),
                TrackColor::from_rgb8(0x25, 0x64, 0xeb),
            );

        let json = serde_json::json!(palette);
        assert_eq!(
            json,
            serde_json::json!({
                "colors": {
                    "Run": "#e0523c",
                    "Yoga": "#2564eb",
                },
                "default": "#000000",
            })
        );
    }

    #[test]
    fn hex_omits_alpha_and_lowercases() {
        assert_eq!(hex(&TrackColor::from_rgb8(0xE0, 0x52, 0x3C)), "#e0523c");
        assert_eq!(
            hex(&TrackColor::from_rgba8(0x00, 0x00, 0x00, 0x00)),
            "#000000"
        );
    }
}
