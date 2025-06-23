use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, instrument, trace, warn};
use uuid::Uuid;

pub type MetricsWrapper = Arc<Metrics>;
type MetricStore = RwLock<VecDeque<Instant>>;

macro_rules! filter {
    ($guard:expr, $ts:expr) => {
        while $guard
            .front()
            .map_or(false, |i| $ts.duration_since(*i) > METRIC_TTL)
        {
            $guard.pop_front();
        }
    };
}

const METRIC_TTL_SECS: u64 = 60;
const METRIC_TTL: Duration = Duration::from_secs(METRIC_TTL_SECS);

#[derive(Debug)]
pub struct Metrics {
    id: Uuid,
    reads: MetricStore,
    writes: MetricStore,
    total_requests: AtomicU64,
    total_failed_requests: AtomicU64,
}

impl Metrics {
    #[instrument]
    pub fn new() -> MetricsWrapper {
        debug!("Creating MetricsWrapper");

        let metrics = Self {
            id: Uuid::new_v4(),
            reads: Default::default(),
            writes: Default::default(),
            total_requests: Default::default(),
            total_failed_requests: Default::default(),
        };
        trace!(?metrics);

        Arc::new(metrics)
    }

    pub fn read(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        Self::insert_metric(&self.reads)
    }
    pub fn write(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        Self::insert_metric(&self.writes)
    }

    pub fn fail(&self) {
        self.total_failed_requests.fetch_add(1, Ordering::Relaxed);
    }

    fn insert_metric(lock: &MetricStore) {
        let now = Instant::now();
        let mut guard = lock.write();

        filter!(guard, now);

        guard.push_back(now);
    }

    pub fn read_ps(&self) -> usize {
        Self::rps(&self.reads)
    }
    pub fn write_ps(&self) -> usize {
        Self::rps(&self.writes)
    }

    fn rps(lock: &MetricStore) -> usize {
        let now = Instant::now();
        let guard = lock.read();
        let reqs_in_win = guard
            .iter()
            .filter(|req_ts| now.duration_since(**req_ts) <= METRIC_TTL)
            .count();

        reqs_in_win / (METRIC_TTL_SECS as usize)
    }

    pub fn get_total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }
    pub fn get_total_fails(&self) -> u64 {
        self.total_failed_requests.load(Ordering::Relaxed)
    }
}
