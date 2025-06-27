macro_rules! backoff {
    ($self:expr) => {{
        #[allow(unused_imports)]
        use anyhow::{anyhow, bail};
        use core::sync::atomic::Ordering;
        use matrix_errors::MongoErr;

        // TODO Write in err db if err
        if $self.guard_running.load(Ordering::SeqCst) {
            bail!(MongoErr::Unreachable(anyhow!("Unreachable")));
        }
        let Some(client) = &$self.client else {
            bail!(MongoErr::InvalidUrl($self.id.to_string()));
        };

        client
    }};
}

macro_rules! fritz {
    ($self:expr, $e:expr) => {{
        use crate::guard::MongoGuard;
        #[allow(unused_imports)]
        use matrix_errors::MongoErr;

        let Some(client) = &$self.client else {
            return MongoErr::InvalidUrl($self.id.to_string());
        };

        if let Some(tx) = MongoGuard::init(client, &$self.guard_running) {
            *$self.guard_tx.lock() = Some(tx);
        };
        MongoErr::Unreachable($e)
    }};
}
