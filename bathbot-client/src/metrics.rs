use eyre::{Result, WrapErr};
use prometheus::{histogram_opts, opts, HistogramVec, IntCounterVec, Registry, DEFAULT_BUCKETS};

pub struct ClientMetrics {
    pub request_count: IntCounterVec,
    pub response_time: HistogramVec,
}

impl ClientMetrics {
    pub fn new(metrics: &Registry) -> Result<Self> {
        let opts = opts!("client_requests_total", "Requests total");
        let request_count = IntCounterVec::new(opts, &["site", "status"])
            .wrap_err("failed to create request count")?;

        let opts = histogram_opts!(
            "client_response_time_seconds",
            "Response times",
            DEFAULT_BUCKETS.to_vec()
        );
        let response_time =
            HistogramVec::new(opts, &["site"]).wrap_err("failed to create response time")?;

        metrics
            .register(Box::new(request_count.clone()))
            .wrap_err("failed to register request count")?;

        metrics
            .register(Box::new(response_time.clone()))
            .wrap_err("failed to register response time")?;

        Ok(Self {
            request_count,
            response_time,
        })
    }
}
