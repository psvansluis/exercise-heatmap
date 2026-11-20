# Exercise Explorer — Design and Implementation Plan

**Status:** Architecture baseline for MVP planning  
**Scope:** Static, client-side application for importing and exploring personal exercise history  
**Repository input inspected:** `assets/export_177699303.zip` (contains GPX, FIT, and `.fit.gz` activity files)

## 1. Purpose and principles

Exercise Explorer helps a person explore past walks, runs, and other recorded exercise on a map, then progressively understand which streets and paths they have covered. It is a static website: importing, parsing, analysis, visualization, and persistence of personal activity data happen in the browser. There is no application backend or application database.

The application should support activity history worldwide and hundreds of activities. It should query detailed tracks according to the visible map range, instead of loading and drawing every track whenever the map opens. Long-term street exploration is a central feature, but it must be built on an explicit, measurable street-network coverage model rather than a visual heatmap alone.

### Decision labels used here

- **Confirmed** means the user explicitly selected or stated this in the design conversation.
- **Recommendation** means a concrete implementation direction proposed here, still open to revision.
- **Open question / spike** means implementation should wait for evidence or a product decision.

## 2. Confirmed product and architecture decisions

| Area | Status | Decision |
|---|---|---|
| Initial import | Confirmed | Import a full Strava data-export ZIP in the browser for MVP. |
| Later source | Confirmed direction | Garmin Personal API may be considered after MVP; it is not an MVP dependency. |
| Scale | Confirmed | Plan for hundreds of historical activities. |
| Geographic scope | Confirmed | Support activities everywhere. |
| Map loading | Confirmed | Load/query only activities that occurred in the current map range. |
| MVP activity fields | Confirmed | Coordinates, distance, duration, activity datetime, and activity type. |
| Client-side boundary | Confirmed | No application backend; static hosting serves application files only. User activity data is not sent to that host. |
| Frontend | Confirmed | Leptos. |
| Map choice | Confirmed | OpenStreetMap (OSM) data/ecosystem. |
| Local persistence | Confirmed | IndexedDB. |
| Backup | Confirmed | User-controlled export. |
| Implementation sequence | Confirmed | Deliver the feature groups in the phased order below. |

The conversation also selected Rust/WASM for the application’s business logic and named MapLibre GL JS as the proposed map renderer. It proposed GitHub Pages and Trunk as deployment/build choices. These are carried forward as **recommendations** in this document, rather than treating every implementation detail as a user-confirmed commitment.

## 3. Recommended system architecture

```text
Static host (GitHub Pages candidate)
  HTML/CSS + Leptos CSR + small JS map bridge + Rust/WASM
       |                     |                    |
       | local import        | IndexedDB          | map style/tile requests
       v                     v                    v
  Strava ZIP          browser-local data      configurable OSM-derived provider
                             |
                             +-- optional OSM street-network acquisition/cache
```

### Responsibilities

- **Leptos client-rendered UI (CSR):** import flow, filters, activity list/details, settings, status and errors. Keep application state and domain decisions in Rust.
- **Rust/WASM core:** archive/file classification where compatible, format parsing, normalization, validation, deduplication, bounding boxes, spatial filtering, street matching, and coverage statistics.
- **MapLibre GL JS (recommendation):** WebGL map display, gestures, map layers and hit testing. A narrow `wasm-bindgen` bridge exposes operations such as create map, set viewport, update routes, select feature, and dispose map. MapLibre owns rendering state; Rust owns activity and analysis state.
- **IndexedDB (confirmed):** persistent local records, tracks, derived indexes, street data cache, settings, and schema version. A thin Rust/JS adapter is acceptable if it makes browser transaction behavior clearer; keep storage behind an interface.
- **Static hosting (GitHub Pages candidate):** ships assets only. No server-rendered personal data, analytics payloads containing user data, or upload endpoint.
- **Map/street providers:** external services are separate from the application host. Their requests can reveal IP address and requested geographic area; see Privacy.

Use GeoJSON as the map interchange format, with coordinates ordered `[longitude, latitude]` as required by GeoJSON even though the domain model names latitude and longitude separately. Avoid sending full activity records to JavaScript when only geometry and a stable ID are needed.

### Suggested Rust workspace

