/// Return error if db is down
///
/// Get `db_pool` otherwise
macro_rules! backoff {
    ($self:expr) => {{
        #[allow(unused_imports)]
        use anyhow::{anyhow, bail};
        use matrix_errors::DbErr;

        if !*$self.up.read() {
            bail!(DbErr::Unreachable(anyhow!("Unreachable")));
        }
        &$self.db_pool
    }};
}

// TODO Better name
macro_rules! db_fail {
    ($self:expr, $e:expr) => {{
        #[allow(unused_imports)]
        use anyhow::{anyhow, bail};
        use matrix_errors::DbErr;

        // TODO Start backoff thread

        // TODO What if backoff thread determines that db is up again
        // TODO And then this write comes through?
        // TODO Solution, start backoff thread below
        *$self.up.write() = false;
        DbErr::Unreachable($e)
    }};
}
