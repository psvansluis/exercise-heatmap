use exercise_domain::ActivityEntry;
use exercise_import::{
    ImportEntryOutcome, ImportEvent, ParseOptions, ZipImportStream, parse_activity_file,
};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Event, File, HtmlInputElement};

#[derive(Clone, Debug, PartialEq)]
pub enum ActivityUpdate {
    Clear,
    Append(ActivityEntry),
}

const MAX_UPLOAD_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone)]
struct ImportUi {
    is_importing: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    selected_filename: RwSignal<Option<String>>,
    progress: RwSignal<Option<String>>,
    on_activity_update: Callback<ActivityUpdate>,
}

#[component]
pub fn ImportPanel(on_activity_update: Callback<ActivityUpdate>) -> impl IntoView {
    let ui = ImportUi {
        is_importing: RwSignal::new(false),
        error: RwSignal::new(None::<String>),
        selected_filename: RwSignal::new(None::<String>),
        progress: RwSignal::new(None::<String>),
        on_activity_update,
    };

    let is_importing = ui.is_importing;
    let error = ui.error;
    let selected_filename = ui.selected_filename;
    let progress = ui.progress;

    view! {
        <section class="import-panel" aria-labelledby="import-title">
            <div class="import-heading">
                <div>
                    <p class="eyebrow">"YOUR LOCAL DATA"</p>
                    <h2 id="import-title">"Import activity files"</h2>
                    <p class="import-copy">
                        "Choose GPX, FIT, FIT.GZ, or ZIP files. Each ZIP can include an activities.csv for exact path matching. Files are processed in this browser."
                    </p>
                </div>
                <label class="file-picker">
                    <span>{move || if is_importing.get() { "Processing files…" } else { "Choose files" }}</span>
                    <input
                        type="file"
                        multiple
                        accept=".gpx,.fit,.fit.gz,.zip,application/zip"
                        aria-label="Choose activity files or ZIP archives"
                        on:change=move |event| handle_file_change(event, ui.clone())
                        disabled=move || is_importing.get()
                    />
                </label>
            </div>

            <div class="import-status" aria-live="polite">
                {move || {
                    if is_importing.get() {
                        progress.get().unwrap_or_else(|| "Processing selected files…".into())
                    } else if let Some(message) = error.get() {
                        message
                    } else if let Some(filename) = selected_filename.get() {
                        format!("Finished processing {filename}.")
                    } else {
                        "Your activity details remain on this device.".to_string()
                    }
                }}
            </div>
        </section>
    }
}

fn handle_file_change(event: Event, ui: ImportUi) {
    let Some(files) = selected_files(event) else {
        return;
    };
    if files.is_empty() {
        return;
    }

    start_import(&files, &ui);
    leptos::task::spawn_local(process_files(files, ui));
}

fn selected_files(event: Event) -> Option<Vec<File>> {
    let input = event
        .target()
        .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())?;
    let files = input.files()?;
    let selected = (0..files.length())
        .filter_map(|index| files.get(index))
        .collect();
    input.set_value("");
    Some(selected)
}

fn start_import(files: &[File], ui: &ImportUi) {
    ui.selected_filename.set(Some(if files.len() == 1 {
        files[0].name()
    } else {
        format!("{} files", files.len())
    }));
    ui.error.set(None);
    ui.progress.set(Some("Preparing selected files…".into()));
    ui.on_activity_update.run(ActivityUpdate::Clear);
    ui.is_importing.set(true);
}

async fn process_files(files: Vec<File>, ui: ImportUi) {
    let options = ParseOptions::default();
    let total_files = files.len();

    for (index, file) in files.into_iter().enumerate() {
        process_file(file, &options, &ui).await;
        ui.progress.set(Some(format!(
            "Finished file {} of {total_files}",
            index + 1
        )));
        yield_to_browser().await;
    }

    ui.is_importing.set(false);
    ui.progress.set(None);
}

async fn process_file(file: File, options: &ParseOptions, ui: &ImportUi) {
    let file_name = file.name();
    if file.size() > MAX_UPLOAD_BYTES as f64 {
        ui.error.set(Some(format!(
            "{file_name} exceeds the 256 MB import limit."
        )));
        return;
    }

    ui.progress.set(Some(format!("Reading {file_name}…")));
    match read_file_bytes(&file).await {
        Ok(bytes) if file_name.to_lowercase().ends_with(".zip") => {
            process_zip(&file_name, &bytes, options, ui).await;
        }
        Ok(bytes) => {
            let outcome = parse_activity_file(&file_name, &bytes, options);
            emit_activity(outcome, ui);
            yield_to_browser().await;
        }
        Err(message) => ui.error.set(Some(message)),
    }
}

async fn read_file_bytes(file: &File) -> Result<Vec<u8>, String> {
    JsFuture::from(file.array_buffer())
        .await
        .map(|buffer| js_sys::Uint8Array::new(&buffer).to_vec())
        .map_err(|_| format!("{} could not be read.", file.name()))
}

async fn process_zip(file_name: &str, bytes: &[u8], options: &ParseOptions, ui: &ImportUi) {
    match ZipImportStream::new(bytes, options.clone()) {
        Ok(mut stream) => process_zip_events(file_name, &mut stream, ui).await,
        Err(parse_error) => ui.error.set(Some(parse_error.to_string())),
    }
}

async fn process_zip_events(file_name: &str, stream: &mut ZipImportStream<'_>, ui: &ImportUi) {
    loop {
        let event = match stream.next_event() {
            Ok(Some(event)) => event,
            Ok(None) => break,
            Err(parse_error) => {
                ui.error.set(Some(parse_error.to_string()));
                break;
            }
        };

        match event {
            ImportEvent::Entry(outcome) => {
                emit_activity(outcome, ui);
                yield_to_browser().await;
            }
            ImportEvent::Progress { completed, total } => {
                ui.progress.set(Some(format!(
                    "Processed {completed} of {total} activity files in {file_name}"
                )));
                yield_to_browser().await;
            }
            ImportEvent::Warning(warning) => {
                ui.progress.set(Some(format!(
                    "{} · {}",
                    warning.path.unwrap_or_else(|| file_name.to_string()),
                    warning.message
                )));
                yield_to_browser().await;
            }
            ImportEvent::Complete => break,
        }
    }
}

fn emit_activity(outcome: ImportEntryOutcome, ui: &ImportUi) {
    match outcome {
        ImportEntryOutcome::Reconciled { activity, track }
        | ImportEntryOutcome::TrackOnly { activity, track } => {
            ui.on_activity_update.run(ActivityUpdate::Append(ActivityEntry::new(
                activity,
                Some(track),
            )));
        }
        ImportEntryOutcome::MetadataOnly { activity, .. } => {
            ui.on_activity_update.run(ActivityUpdate::Append(ActivityEntry::new(
                activity,
                None,
            )));
        }
        ImportEntryOutcome::Duplicate { path } => {
            ui.error.set(Some(format!("Duplicate activity at {path}.")));
        }
        ImportEntryOutcome::Malformed { path, reason } => {
            ui.error.set(Some(format!("{path}: {reason}")));
        }
    }
}

async fn yield_to_browser() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let Some(window) = web_sys::window() else {
            let _ = resolve.call0(&wasm_bindgen::JsValue::NULL);
            return;
        };
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0);
    });
    let _ = JsFuture::from(promise).await;
}
