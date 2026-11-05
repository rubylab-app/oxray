//! Exercises the public duration formatter.

use std::time::Duration;

#[test]
fn published_examples_match_the_public_formatter() {
    assert_eq!(oxray::format_duration(Duration::from_micros(74)), "74 µs");
    assert_eq!(
        oxray::format_duration(Duration::from_micros(1_200)),
        "1.2 ms"
    );
    assert_eq!(
        oxray::format_duration(Duration::from_micros(812_000)),
        "812 ms"
    );
    assert_eq!(
        oxray::format_duration(Duration::from_micros(1_240_000)),
        "1.24 s"
    );
    assert_eq!(oxray::format_duration(Duration::from_secs(64)), "1 min 4 s");
}
