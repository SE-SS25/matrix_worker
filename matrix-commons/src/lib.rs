use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const DEFAULT_BACKOFF: Duration = Duration::from_millis(500);
pub const MAX_BACKOFF: u128 = 5 * 60 * 1_000; // Max 5 mins

pub fn jitter(current: Duration) -> Duration {
    let current_millis = current.as_millis();
    if current_millis >= MAX_BACKOFF {
        return Duration::from_millis(MAX_BACKOFF as u64);
    }

    let backoff_millis =
        rand::random_range(current_millis..MAX_BACKOFF.min(current_millis * 2)) as u64;

    Duration::from_millis(backoff_millis)
}
