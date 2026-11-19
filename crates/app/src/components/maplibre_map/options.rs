use std::collections::BTreeMap;

use serde::Serialize;

/// Camera, controls, and style configuration for the map renderer.
#[derive(Serialize)]
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionControl {
    pub compact: bool,
}

/// Mapbox GL style v8 document, restricted to the raster sources the heatmap
/// currently uses.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapStyle {
    pub version: u8,
    pub sources: BTreeMap<String, TileSource>,
    pub layers: Vec<RasterLayer>,
}

#[derive(Serialize)]
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

#[cfg(test)]
mod tests {
    use super::MapOptions;
    use serde::Serialize;
    use serde_wasm_bindgen::Serializer;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_test::*;

    use crate::components::maplibre_map::adapter;

    fn sample() -> MapOptions {
        MapOptions::openstreetmap(
            [4.895, 52.37],
            10.0,
            2.0,
            19.0,
            "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_string(),
            "© OpenStreetMap contributors".to_string(),
        )
    }

    /// Mirrors the serialization in `create_map`.
    fn to_js(options: &MapOptions) -> JsValue {
        options
            .serialize(&Serializer::new().serialize_maps_as_objects(true))
            .expect("MapOptions must serialize to a JsValue")
    }

    #[wasm_bindgen_test]
    fn sources_serialize_as_plain_object() {
        let style = js_sys::Reflect::get(&to_js(&sample()), &"style".into()).unwrap();
        let sources = js_sys::Reflect::get(&style, &"sources".into()).unwrap();
        let json = js_sys::JSON::stringify(&sources)
            .unwrap()
            .as_string()
            .expect("stringify returns a JS string");
        // An ES Map stringifies to "{}" and MapLibre's style validator then
        // fails with: layers[0]: source "openstreetmap" not found.
        assert!(
            json.contains("\"openstreetmap\""),
            "sources must be a plain object, got: {json}"
        );
    }

    #[wasm_bindgen_test]
    fn keys_use_maplibre_camel_case() {
        let json = js_sys::JSON::stringify(&to_js(&sample()))
            .unwrap()
            .as_string()
            .expect("stringify returns a JS string");
        for key in [
            "center",
            "zoom",
            "minZoom",
            "maxZoom",
            "attributionControl",
            "style",
            "tileSize",
        ] {
            assert!(
                json.contains(&format!("\"{key}\"")),
                "missing {key}: {json}"
            );
        }
    }

    #[wasm_bindgen_test]
    fn adapter_reasons_map_from_js_values() {
        let with_reason = |reason: &str| {
            let error = js_sys::Error::new(reason);
            js_sys::Reflect::set(&error, &"reason".into(), &reason.into()).unwrap();
            error.into()
        };
        assert_eq!(
            adapter::AdapterReason::from_js(&with_reason("map-module")),
            adapter::AdapterReason::Module
        );
        assert_eq!(
            adapter::AdapterReason::from_js(&with_reason("map-init")),
            adapter::AdapterReason::Init
        );
        assert_eq!(
            adapter::AdapterReason::from_js(&with_reason("removed")),
            adapter::AdapterReason::Removed
        );
        assert_eq!(
            adapter::AdapterReason::from_js(&with_reason("who-knows")),
            adapter::AdapterReason::Other
        );
        assert_eq!(
            adapter::AdapterReason::from_js(&{ js_sys::Error::new("no reason").into() }),
            adapter::AdapterReason::Other
        );
    }
}
