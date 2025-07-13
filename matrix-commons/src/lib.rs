use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const DEFAULT_BACKOFF: u64 = 500;
pub const MAX_BACKOFF: u64 = 5 * 60 * 1_000; // Max 5 mins

/// Returning the new maximum sleep time and a random sample from the range
pub fn jitter(current: u64) -> (u64, Duration) {
    let current_millis = current.max(DEFAULT_BACKOFF);
    let new_millis = (current_millis * 2).min(MAX_BACKOFF);

    let backoff_millis = rand::random_range(DEFAULT_BACKOFF..=new_millis);

    (new_millis, Duration::from_millis(backoff_millis))
}
