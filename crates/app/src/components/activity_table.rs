use exercise_domain::{Activity, ActivityType};
use leptos::prelude::*;

struct ActivityTableRow {
    name: String,
    timestamp: String,
    activity_type: String,
    distance: String,
    elapsed_duration: String,
    timer_duration: String,
    status: String,
}

impl From<Activity> for ActivityTableRow {
    fn from(activity: Activity) -> Self {
        let track_status = if activity.bounds.is_some() {
            "Track available"
        } else {
            "Metadata only"
        };
        Self {
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
            elapsed_duration: activity
                .elapsed_duration_seconds
                .map(format_duration)
                .unwrap_or_else(|| "—".into()),
            timer_duration: activity
                .timer_duration_seconds
                .map(format_duration)
                .unwrap_or_else(|| "—".into()),
            status: track_status.into(),
        }
    }
}

#[component]
pub fn ActivityTable(activities: RwSignal<Vec<Activity>>) -> impl IntoView {
    view! {
        <Show when=move || !activities.get().is_empty()>
            <div class="activity-table-scroll" role="region" aria-label="Imported activity results" tabindex="0">
                <table class="activity-table">
                    <thead>
                        <tr>
                            <th scope="col">"Activity name"</th>
                            <th scope="col">"Timestamp"</th>
                            <th scope="col">"Activity type"</th>
                            <th scope="col">"Distance"</th>
                            <th scope="col">"Elapsed time"</th>
                            <th scope="col">"Timer time"</th>
                            <th scope="col">"Track"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || activities.get().into_iter().map(ActivityTableRow::from).map(|row| view! {
                            <tr>
                                <td>{row.name}</td>
                                <td>{row.timestamp}</td>
                                <td>{row.activity_type}</td>
                                <td>{row.distance}</td>
                                <td>{row.elapsed_duration}</td>
                                <td>{row.timer_duration}</td>
                                <td class="activity-result">{row.status}</td>
                            </tr>
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
            <p class="activity-count">
                {move || format!("{} activities", activities.get().len())}
            </p>
        </Show>
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
