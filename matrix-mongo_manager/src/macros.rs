macro_rules! backoff {
    ($manager:expr) => {{
        #[allow(unused_imports)]
        use ::tracing::warn;
        use anyhow::{anyhow, bail};
        use core::sync::atomic::Ordering;
        use matrix_errors::MongoErr;

        if $manager.db_has_problem.load(Ordering::SeqCst) {
            bail!(MongoErr::Unreachable(anyhow!("Unreachable")));
        }
        let Some(client) = &*$manager.client else {
            if let Err(e) = $manager.tx.try_send($manager.url.clone()) {
                warn!(?e, "Failed to send url to db_manager");
            }
            bail!(MongoErr::InvalidUrl($manager.db_id.to_string()));
        };

        client
    }};
}

macro_rules! fritz {
    ($manager:expr, $e:expr) => {{
        #[allow(unused_imports)]
        use crate::guard::MongoGuard;
        use ::tracing::warn;
        use core::sync::atomic::Ordering;
        use matrix_errors::MongoErr;

        if let Err(e) = $manager.tx.try_send($manager.url.clone()) {
            warn!(?e, "Failed to send url to db_manager");
        }
        if $manager.client.is_none() {
            return MongoErr::InvalidUrl($manager.db_id.to_string());
        };

        $manager.db_has_problem.store(true, Ordering::SeqCst);
        MongoErr::Unreachable($e)
    }};
}
