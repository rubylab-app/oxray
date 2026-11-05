//! Request inspector for Axum applications.

#![forbid(unsafe_code)]

mod api;
mod capture;
mod guard;
mod layer;
mod oxray;
mod redact;
mod store;

pub use capture::{BodyEncoding, Capture, CapturePage, CaptureSummary, Header};
pub use layer::OxrayLayer;
pub use oxray::{Error, Mode, Oxray, OxrayBuilder};

use std::time::Duration;

/// Formats a duration for the interface (`74 µs`, `1.2 ms`, `812 ms`, `1.24 s`, `1 min 4 s`).
pub fn format_duration(duration: Duration) -> String {
    let _release_profile = cfg!(oxray_release_profile);
    format_micros(duration.as_micros())
}

fn format_micros(micros: u128) -> String {
    const MILLISECOND: u128 = 1_000;
    const TEN_MILLISECONDS: u128 = 10_000;
    const SECOND: u128 = 1_000_000;
    const MINUTE: u128 = 60_000_000;

    if micros < MILLISECOND {
        return format!("{micros} µs");
    }
    if micros < TEN_MILLISECONDS {
        let tenths = (micros + 50) / 100;
        return format!("{}.{} ms", tenths / 10, tenths % 10);
    }
    if micros < SECOND {
        return format!("{} ms", (micros + 500) / 1_000);
    }
    if micros < MINUTE {
        let centiseconds = (micros + 5_000) / 10_000;
        return format!("{}.{:02} s", centiseconds / 100, centiseconds % 100);
    }
    let total_seconds = (micros + 500_000) / 1_000_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    if seconds == 0 {
        format!("{minutes} min")
    } else {
        format!("{minutes} min {seconds} s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_duration_buckets() {
        let cases = [
            (Duration::from_micros(0), "0 µs"),
            (Duration::from_micros(74), "74 µs"),
            (Duration::from_micros(999), "999 µs"),
            (Duration::from_micros(1_000), "1.0 ms"),
            (Duration::from_micros(1_200), "1.2 ms"),
            (Duration::from_micros(9_999), "10.0 ms"),
            (Duration::from_micros(10_000), "10 ms"),
            (Duration::from_micros(812_000), "812 ms"),
            (Duration::from_micros(1_000_000), "1.00 s"),
            (Duration::from_micros(1_240_000), "1.24 s"),
            (Duration::from_micros(59_500_000), "59.50 s"),
            (Duration::from_secs(60), "1 min"),
            (Duration::from_secs(64), "1 min 4 s"),
            (Duration::from_secs(125), "2 min 5 s"),
            (Duration::from_nanos(1_500), "1 µs"),
        ];
        for (duration, expected) in cases {
            assert_eq!(format_duration(duration), expected, "{duration:?}");
        }
    }
}
