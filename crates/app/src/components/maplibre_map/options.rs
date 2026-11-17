use std::collections::BTreeMap;

use serde::Serialize;

/// Camera, controls, and style configuration for the map renderer.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapOptions {
    pub center: [f64; 2],
    pub zoom: f64,
    pub min_zoom: f64,
    pub max_zoom: f64,
    pub attribution_control: AttributionControl,
    pub style: MapStyle,
}

impl MapOptions {
    /// Configuration for the OpenStreetMap raster basemap this app renders.
    pub fn openstreetmap(
        center: [f64; 2],
        zoom: f64,
        min_zoom: f64,
        max_zoom: f64,
        tile_url: String,
        tile_attribution: String,
    ) -> Self {
        Self {
            center,
            zoom,
            min_zoom,
            max_zoom,
            attribution_control: AttributionControl { compact: true },
            style: MapStyle {
                version: 8,
                sources: BTreeMap::from([(
                    "openstreetmap".to_string(),
                    TileSource {
                        source_type: "raster".into(),
                        tiles: vec![tile_url],
                        tile_size: 256,
                        attribution: tile_attribution,
                    },
                )]),
                layers: vec![RasterLayer {
                    id: "openstreetmap-raster".into(),
                    layer_type: "raster".into(),
                    source: "openstreetmap".into(),
                    minzoom: 0,
                    maxzoom: 19,
                }],
            },
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionControl {
    pub compact: bool,
}

/// Mapbox GL style v8 document, restricted to the raster sources the heatmap
/// currently uses.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapStyle {
    pub version: u8,
    pub sources: BTreeMap<String, TileSource>,
    pub layers: Vec<RasterLayer>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub tiles: Vec<String>,
    pub tile_size: u32,
    pub attribution: String,
}

#[derive(Clone, Serialize)]
pub struct RasterLayer {
    pub id: String,
    #[serde(rename = "type")]
    pub layer_type: String,
    pub source: String,
    pub minzoom: u8,
    pub maxzoom: u8,
}
