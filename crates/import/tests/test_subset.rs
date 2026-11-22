use exercise_domain::ActivityType;
use exercise_import::{ImportEntryOutcome, parse_strava_zip};

#[test]
fn imports_the_sanitized_test_subset() {
    let archive_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets/test-subset.zip");
    let bytes = std::fs::read(archive_path)
        .expect("crates/import/tests/assets/test-subset.zip must be present");
    let report = parse_strava_zip(&bytes).expect("test-subset.zip should parse");

    assert_eq!(report.entries.len(), 6, "expected six activity rows");
    let mut runs = 0;
    let mut walks = 0;
    let mut rides = 0;
    let mut swims = 0;
    let mut timer_durations = 0;

    for entry in &report.entries {
        let ImportEntryOutcome::Reconciled { activity, track } = entry else {
            panic!("expected every sample activity to have a usable track: {entry:?}");
        };
        assert!(
            !track.points.is_empty(),
            "each imported track should have points"
        );
        assert!(
            activity.started_at.is_some(),
            "activity timestamp should be inferred"
        );
        assert!(
            activity.distance_meters.is_some(),
            "activity distance should be present"
        );
        assert!(
            activity.elapsed_duration_seconds.is_some(),
            "elapsed duration should be present"
        );
        if activity.timer_duration_seconds.is_some() {
            timer_durations += 1;
        }
        if let (Some(elapsed), Some(timer)) = (
            activity.elapsed_duration_seconds,
            activity.timer_duration_seconds,
        ) {
            assert!(
                timer <= elapsed,
                "timer time should not exceed elapsed time"
            );
        }

        match activity.activity_type {
            ActivityType::Run => runs += 1,
            ActivityType::Walk => walks += 1,
            ActivityType::Ride => rides += 1,
            ActivityType::Swim => swims += 1,
            ref other => panic!("unexpected sample activity type: {other:?}"),
        }
    }

    assert_eq!((runs, walks, rides, swims), (2, 2, 1, 1));
    assert_eq!(
        timer_durations, 5,
        "the five FIT activities should provide timer time"
    );
}