Start with a small workspace; split crates only when boundaries provide testability or compilation value. Proposed layout:

```text
Cargo.toml                  # workspace and shared dependency versions
rust-toolchain.toml
Trunk.toml                  # candidate build configuration
crates/
  domain/                   # IDs, types, units, activity and track types
  import/                   # archive inspection, Strava manifest/CSV, GPX/FIT adapters
  geo/                      # bounds, viewport query, geometry and matching
  storage/                  # IndexedDB abstraction, migrations, backup format
  app/                      # Leptos CSR UI, application state, worker orchestration
web/
  index.html
  map_bridge.js             # minimal MapLibre interop
  styles/
```

Avoid premature crate proliferation. The ZIP decoder and format libraries are candidate dependencies only. Rust crates worth evaluating include `serde`, `serde_json`, `wasm-bindgen`, `serde-wasm-bindgen`, `geo`, `rstar`, and `quick-xml`; IndexedDB wrappers and FIT decoders must be selected after a browser/WASM proof of concept. Use a Web Worker for long parsing/analysis tasks where supported and useful, with cancellation/progress messages so a large import does not freeze the UI.

## 4. Canonical data model

The internal schema is provider-independent. Import formats are adapters; they must not dictate persistent identity or map semantics.

```rust
struct Activity {
    id: ActivityId,                  // app-generated stable ID
    source: ActivitySource,          // StravaArchive, future provider, backup
    source_id: Option<String>,       // original provider ID when available
    name: Option<String>,
    activity_type: ActivityType,
    started_at: Option<DateTime<Utc>>,
    elapsed_duration_seconds: Option<f64>, // wall-clock interval, including pauses
    timer_duration_seconds: Option<f64>,   // active timer interval, excluding pauses
    distance_meters: Option<f64>,
    bounds: Option<BoundingBox>,
}

struct Track {
    activity_id: ActivityId,
    points: Vec<TrackPoint>,
}

struct TrackPoint {
    latitude: f64,
    longitude: f64,
    timestamp: Option<DateTime<Utc>>,
}
```

**Confirmed MVP fields** are coordinates, distance, duration, activity datetime and type. The name, source/source ID, bounding box, application ID, and optional per-point timestamp above are **implementation recommendations**: provenance and IDs support deduplication; bounds support viewport queries; timestamps should be preserved when present but are not required for MVP display. The user did not settle whether per-point timestamps are in scope, so do not make timestamp availability a condition of import.

Data rules:

- Store missing/unknown values as absent, never as zero.
- Validate finite coordinate values and legal latitude/longitude ranges; retain source data only if the product later chooses to offer raw-file preservation.
- Normalize timestamps to UTC while retaining enough information to present local time correctly.
- Keep track points in source order. Never infer a timestamp for a point from activity start time without marking it as derived.
- Store distance in metres and both elapsed and timer duration in seconds; elapsed time includes pauses, while timer time excludes pauses. Prefer valid FIT summary values and GPX point-time elapsed estimates when available, with CSV values as fallback. Format values at the UI boundary.
- Use an app-owned ID plus a deduplication key derived from provider ID where available. Fallback matching should combine stable metadata and track characteristics and must avoid silently merging similar activities.
- A track with no valid coordinates may still have useful activity metadata; record an import warning and retain or skip it according to an explicit policy.

## 5. Strava ZIP import

### Recommended import pipeline

1. User selects a local ZIP; show that it is processed in the browser.
2. Inspect archive entry names and sizes before extraction. Ignore unrelated content; reject path traversal, unreasonable expansion ratios, oversized entries, and malformed archives.
3. Locate Strava activity metadata (typically activity CSV/metadata files) and activity files by archive path/extension. The repository sample includes `.gpx`, `.fit`, and `.fit.gz`, so support for compressed FIT is a concrete compatibility requirement to investigate.
4. Parse each supported activity file into the canonical model. Reconcile metadata and track file by provider ID when possible. Do not assume every metadata row has a matching GPS file or that filenames alone are durable IDs.
5. Validate, normalize, compute bounds, and produce per-file warnings. Make import outcomes reviewable before committing: imported, skipped, duplicate, unsupported, malformed.
6. Write a batch in IndexedDB transaction(s) with progress and cancellation. Avoid leaving a half-imported archive if the browser closes; record an import batch or commit atomically where practical.
7. Refresh activity metadata/indexes and query tracks for the current viewport.

