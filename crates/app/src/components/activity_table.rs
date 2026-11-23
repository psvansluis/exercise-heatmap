use crate::format::{OptionDisplayExt, format_distance, format_duration};
use exercise_domain::{Activity, ActivityEntry, ActivityId, ActivityType};
use indexmap::IndexMap;
use leptos::prelude::*;

struct ActivityTableRow {
    id: ActivityId,
    name: String,
    timestamp: String,
    activity_type: String,
    distance: String,
    elapsed_duration: String,
    timer_duration: String,
    status: String,
    include_on_map: bool,
    can_include_on_map: bool,
}

impl From<&ActivityEntry> for ActivityTableRow {
    fn from(entry: &ActivityEntry) -> Self {
        let can_include = entry.can_include_on_map();
        let track_status = if can_include {
            "Track available"
        } else {
            "Metadata only"
        };
        Self {
            id: entry.activity.id.clone(),
            name: entry.activity.name.as_deref().or_dash(),
            timestamp: entry.activity.started_at.format_or_dash(|t| t.to_rfc3339()),
            activity_type: display_activity_type(&entry.activity.activity_type),
            distance: entry
                .activity
                .distance_meters
                .format_or_dash(format_distance),
            elapsed_duration: entry
                .activity
                .elapsed_duration_seconds
                .format_or_dash(format_duration),
            timer_duration: entry
                .activity
                .timer_duration_seconds
                .format_or_dash(format_duration),
            status: track_status.into(),
            include_on_map: entry.include_on_map,
            can_include_on_map: can_include,
        }
    }
}

impl From<ActivityEntry> for ActivityTableRow {
    fn from(entry: ActivityEntry) -> Self {
        Self::from(&entry)
    }
}

impl From<&Activity> for ActivityTableRow {
    fn from(activity: &Activity) -> Self {
        Self::from(&ActivityEntry::new(activity.clone(), None))
    }
}

impl From<Activity> for ActivityTableRow {
    fn from(activity: Activity) -> Self {
        Self::from(&activity)
    }
}

#[component]
pub fn ActivityTable(activities: RwSignal<IndexMap<ActivityId, ActivityEntry>>) -> impl IntoView {
    let toggle_activity = move |id: ActivityId| {
        activities.update(|items| {
            if let Some(entry) = items.get_mut(&id) {
                if entry.can_include_on_map() {
                    entry.include_on_map = !entry.include_on_map;
                }
            }
        });
    };

    view! {
        <Show when=move || !activities.get().is_empty()>
            <div class="activity-table-scroll" role="region" aria-label="Imported activity results" tabindex="0">
                <table class="activity-table">
                    <thead>
                        <tr>
                            <th scope="col" class="activity-select-col">"Map"</th>
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
                        {move || {
                            let map = activities.get();
                            map.values().map(ActivityTableRow::from).map(|row| {
                                let id = row.id.clone();
                                let aria_label = format!("Include {} on map", row.name);
                                view! {
                                    <tr>
                                        <td class="activity-select-cell">
                                            <input
                                                type="checkbox"
                                                class="activity-map-checkbox"
                                                prop:checked=row.include_on_map
                                                disabled=!row.can_include_on_map
                                                aria-label=aria_label
                                                on:change=move |_| toggle_activity(id.clone())
                                            />
                                        </td>
                                        <td>{row.name}</td>
                                        <td>{row.timestamp}</td>
                                        <td>{row.activity_type}</td>
                                        <td>{row.distance}</td>
                                        <td>{row.elapsed_duration}</td>
                                        <td>{row.timer_duration}</td>
                                        <td class="activity-result">{row.status}</td>
                                    </tr>
                                }
                            }).collect_view()
                        }}
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
