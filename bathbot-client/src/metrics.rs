use std::time::Duration;

use http::StatusCode;
use metrics::{counter, describe_counter, describe_histogram, histogram};

use crate::site::Site;

const CLIENT_RESPONSE_TIME: &str = "client_response_time";
const CLIENT_INTERNAL_ERRORS: &str = "client_internal_errors";

pub(crate) struct ClientMetrics;

impl ClientMetrics {
    pub(crate) fn init() {
        describe_histogram!(
            CLIENT_RESPONSE_TIME,
            "Response time for client requests in seconds"
        );

        describe_counter!(
            CLIENT_INTERNAL_ERRORS,
            "Number of times an internal error occurred"
        );
    }

    pub(crate) fn observe(site: Site, status: StatusCode, latency: Duration) {
        histogram!(
            CLIENT_RESPONSE_TIME,
            latency,
            "site" => site.as_str(),
            "status" => status.as_str().to_owned()
        );
    }

    pub(crate) fn internal_error(site: Site) {
        counter!(CLIENT_INTERNAL_ERRORS, 1, "site" => site.as_str());
    }
}
