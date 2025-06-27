macro_rules! backoff {
    ($self:expr) => {{
        use crate::guard::MongoGuard;
        #[allow(unused_imports)]
        use anyhow::{anyhow, bail};
        use core::sync::atomic::Ordering;
        use matrix_errors::MongoErr;

        if MongoGuard::is_running(Ordering::SeqCst) {
            bail!(MongoErr::Unreachable(anyhow!("Unreachable")));
        }
        $self.client.clone()
    }};
}

macro_rules! fritz {
    ($self:expr, $e:expr) => {{
        use crate::guard::MongoGuard;
        #[allow(unused_imports)]
        use matrix_errors::MongoErr;

        MongoGuard::init(&$self.client);
        MongoErr::Unreachable($e)
    }};
}
