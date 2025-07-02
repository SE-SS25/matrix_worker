use std::sync::Arc;
use std::sync::mpsc::Sender;

pub(super) type MongoHookT = Arc<MongoHook>;

#[derive(Debug)]
pub(super) struct MongoHook {
    tx: Sender<()>,
}

impl MongoHook {
    pub(super) fn new(tx: Sender<()>) -> Self {
        Self { tx }
    }
}

impl Drop for MongoHook {
    fn drop(&mut self) {
        let _ = self.tx.send(());
    }
}
