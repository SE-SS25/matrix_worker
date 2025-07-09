macro_rules! backoff {
    ($self:expr) => {{
        #[allow(unused_imports)]
        use anyhow::{anyhow, bail};
        use core::sync::atomic::Ordering;
        use matrix_errors::MongoErr;

        // TODO Write in err db if err
        if $self.db_has_problem.load(Ordering::SeqCst) {
            bail!(MongoErr::Unreachable(anyhow!("Unreachable")));
        }
        let Some(client) = &*$self.client else {
            bail!(MongoErr::InvalidUrl($self.db_id.to_string()));
        };

        client
    }};
}

macro_rules! fritz {
    ($self:expr, $e:expr) => {{
        #[allow(unused_imports)]
        use crate::guard::MongoGuard;
        use core::sync::atomic::Ordering;
        use matrix_errors::MongoErr;

        if $self.client.is_none() {
            return MongoErr::InvalidUrl($self.db_id.to_string());
        };

        $self.db_has_problem.store(true, Ordering::SeqCst);
        MongoErr::Unreachable($e)
    }};
}
