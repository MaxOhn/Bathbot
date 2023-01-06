use bathbot_server::{AppStateBuilder, Server};
use prometheus::{IntCounter, Registry};
use tokio::{runtime::Runtime, sync::oneshot::channel};
use tracing_subscriber::FmtSubscriber;

fn main() {
    let subscriber = FmtSubscriber::builder()
        .compact()
        .with_env_filter("bathbot=debug,info")
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .unwrap_or_else(|err| panic!("Unable to set global default subscriber: {err}"));

    let (_tx, rx) = channel();

    let registry = Registry::new();
    registry
        .register(Box::new(IntCounter::new("counter", "My counter").unwrap()))
        .unwrap();

    let state = AppStateBuilder::default()
        .website("")
        .metrics(registry)
        .osu_client_id(10364)
        .osu_client_secret("")
        .osu_redirect("")
        .twitch_client_id("")
        .twitch_token("")
        .twitch_redirect("")
        .build()
        .unwrap();

    Runtime::new()
        .unwrap()
        .block_on(Server::run(7277, rx, state));
}