Support GPX and FIT as initial parsers because both appear in the sample. Treat `.fit.gz` decompression and FIT parsing in WASM as an explicit spike. TCX is a reasonable later format adapter, not a confirmed sample requirement. Parse Strava CSV/metadata independently from track parsing so missing or evolving archive fields do not break all import paths.

The sample archive is user data and should remain a local fixture; do not add it to public builds, logs, screenshots, or test fixtures committed to a public repository. Create small synthetic and anonymized fixtures for tests.

## 6. IndexedDB, viewport queries, and portable backup

### Suggested object stores

| Store | Key | Contents |
|---|---|---|
| `activities` | `ActivityId` | metadata and derived bounding box |
| `tracks` | `ActivityId` | ordered points, optionally chunked for long tracks |
| `spatial_index` | implementation key | rebuildable spatial index or bbox cells |
| `street_network` | region/tile key | cached OSM-derived network geometry and source/version metadata |
| `settings` | setting key | preferences and schema version |

Keep the index rebuildable from activity/track data. Consider storing a compact grid/H3-like cell index or R-tree only after measuring IndexedDB query cost; the number of activities is in the hundreds, so a simple bounding-box index may be adequate initially.

### Viewport loading behavior

- On startup, read compact activity metadata/indexes, not every point array.
- On settled map movement, convert visible bounds to one or more query boxes. Split boxes crossing the antimeridian.
- Fetch candidate activities by bounding-box intersection, then apply a more precise track/viewport intersection test to reduce false positives from long-route bounding boxes.
- Load only tracks belonging to activities occurring in the visible range. Clip geometry to the viewport when practical; do not confuse viewport loading with date/time filtering.
- Debounce map events; cancel or ignore stale results using a monotonically increasing query token. Render a loading state without blocking map gestures.
- Preserve previous visible results briefly during a new query to avoid flicker, then replace them atomically.

### Portable backup

Use a versioned, documented ZIP export, for example:

```text
exercise-explorer-backup.zip
  manifest.json
  activities.json
  tracks/<activity-id>.json
  settings.json
```

The manifest should identify backup-format version, application version, export time, record counts, and serialization rules. Export should include all locally stored canonical activity/track data and settings selected for backup; it should not include rebuildable spatial indexes, provider credentials, or cached basemap tiles. Restore must validate before writing, show counts/warnings, support duplicate detection and merge, and handle unknown future versions safely. Add round-trip and interrupted-restore tests.

## 7. MapLibre and OpenStreetMap strategy

**Confirmed:** OSM is the chosen map ecosystem. **Recommendation:** use MapLibre GL JS as the renderer and keep its integration behind a small JS bridge so it can be replaced independently.

There are two distinct data products:

1. **Basemap:** cartographic streets, labels, parks and water for display.
2. **Street network:** routable/attributable OSM way geometry used for coverage calculations.

Do not treat visible basemap features as a complete analysis network. Map styles may omit, generalize, or change features at zoom levels.

The public `tile.openstreetmap.org` service is not an unrestricted production tile CDN. It has a usage policy, attribution and caching requirements, and does not permit bulk prefetching; it offers no service-level guarantee. Use it only for a compliant prototype if appropriate, show attribution, obey cache headers, and never hard-code it as an assumed permanent provider. Before public launch, choose a provider whose style/tile terms permit the intended use, or operate a suitable tile source. A configurable style URL/provider boundary is recommended.

Map tile requests disclose approximate viewport geography and ordinary network metadata such as IP address to the selected provider. Activity coordinates should be passed only to local map rendering code, never encoded in tile, style, geocoding, or telemetry requests. Review the chosen provider’s terms, attribution requirements, retention practices, and privacy policy before release.

## 8. Street-network acquisition and coverage methodology

### Acquisition recommendation

Define a `StreetNetworkProvider` boundary that retrieves pedestrian-relevant ways for a requested region. Candidate source: Overpass API, queried for footways, pedestrian streets, paths, steps where appropriate, service roads and alley-like ways with access tags considered. Cache results by geographic tile/region in IndexedDB and store retrieval time, query/source, attribution/license metadata, and a dataset/version fingerprint. Bound query area and size, handle failures gracefully, and never make street coverage dependent on an unbounded global download.

