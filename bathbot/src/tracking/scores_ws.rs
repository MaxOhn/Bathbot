use std::{fs, time::Duration};

use eyre::{Report, Result, WrapErr};
use futures::{SinkExt, StreamExt};
use rosu_v2::prelude::Score;
use tokio::{net::TcpStream, sync::oneshot};
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{core::BotConfig, tracking::OsuTracking};

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// <https://github.com/MaxOhn/scores-ws>
pub struct ScoresWebSocket;

impl ScoresWebSocket {
    const RESUME_ID_PATH: &str = "./resume_score_id.txt";

    pub async fn connect() -> Result<ScoresWebSocketDisconnect> {
        let score_id = match fs::read_to_string(Self::RESUME_ID_PATH) {
            Ok(content) => match content.parse::<u64>() {
                Ok(score_id) => Some(score_id),
                Err(_) => {
                    error!(content, "Expected integer in `{}`", Self::RESUME_ID_PATH);

                    None
                }
            },
            Err(err) => {
                error!(?err, "Failed to read `{}`", Self::RESUME_ID_PATH);

                None
            }
        };

        let port = BotConfig::get().scores_ws_port;
        let url = format!("ws://127.0.0.1:{port}");

        let timeout = Duration::from_secs(5);
        let connect_fut = tokio_tungstenite::connect_async(url);

        let mut stream = match tokio::time::timeout(timeout, connect_fut).await {
            Ok(Ok((stream, _))) => stream,
            Ok(Err(err)) => return Err(Report::new(err).wrap_err("Failed to connect")),
            Err(_) => bail!("Timed out while connecting"),
        };

        let init = if let Some(score_id) = score_id {
            Message::text(score_id.to_string())
        } else {
            Message::text("connect")
        };

        stream
            .send(init)
            .await
            .wrap_err("Failed to send initial message")?;

        let (output, keep) = ScoresWebSocketDisconnect::new();

        tokio::spawn(Self::run(stream, keep));

        Ok(output)
    }

    async fn run(mut stream: WebSocket, disconnect: ScoresWebSocketDisconnect) {
        let ScoresWebSocketDisconnect { mut tx, mut rx } = disconnect;

        let Some((disconnect_tx, disconnect_rx)) = tx.take().zip(rx.take()) else {
            return;
        };

        tokio::select! {
            _ = Self::read(&mut stream) => error!("Scores websocket stream ended"),
            _ = disconnect_rx => {
                Self::disconnect(stream).await;
                let _: Result<_, _> = disconnect_tx.send(());
            },
        }
    }

    async fn read(stream: &mut WebSocket) {
        while let Some(res) = stream.next().await {
            let bytes = match res {
                Ok(Message::Binary(bytes)) => bytes,
                Ok(msg) => {
                    warn!(?msg, "Expected binary message");

                    continue;
                }
                Err(err) => {
                    error!(?err, "Websocket error");

                    break;
                }
            };

            let score: Score = match serde_json::from_slice(&bytes) {
                Ok(score) => score,
                Err(err) => {
                    warn!(?err, "Failed to deserialize websocket message");

                    continue;
                }
            };

            OsuTracking::process_score(score).await;
        }
    }

    async fn disconnect(mut stream: WebSocket) {
        info!("Initiating scores websocket disconnect...");

        if let Err(err) = stream.send(Message::text("disconnect")).await {
            return warn!(?err, "Failed to send disconnect message");
        }

        let timeout = Duration::from_secs(10);

        let data = match tokio::time::timeout(timeout, stream.next()).await {
            Ok(Some(Ok(msg))) => msg.into_data(),
            Ok(Some(Err(err))) => return warn!(?err, "Error in disconnect response"),
            Ok(None) => return warn!("Did not receive disconnect response"),
            Err(_) => return warn!("Timed out while awaiting disconnect response"),
        };

        let opt = std::str::from_utf8(&data)
            .ok()
            .and_then(|content| content.parse::<u64>().ok());

        let Some(score_id) = opt else {
            return warn!(?data, "Expected score id as disconnect response");
        };

        if let Err(err) = fs::write(Self::RESUME_ID_PATH, score_id.to_string().as_bytes()) {
            error!(?err, "Failed to store score id");
        }
    }
}

pub struct ScoresWebSocketDisconnect {
    /// Sender to notify when a disconnect should be initiated
    pub tx: Option<oneshot::Sender<()>>,
    /// Receiver for when the disconnect as been finished
    pub rx: Option<oneshot::Receiver<()>>,
}

impl ScoresWebSocketDisconnect {
    fn new() -> (Self, Self) {
        let (start_tx, start_rx) = oneshot::channel();
        let (end_tx, end_rx) = oneshot::channel();

        (
            Self {
                tx: Some(start_tx),
                rx: Some(end_rx),
            },
            Self {
                tx: Some(end_tx),
                rx: Some(start_rx),
            },
        )
    }
}
