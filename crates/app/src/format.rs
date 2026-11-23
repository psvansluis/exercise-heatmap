//! Formatting helpers and display extension traits for UI presentation.

/// Standard em dash used across the application to represent absent or empty data.
pub const EMPTY_DASH: &str = "—";

/// Extension trait for `Option<T>` providing ergonomic formatting fallbacks for UI presentation.
pub trait OptionDisplayExt<T> {
    /// Format an optional value with a mapping function, defaulting to "—" if None or if the result is empty.
    fn format_or_dash(self, f: impl FnOnce(T) -> String) -> String;

    /// Unwrap an optional string/string slice, defaulting to "—" if None or empty.
    fn or_dash(self) -> String
    where
        T: AsRef<str>;
}

impl<T> OptionDisplayExt<T> for Option<T> {
    fn format_or_dash(self, f: impl FnOnce(T) -> String) -> String {
        match self.map(f) {
            Some(s) if !s.trim().is_empty() => s,
            _ => EMPTY_DASH.to_string(),
        }
    }

    fn or_dash(self) -> String
    where
        T: AsRef<str>,
    {
        match self {
            Some(s) if !s.as_ref().trim().is_empty() => s.as_ref().to_string(),
            _ => EMPTY_DASH.to_string(),
        }
    }
}

/// Format distance in meters as kilometers or meters.
pub fn format_distance(meters: f64) -> String {
    if meters >= 1000.0 {
        format!("{:.2} km", meters / 1000.0)
    } else {
        format!("{meters:.0} m")
    }
}

/// Format duration in seconds as HH:MM:SS or MM:SS.
pub fn format_duration(seconds: f64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn or_dash_handles_none_empty_and_whitespace() {
        let none: Option<String> = None;
        assert_eq!(none.or_dash(), "—");

        let empty: Option<&str> = Some("");
        assert_eq!(empty.or_dash(), "—");

        let whitespace: Option<&str> = Some("   ");
        assert_eq!(whitespace.or_dash(), "—");

        let valid: Option<&str> = Some("Morning Run");
        assert_eq!(valid.or_dash(), "Morning Run");
    }

    #[test]
    fn format_or_dash_handles_none_and_empty_results() {
        let none: Option<f64> = None;
        assert_eq!(none.format_or_dash(format_distance), "—");

        let some: Option<f64> = Some(5000.0);
        assert_eq!(some.format_or_dash(format_distance), "5.00 km");

        let empty_output: Option<i32> = Some(42);
        assert_eq!(empty_output.format_or_dash(|_| "".into()), "—");
        assert_eq!(empty_output.format_or_dash(|_| "   ".into()), "—");
    }

    #[test]
    fn formats_distances_correctly() {
        assert_eq!(format_distance(500.0), "500 m");
        assert_eq!(format_distance(1500.0), "1.50 km");
    }

    #[test]
    fn formats_durations_correctly() {
        assert_eq!(format_duration(65.0), "1:05");
        assert_eq!(format_duration(3665.0), "1:01:05");
    }
}
