//! The age in the tail of a row.

/// The age of something that started at `since_ms`, at `now_ms`, in the
/// short form of a tail: `now`, `5m`, `3h`, `2d`. Milliseconds after the
/// Unix epoch.
#[must_use]
pub fn format_age(since_ms: u64, now_ms: u64) -> String {
    let minutes = now_ms.saturating_sub(since_ms) / 60_000;
    match minutes {
        0 => "now".to_owned(),
        1..60 => format!("{minutes}m"),
        60..1440 => format!("{}h", minutes / 60),
        _ => format!("{}d", minutes / 1440),
    }
}
