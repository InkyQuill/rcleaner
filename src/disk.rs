//! Native disk queries; no platform-specific `df` output parsing.
use crate::i18n::{message, text};
use std::path::Path;

pub fn stat(path: &Path) -> String {
    match (fs2::total_space(path), fs2::available_space(path)) {
        (Ok(total), Ok(available)) if total > 0 => message(
            "disk",
            &[
                &((total.saturating_sub(available)) as f64 * 100.0 / total as f64).round(),
                &format!("{:.2}", available as f64 / 1_073_741_824.0),
            ],
        ),
        _ => text("unknown").into(),
    }
}
