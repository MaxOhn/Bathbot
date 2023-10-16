use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};

use metrics::{Counter, Gauge, Histogram, HistogramFn, Key, KeyName, Recorder, SharedString, Unit};
use metrics_util::registry::{Registry, Storage};

/// Only records the amount of times histograms where updated, not the specific
/// values.
struct HistogramCount(AtomicUsize);

impl HistogramFn for HistogramCount {
    fn record(&self, _: f64) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

struct ReaderStorage;

impl<K> Storage<K> for ReaderStorage {
    type Counter = Arc<AtomicU64>;
    type Gauge = Arc<AtomicU64>;
    type Histogram = Arc<HistogramCount>;

    fn counter(&self, _: &K) -> Self::Counter {
        Arc::new(AtomicU64::new(0))
    }

    fn gauge(&self, _: &K) -> Self::Gauge {
        Arc::new(AtomicU64::new(0))
    }

    fn histogram(&self, _: &K) -> Self::Histogram {
        Arc::new(HistogramCount(AtomicUsize::new(0)))
    }
}

struct Inner {
    registry: Registry<Key, ReaderStorage>,
}

impl Inner {
    fn new() -> Self {
        Self {
            registry: Registry::new(ReaderStorage),
        }
    }
}

#[derive(Clone)]
pub struct MetricsReader {
    inner: Arc<Inner>,
}

impl Default for MetricsReader {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsReader {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner::new()),
        }
    }

    pub fn counter_value(&self, key: &Key) -> u64 {
        self.inner
            .registry
            .get_or_create_counter(key, |counter| counter.load(Ordering::Relaxed))
    }

    pub fn gauge_value(&self, key: &Key) -> f64 {
        self.inner
            .registry
            .get_or_create_gauge(key, |gauge| f64::from_bits(gauge.load(Ordering::Relaxed)))
    }

    pub fn collect_counters<F, U>(&self, key: &Key, mut f: F) -> Vec<U>
    where
        F: FnMut(&Key, u64) -> U,
    {
        let mut counters = Vec::new();

        self.inner.registry.visit_counters(|counter_key, counter| {
            if eq_keys(key, counter_key) {
                counters.push(f(counter_key, counter.load(Ordering::Relaxed)));
            }
        });

        counters
    }

    pub fn collect_histograms<F, U>(&self, key: &Key, mut f: F) -> Vec<U>
    where
        F: FnMut(&Key, usize) -> U,
    {
        let mut counters = Vec::new();

        self.inner.registry.visit_histograms(|hist_key, hist| {
            if eq_keys(key, hist_key) {
                counters.push(f(hist_key, hist.0.load(Ordering::Relaxed)));
            }
        });

        counters
    }

    pub fn sum_counters(&self, key: &Key) -> u64 {
        let mut sum = 0;

        self.inner.registry.visit_counters(|counter_key, counter| {
            if eq_keys(key, counter_key) {
                sum += counter.load(Ordering::Relaxed);
            }
        });

        sum
    }

    pub fn sum_histograms(&self, key: &Key) -> usize {
        let mut sum = 0;

        self.inner.registry.visit_histograms(|hist_key, hist| {
            if eq_keys(key, hist_key) {
                sum += hist.0.load(Ordering::Relaxed);
            }
        });

        sum
    }
}

impl Recorder for MetricsReader {
    fn register_counter(&self, key: &Key) -> Counter {
        self.inner
            .registry
            .get_or_create_counter(key, |c| Counter::from_arc(c.clone()))
    }

    fn register_gauge(&self, key: &Key) -> Gauge {
        self.inner
            .registry
            .get_or_create_gauge(key, |c| Gauge::from_arc(c.clone()))
    }

    fn register_histogram(&self, key: &Key) -> Histogram {
        self.inner
            .registry
            .get_or_create_histogram(key, |c| Histogram::from_arc(c.clone()))
    }

    fn describe_counter(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn describe_gauge(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn describe_histogram(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}
}

fn eq_keys(a: &Key, b: &Key) -> bool {
    if a.name() != b.name() {
        return false;
    }

    a.labels()
        .all(|label| b.labels().any(|counter_label| label == counter_label))
}
