use exercise_domain::{Activity, Track};

/// A per-entry outcome suitable for previewing an import before persistence.
#[derive(Clone, Debug, PartialEq)]
pub enum ImportEntryOutcome {
    Imported {
        activity: Activity,
        track: Option<Track>,
    },
    Skipped {
        path: String,
        reason: String,
    },
    Duplicate {
        path: String,
    },
    Unsupported {
        path: String,
        format: String,
    },
    Malformed {
        path: String,
        reason: String,
    },
}

/// A non-fatal issue found while inspecting or parsing an archive entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportWarning {
    pub path: Option<String>,
    pub message: String,
}
