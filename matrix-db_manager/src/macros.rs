/// Return error if db is down
///
/// Get `db_pool` otherwise
macro_rules! backoff {
    ($manager:expr) => {{
        use crate::guard::DbGuard;
        #[allow(unused_imports)]
        use anyhow::{anyhow, bail};
        use core::sync::atomic::Ordering;
        use matrix_errors::DbErr;

        if DbGuard::is_running(Ordering::SeqCst) {
            bail!(DbErr::Unreachable(anyhow!("Unreachable")));
        }
        &$manager.db_pool
    }};
}

// Shoutout to the server
macro_rules! hans {
    ($manager:expr, $e:expr) => {{
        use crate::guard::DbGuard;
        #[allow(unused_imports)]
        use matrix_errors::DbErr;

        DbGuard::init(&$manager.db_pool);
        DbErr::Unreachable($e)
    }};
}
