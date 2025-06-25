/// Return error if db is down
///
/// Get `db_pool` otherwise
macro_rules! backoff {
    ($self:expr) => {{
        use crate::guard::GUARD_RUNNING;
        #[allow(unused_imports)]
        use anyhow::{anyhow, bail};
        use core::sync::atomic::Ordering;
        use matrix_errors::DbErr;

        if GUARD_RUNNING.load(Ordering::SeqCst) {
            bail!(DbErr::Unreachable(anyhow!("Unreachable")));
        }
        &$self.db_pool
    }};
}

// TODO Better name
macro_rules! db_fail {
    ($self:expr, $e:expr) => {{
        use crate::guard::DbGuard;
        #[allow(unused_imports)]
        use anyhow::{anyhow, bail};
        use matrix_errors::DbErr;

        // TODO What if backoff thread determines that db is up again
        // TODO And then this write comes through?
        // TODO Solution, start backoff thread below
        DbGuard::init(&$self.db_pool);
        DbErr::Unreachable($e)
    }};
}
