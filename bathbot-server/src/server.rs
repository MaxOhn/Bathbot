use std::sync::Arc;

use eyre::{Report, Result};
use tokio::sync::oneshot::Receiver;

use crate::{
    router::create_router, standby::AuthenticationStandby, state::AppState, AppStateBuilder,
};

pub struct Server {
    state: AppState,
}

impl Server {
    pub fn new(state: AppStateBuilder) -> Result<(Self, Arc<AuthenticationStandby>)> {
        let state = state.build()?;
        let standby = Arc::clone(&state.standby);

        Ok((Self { state }, standby))
    }

    pub async fn run(port: u16, shutdown_rx: Receiver<()>, state: AppState) {
        let app = create_router(state);

        let server = axum::Server::bind(&([0, 0, 0, 0], port).into())
            .serve(app.into_make_service())
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });

        info!("Running server...");

        if let Err(err) = server.await {
            error!("{:?}", Report::new(err).wrap_err("server failed"));
        }
    }
}
