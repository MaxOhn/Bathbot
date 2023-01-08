use std::env;

use bathbot_server::{AppStateBuilder, Server};
use prometheus::{IntGauge, Registry};
use tokio::runtime::Runtime;
use tracing::error;
use tracing_subscriber::FmtSubscriber;

fn main() {
    dotenv::dotenv().unwrap();

    let subscriber = FmtSubscriber::builder()
        .compact()
        .with_env_filter("bathbot=debug,info")
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .unwrap_or_else(|err| panic!("Unable to set global default subscriber: {err}"));

    let metrics = Registry::new();

    let guild_counter = IntGauge::new("guild_count", "Guild count").unwrap();
    guild_counter.set(42);

    let port = env::var("SERVER_PORT").unwrap().parse().unwrap();

    let builder = AppStateBuilder {
        website_path: env::var("WEBSITE_PATH").unwrap().into(),
        metrics,
        guild_counter,
        osu_client_id: env::var("OSU_CLIENT_ID").unwrap().parse().unwrap(),
        osu_client_secret: env::var("OSU_CLIENT_SECRET").unwrap(),
        twitch_client_id: env::var("TWITCH_CLIENT_ID").unwrap(),
        twitch_token: env::var("TWITCH_TOKEN").unwrap(),
        redirect_base: env::var("PUBLIC_URL").unwrap(),
    };

    let (server, standby, _tx) = Server::new(builder);

    if let Err(err) = Runtime::new().unwrap().block_on(async {
        let _x = standby.wait_for_osu();

        server.run(port).await
    }) {
        error!("{:?}", err.wrap_err("Failed to run server"));
    }
}
