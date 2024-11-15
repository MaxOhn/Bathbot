use std::{path::PathBuf, sync::Arc, time::Duration};

use axum::{
    http::StatusCode,
    middleware,
    response::Response,
    routing::{get, get_service},
    Router,
};
use eyre::Result;
use hyper::Request;
use tokio::sync::oneshot::{channel, Receiver, Sender};
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::Span;

use crate::{
    middleware::metrics::track_metrics,
    routes::{
        auth::{osu::auth_osu, twitch::auth_twitch},
        guild_count::get_guild_count,
        metrics::get_metrics,
        osudirect::redirect_osudirect,
    },
    standby::AuthenticationStandby,
    state::AppState,
    AppStateBuilder,
};

pub struct Server {
    state: AppState,
    website_path: PathBuf,
    shutdown_rx: Receiver<()>,
}

impl Server {
    pub fn new(builder: AppStateBuilder) -> Result<(Self, Arc<AuthenticationStandby>, Sender<()>)> {
        let (shutdown_tx, shutdown_rx) = channel();
        let standby = Arc::new(AuthenticationStandby::new());
        let (state, website_path) = builder.build(Arc::clone(&standby))?;

        let server = Self {
            state,
            website_path,
            shutdown_rx,
        };

        Ok((server, standby, shutdown_tx))
    }

    pub async fn run(self, port: u16) {
        let Self {
            state,
            website_path,
            shutdown_rx,
        } = self;

        let state = Arc::new(state);
        let app = Self::bathbot_app(website_path, Arc::clone(&state));

        let server = axum::Server::bind(&([0, 0, 0, 0], port).into())
            .serve(app.with_state(state).into_make_service())
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });

        info!("Running server...");

        if let Err(err) = server.await {
            error!(?err, "Server failed");
        }
    }

    fn bathbot_app(website_path: PathBuf, state: Arc<AppState>) -> Router<Arc<AppState>> {
        let trace = TraceLayer::new_for_http()
            .on_request(|req: &Request<_>, _: &Span| info!("{} {}", req.method(), req.uri().path()))
            .on_response(|res: &Response, latency: Duration, _: &Span| {
                let status = res.status().as_u16();

                if (500..600).contains(&status) {
                    error!(status, "Response: latency={}ms", latency.as_millis());
                } else {
                    info!(status, "Response: latency={}ms", latency.as_millis());
                }
            });

        Router::new()
            .route("/metrics", get(get_metrics))
            .route("/guild_count", get(get_guild_count))
            .nest("/auth", Self::auth_app(website_path))
            .route("/osudirect/:mapset_id", get(redirect_osudirect))
            .layer(CorsLayer::permissive())
            .layer(middleware::from_fn_with_state(state, track_metrics))
            .layer(trace)
    }

    fn auth_app(website_path: PathBuf) -> Router<Arc<AppState>> {
        let mut auth_assets = website_path;
        auth_assets.push("assets/auth");

        Router::new()
            .route("/osu", get(auth_osu))
            .route("/twitch", get(auth_twitch))
            .fallback_service(
                get_service(ServeDir::new(auth_assets).with_buf_chunk_size(16_384)).handle_error(
                    |err| async move {
                        error!(?err, "Failed to serve static file");

                        StatusCode::INTERNAL_SERVER_ERROR
                    },
                ),
            )
    }
}