Overpass is shared infrastructure, not a guaranteed high-volume application API. Query CORS behavior from the actual static deployment origin, instance policies/rate limits, timeouts, max-area constraints, and whether a user-operated download/import path is needed. Provide backoff, caching, cancellation, and clear retry messaging. Do not repeatedly query on every pan; fetch only uncovered regions with a deliberate margin.

### Coverage methodology recommendation

1. Filter OSM ways to the relevant walk/run network using tags and access semantics; preserve way/node IDs and tags for deduplication and explanation.
2. Build spatial indexes for both track segments and network segments.
3. Match track segments to nearby candidate street segments using a configurable distance tolerance, GPS accuracy considerations, heading/continuity, and topology where feasible.
4. Avoid declaring a street covered from simple proximity alone. Parallel roads, GPS drift, bridges, private paths, and paths crossing streets create false positives. Require plausible alignment/overlap and expose threshold assumptions.
5. Aggregate covered length / eligible network length, with explicit treatment of duplicates, direction, disconnected segments, access-restricted ways, and missing OSM data.
6. Visualize matched/uncertain/unmatched network separately during development. Show coverage as an estimate, not an objective claim that a person physically traversed every mapped way.

Street coverage can vary with OSM completeness and tagging. A regional score must state its denominator, network filters, date/version, and matching tolerance. “Unexplored” means not matched by imported tracks in the selected dataset, not necessarily never walked.

## 9. Feature roadmap and acceptance checkpoints

Implement in this order, preserving usable increments.

### Phase 0 — Foundation and first GitHub Pages release

Set up the Cargo workspace, Leptos client-side rendering (CSR), WASM build, MapLibre bridge, and a responsive landing page. Add a GitHub Actions workflow that builds the static single-page application, packages the generated site as a Pages artifact, and deploys it to GitHub Pages on pushes to the default branch; pull requests should run the build without deploying. Configure the repository Pages source to GitHub Actions and grant the workflow only the permissions needed to publish. Use Trunk as the proposed Rust/WASM build tool, subject to confirming the repository setup.

**Phase 0 is complete when the landing page is publicly visible at the repository’s GitHub Pages URL after a successful Actions deployment.** The deployed output must be static and require no application backend. Confirm the Pages project-site base path, asset/WASM URLs, and SPA navigation behavior so the page also works when the repository is hosted below the account root. The map bridge and map rendering can be added in this phase, but are not prerequisites for the landing-page deployment milestone.

### Phase 1 — Activity explorer and local data

Implement Strava ZIP selection/inspection, metadata and GPX/FIT parsing, normalization, deduplication, IndexedDB persistence, backup export/restore, filters, activity selection, map routes, and viewport-based queries. **Checkpoint:** hundreds of sample-like activities import without duplicate re-import; refresh retains them; only relevant track geometry is fetched/rendered for the viewport; export can restore data.

### Phase 2 — Street coverage

Prove street-data acquisition and caching; implement network filters, segment matching, coverage estimate, and map overlay. **Checkpoint:** test regions include known parallel streets, GPS jitter, crossings, and incomplete OSM data; the UI communicates estimate assumptions.

### Phase 3 — Exploration tools

Identify candidate uncovered streets/areas and add sorting/filtering or route suggestions only after coverage quality is measured. **Checkpoint:** suggestions explain why a segment is considered uncovered and do not imply turn-by-turn routing unless a routing source is deliberately added.

### Later candidates (not MVP commitments)

Temporal summaries, richer route analysis, GPX/TCX imports, Garmin Personal API, offline basemap support, account/multi-user features, and route generation. Garmin is explicitly deferred until after the Strava ZIP MVP.

## 10. Performance and reliability

