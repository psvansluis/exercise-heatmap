use crate::components::maplibre_map::MapLibreMap;
use exercise_domain::{Activity, ActivityType};
use exercise_import::{
    ImportEntryOutcome, ImportReport, ParseOptions, parse_strava_zip_with_options,
};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Event, HtmlInputElement};

#[derive(Clone)]
struct ActivityTableRow {
    name: String,
    timestamp: String,
    activity_type: String,
    distance: String,
    duration: String,
    status: String,
}

const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;

#[component]
pub fn Home() -> impl IntoView {
    let rows = RwSignal::new(Vec::<ActivityTableRow>::new());
    let is_importing = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let selected_filename = RwSignal::new(None::<String>);

    let on_file_change = move |event: Event| {
        let input = event
            .target()
            .and_then(|target| target.dyn_into::<HtmlInputElement>().ok());
        let Some(input) = input else { return };
        let Some(file) = input.files().and_then(|files| files.get(0)) else {
            return;
        };
        input.set_value("");

        selected_filename.set(Some(file.name()));
        error.set(None);
        rows.set(Vec::new());
        if file.size() > MAX_ARCHIVE_BYTES as f64 {
            error.set(Some(
                "The selected ZIP exceeds the 256 MB import limit.".into(),
            ));
            return;
        }

        is_importing.set(true);
        leptos::task::spawn_local(async move {
            let file_bytes = JsFuture::from(file.array_buffer()).await;
            match file_bytes {
                Ok(buffer) => {
                    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                    let options = ParseOptions::default();
                    match parse_strava_zip_with_options(&bytes, &options) {
                        Ok(report) => rows.set(rows_from_report(report)),
                        Err(parse_error) => error.set(Some(parse_error.to_string())),
                    }
                }
                Err(_) => error.set(Some("The selected ZIP could not be read.".into())),
            }
            is_importing.set(false);
        });
    };

    view! {
        <main class="page-shell">
            <header class="hero">
                <p class="eyebrow">"PERSONAL ACTIVITY EXPLORER"</p>
                <h1>"Exercise heatmap"</h1>
                <p class="hero-copy">
                    "Explore your walks and runs on a map, see the routes you have taken, and discover the streets still waiting to be explored."
                </p>
            </header>

            <section class="import-panel" aria-labelledby="import-title">
                <div class="import-heading">
                    <div>
                        <p class="eyebrow">"YOUR LOCAL DATA"</p>
                        <h2 id="import-title">"Import an activity archive"</h2>
                        <p class="import-copy">
                            "Choose a Strava ZIP. It is read and parsed in this browser; activity data is not uploaded."
                        </p>
                    </div>
                    <label class="file-picker">
                        <span>{move || if is_importing.get() { "Reading archive…" } else { "Choose ZIP file" }}</span>
                        <input
                            type="file"
                            accept=".zip,application/zip"
                            aria-label="Choose Strava ZIP file"
                            on:change=on_file_change
                            disabled=move || is_importing.get()
                        />
                    </label>
                </div>

                <div class="import-status" aria-live="polite">
                    {move || {
                        if is_importing.get() {
                            "Reading and parsing the selected archive in this browser.".to_string()
                        } else if let Some(message) = error.get() {
                            message
                        } else if let Some(filename) = selected_filename.get() {
                            format!("Finished processing {filename}.")
                        } else {
                            "Your activity details remain on this device.".to_string()
                        }
                    }}
                </div>

                <Show when=move || !rows.get().is_empty()>
                    <div class="activity-table-scroll" role="region" aria-label="Imported activity results" tabindex="0">
                        <table class="activity-table">
                            <thead>
                                <tr>
                                    <th scope="col">"Activity name"</th>
                                    <th scope="col">"Timestamp"</th>
                                    <th scope="col">"Activity type"</th>
                                    <th scope="col">"Distance"</th>
                                    <th scope="col">"Time spent"</th>
                                    <th scope="col">"Import result"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {move || rows.get().into_iter().map(|row| view! {
                                    <tr>
                                        <td>{row.name}</td>
                                        <td>{row.timestamp}</td>
                                        <td>{row.activity_type}</td>
                                        <td>{row.distance}</td>
                                        <td>{row.duration}</td>
                                        <td class="activity-result">{row.status}</td>
                                    </tr>
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                    <p class="activity-count">
                        {move || format!("{} result rows", rows.get().len())}
                    </p>
                </Show>
            </section>

            <MapLibreMap />

            <footer class="page-footer">
                "A personal project for making exercise history easier to explore."
            </footer>
        </main>
    }
}

fn rows_from_report(report: ImportReport) -> Vec<ActivityTableRow> {
    report
        .entries
        .into_iter()
        .map(|outcome| match outcome {
            ImportEntryOutcome::Imported { activity, track } => activity_row(
                activity,
                format!("Imported · {} points", track.points.len()),
            ),
            ImportEntryOutcome::MetadataOnly { activity, reason } => {
                activity_row(activity, format!("Metadata only · {reason}"))
            }
            ImportEntryOutcome::Skipped { path, reason } => {
                empty_row(path, format!("Skipped · {reason}"))
            }
            ImportEntryOutcome::Duplicate { path } => empty_row(path, "Duplicate activity".into()),
            ImportEntryOutcome::Malformed { path, reason } => {
                empty_row(path, format!("Malformed · {reason}"))
            }
        })
        .collect()
}

fn activity_row(activity: Activity, status: String) -> ActivityTableRow {
    ActivityTableRow {
        name: activity.name.unwrap_or_else(|| "—".into()),
        timestamp: activity
            .started_at
            .map(|time| time.to_rfc3339())
            .unwrap_or_else(|| "—".into()),
        activity_type: display_activity_type(&activity.activity_type),
        distance: activity
            .distance_meters
            .map(format_distance)
            .unwrap_or_else(|| "—".into()),
        duration: activity
            .duration_seconds
            .map(format_duration)
            .unwrap_or_else(|| "—".into()),
        status,
    }
}

fn empty_row(name: String, status: String) -> ActivityTableRow {
    ActivityTableRow {
        name,
        timestamp: "—".into(),
        activity_type: "—".into(),
        distance: "—".into(),
        duration: "—".into(),
        status,
    }
}

fn display_activity_type(activity_type: &ActivityType) -> String {
    match activity_type {
        ActivityType::Run => "Run".into(),
        ActivityType::Walk => "Walk".into(),
        ActivityType::Hike => "Hike".into(),
        ActivityType::Ride => "Ride".into(),
        ActivityType::Swim => "Swim".into(),
        ActivityType::Other(raw) => raw.clone(),
    }
}

fn format_distance(meters: f64) -> String {
    if meters >= 1000.0 {
        format!("{:.2} km", meters / 1000.0)
    } else {
        format!("{meters:.0} m")
    }
}

fn format_duration(seconds: f64) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let remainder = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{remainder:02}")
    } else {
        format!("{minutes}:{remainder:02}")
    }
}
