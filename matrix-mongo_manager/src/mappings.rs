use parking_lot::RwLock;
use std::sync::Arc;

type MongoMappings = Arc<RwLock<Vec<Instance>>>;

#[derive(Debug)]
struct Instance {
    url: String,
    from: String,
    to: String,
}