- Keep initial download small; lazy-load map/style and expensive parsing code where practical.
- Parse/import in a worker when practical, stream entries where the ZIP library permits, and report progress, warnings, and cancellation.
- Avoid copying full tracks repeatedly across Rust/WASM/JS boundaries; serialize only visible GeoJSON and batch map updates.
- Cluster or aggregate display at low zoom rather than creating one map feature per dense segment if profiling shows the need.
- Debounce and cancel stale viewport work; cap concurrent IndexedDB reads and network fetches.
- Use bounded, indexed street caches with clear eviction behavior; never evict canonical activity data automatically.
- Measure startup time, import throughput, peak memory, viewport query latency, and frame responsiveness with synthetic large archives as well as the actual scale target of hundreds.
- Treat browser storage as quota-limited and user-clearable. Surface quota/write errors and offer an export reminder after successful imports.

## 11. Testing strategy

Test the domain and algorithms natively where possible, then test browser-specific integrations in a real browser.

- Unit tests: normalization, unit conversion, coordinate validation, timestamps, bounds, antimeridian handling, deduplication, and backup versioning.
- Parser tests: minimal GPX/FIT/CSV fixtures, malformed and truncated entries, `.fit.gz`, missing metadata, unsupported files, and compressed-size limits.
- Storage tests: IndexedDB migrations, transaction rollback/failure, duplicate merge, export/restore round-trip, and quota errors.
- Geospatial tests: viewport candidate queries versus brute-force results; crossing bounds; long-route false positives; clipping; tolerance behavior; parallel street and crossing cases.
- Browser end-to-end checks: import, refresh persistence, route selection, map pan/zoom query refresh, backup download/restore, offline/error states, and no activity payload to app host.
- Privacy regression: inspect network requests and assert activity coordinates are absent from requests except the local renderer boundary; verify no analytics or error reporting captures personal records.
- Use synthetic/anonymized data in CI. Never publish the supplied real export as a fixture.

## 12. Privacy and security

The confirmed privacy boundary is “no personal activity data reaches the static application backend.” Since there is no application backend, static hosting receives asset requests only. This does not mean every external network interaction is private: map style/tile and optional Overpass providers can see IP address and requested geographic area.

- Keep parsing, activity storage, filtering, and analysis in the browser.
- Minimize third-party scripts and dependencies; pin and audit build dependencies.
- Never send raw ZIP contents, route geometry, home/work locations, or activity identifiers to telemetry or tile endpoints.
- Avoid logging coordinates, archive filenames, parser payloads, or personal activity names. Error messages should be actionable but data-minimal.
- Use HTTPS; apply a restrictive Content Security Policy and explicit `connect-src` allowlist for selected map/network providers.
- Treat imported ZIPs as untrusted input: defend against path traversal, decompression bombs, huge entries, malformed encodings, parser panics, and memory exhaustion; set limits and isolate work.
- IndexedDB data is available to scripts running on the same origin and can be removed by browser/user actions. A public static site’s shipped JavaScript is inspectable. Keep future credentials out of persistent storage by default; Garmin is not MVP.
- Make export and clear-local-data controls obvious. Document that backup files contain sensitive location history and should be stored carefully.
- Consider optional home-location masking as a later product decision; it was not confirmed and must not silently alter imported coordinates.

## 13. Open technical spikes and product questions

| Question | Why it matters | Spike / decision gate |
|---|---|---|
| FIT parser compatibility with `wasm32-unknown-unknown` | Candidate crates may use unsupported APIs or inflate WASM. | Compile and parse a small FIT fixture in browser WASM; inspect license, bundle size, error handling, and `.fit.gz` decompression path. Keep parser behind adapter. |
| Overpass CORS and rate limits | Browser fetch may be blocked or constrained, and shared instances may throttle. | Test from actual static origin against candidate instances; record CORS, query limits, timeouts, terms, cache policy. Decide whether optional user-supplied extract import is needed. |
| Trackpoint timestamps | MVP says activity datetime but not explicitly per-point timestamps. | Preserve timestamps when available, but allow absent values. Confirm whether pace/time-colored route visualization is a near-term requirement before expanding UI or requiring timestamps. |
| Strava export shape variation | CSV columns and file naming can vary; archives may contain compressed files. | Inspect archive metadata formats with anonymized samples; make parsers tolerant and version-independent. |
| IndexedDB Rust interface | Wrapper ergonomics, transaction semantics, and WASM support differ. | Prototype atomic batch import and backup with candidate wrapper vs a tiny JS adapter; select the thinnest maintainable boundary. |
| OSM map tile/style provider | Public OSM tile server has policy limits; vector providers have different cost/privacy terms. | Select before public launch based on attribution, terms, privacy, reliability, and expected traffic. |
| Coverage tolerance and eligible network | GPS precision and OSM tagging change the score. | Build a small labeled evaluation area; tune thresholds and report uncertainty before presenting a percentage. |
| GitHub Pages SPA path and build configuration | GitHub Pages and a GitHub Actions deployment pipeline are confirmed; project sites are hosted below the account root and Rust/WASM output needs correct asset paths and MIME handling. | In Phase 0, build and deploy a landing-page SPA through Actions; verify the repository base path, generated asset/WASM URLs, direct navigation behavior, and Pages configuration. Keep Trunk as the proposed build tool until the repository build proves it fits. |

