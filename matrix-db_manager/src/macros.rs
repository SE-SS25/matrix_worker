/// Return error if db is down
///
/// Get `db_pool` otherwise
macro_rules! backoff {
    ($self:expr) => {{
        use crate::guard::DbGuard;
        #[allow(unused_imports)]
        use anyhow::{anyhow, bail};
        use core::sync::atomic::Ordering;
        use matrix_errors::DbErr;

        if DbGuard::is_running(Ordering::SeqCst) {
            bail!(DbErr::Unreachable(anyhow!("Unreachable")));
        }
        &$self.db_pool
    }};
}

// Shoutout to the server
macro_rules! hans {
    ($self:expr, $e:expr) => {{
        use crate::guard::DbGuard;
        #[allow(unused_imports)]
        use anyhow::{anyhow, bail};
        use matrix_errors::DbErr;

        DbGuard::init(&$self.db_pool);
        DbErr::Unreachable($e)
    }};
}