Garmin Personal API remains a future investigation. Direct browser integration may encounter CORS, credential, authentication, and provider-policy constraints; no design should assume that a token alone makes it safe or possible. Reassess only after MVP and verify current official provider requirements at that time.

## 14. ADR-style decisions

### ADR-001 — Client-only application

- **Status:** Accepted (confirmed)
- **Context:** Personal routes are sensitive; user wants a static website and client-side processing.
- **Decision:** Serve static assets only; process and persist activity data in the browser. No application server or database.
- **Consequences:** Simple deployment and a strong application-host data boundary; map/network providers still receive ordinary requests and approximate areas.

### ADR-002 — Leptos frontend with Rust/WASM domain logic

- **Status:** Leptos confirmed; exact UI/WASM split recommended
- **Context:** User is curious about a Rust-based frontend and selected Leptos.
- **Decision:** Use Leptos CSR and Rust for domain logic. Keep browser APIs and third-party map integration behind narrow adapters.
- **Consequences:** More logic can be shared/tested in Rust; some JS interop remains necessary, especially for MapLibre and potentially IndexedDB.

### ADR-003 — Strava ZIP as MVP ingestion

- **Status:** Accepted (confirmed)
- **Context:** Strava offers a full ZIP export and direct API integration would add backend/auth complexity.
- **Decision:** Import the archive locally in the browser; do not make live Strava or Garmin API access an MVP dependency.
- **Consequences:** User initiates updates; importer must tolerate archive differences and supported file formats.

### ADR-004 — Provider-independent canonical activity model

- **Status:** Recommended
- **Context:** Multiple file formats and a possible future Garmin source should feed one analysis engine.
- **Decision:** Normalize provider records into stable app-owned activity/track entities; preserve source provenance and optional source ID.
- **Consequences:** Adapters and deduplication require design, but future sources do not dictate storage schema.

### ADR-005 — IndexedDB and versioned export backup

- **Status:** Accepted storage/backup choices; format recommended
- **Context:** Data should persist locally and remain portable without a server.
- **Decision:** Store canonical records in IndexedDB and provide a versioned ZIP export/restore; rebuild indexes after restore.
- **Consequences:** Browser quota and migration handling matter; backups themselves contain sensitive data.

### ADR-006 — OSM ecosystem, MapLibre candidate, separate network analysis data

- **Status:** OSM confirmed; MapLibre and acquisition approach recommended
- **Context:** The product needs a map display and a distinct road graph for coverage analysis.
- **Decision:** Keep the renderer/provider configurable; use separate sources and policies for basemap tiles and street-network geometry.
- **Consequences:** Attribution and provider terms are part of product behavior; coverage depends on OSM completeness and matching quality.

### ADR-007 — Viewport-scoped track retrieval

- **Status:** Accepted (confirmed)
- **Context:** Worldwide data should remain responsive and map-range aware.
- **Decision:** Query local metadata/indexes by current bounds and retrieve detailed tracks only for intersecting activities, with precise filtering after bbox candidate selection.
- **Consequences:** Requires index maintenance, antimeridian handling, cancellation, and careful distinction between candidate and true intersection.

## 15. MVP exit criteria

The MVP is ready for broader use when a user can import the supplied archive types supported by the chosen parser path, review warnings, store hundreds of activities locally, see routes for the visible map range, filter/select activities, export a portable backup, restore it without data loss, and confirm by network inspection that no activity data reaches the application host or external map requests. Street coverage is a later phase and must be labeled approximate until validated against known cases.

