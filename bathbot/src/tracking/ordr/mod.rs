use std::{
    borrow::Borrow,
    sync::{Arc, Mutex},
};

use bathbot_util::IntHasher;
use eyre::{Report, Result, WrapErr};
use flexmap::tokio::TokioRwLockMap;
use rosu_render::{
    OrdrClient, OrdrWebsocket,
    model::{RenderDone, RenderFailed, RenderProgress, Verification},
    websocket::event::RawEvent,
};
use tokio::sync::{mpsc, oneshot};

pub struct Ordr {
    pub client: OrdrClient,
    pub senders: Arc<SenderMap>,
    pub shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
}

type SenderMap = TokioRwLockMap<RenderId, OrdrSenders, IntHasher>;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderId(u32);

impl Borrow<u32> for RenderId {
    fn borrow(&self) -> &u32 {
        &self.0
    }
}

pub struct OrdrSenders {
    pub done: mpsc::Sender<RenderDone>,
    pub failed: mpsc::Sender<RenderFailed>,
    pub progress: mpsc::Sender<RenderProgress>,
}

pub struct OrdrReceivers {
    pub done: mpsc::Receiver<RenderDone>,
    pub failed: mpsc::Receiver<RenderFailed>,
    pub progress: mpsc::Receiver<RenderProgress>,
}

impl Ordr {
    pub async fn new(
        #[cfg(not(debug_assertions))] verification_key: impl Into<Box<str>>,
    ) -> Result<Self> {
        let senders = Arc::new(SenderMap::with_shard_amount_and_hasher(8, IntHasher));
        let senders_clone = Arc::clone(&senders);

        #[cfg(debug_assertions)]
        let verification = Verification::DevModeSuccess;

        #[cfg(not(debug_assertions))]
        let verification = Verification::Key(verification_key.into());

        let websocket = OrdrWebsocket::connect()
            .await
            .wrap_err("Failed to connect to o!rdr websocket")?;

        let client = OrdrClient::builder()
            .render_ratelimit(5_000, 1, 2) // Two request per 10 seconds
            .verification(verification)
            .build();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        tokio::spawn(handle_ordr_events(websocket, senders_clone, shutdown_rx));

        Ok(Self {
            client,
            senders,
            shutdown_tx: Mutex::new(Some(shutdown_tx)),
        })
    }

    pub fn client(&self) -> &OrdrClient {
        &self.client
    }

    pub fn disconnect(&self) {
        if let Ok(mut unlocked) = self.shutdown_tx.lock()
            && let Some(tx) = unlocked.take()
        {
            let _ = tx.send(());
        }
    }

    pub async fn subscribe_render_id(&self, render_id: u32) -> OrdrReceivers {
        let (done_tx, done_rx) = mpsc::channel(1);
        let (failed_tx, failed_rx) = mpsc::channel(1);
        let (progress_tx, progress_rx) = mpsc::channel(4);

        let senders = OrdrSenders {
            done: done_tx,
            failed: failed_tx,
            progress: progress_tx,
        };

        let receivers = OrdrReceivers {
            done: done_rx,
            failed: failed_rx,
            progress: progress_rx,
        };

        self.senders.own(RenderId(render_id)).await.insert(senders);

        receivers
    }

    pub async fn unsubscribe_render_id(&self, render_id: u32) {
        self.senders.own(RenderId(render_id)).await.remove();
    }
}

async fn handle_ordr_events(
    mut websocket: OrdrWebsocket,
    senders: Arc<SenderMap>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    loop {
        let event_res = tokio::select! {
            event_res = websocket.next_event() => event_res,
            _ = &mut shutdown_rx => {
                if let Err(err) = websocket.disconnect().await {
                    warn!(
                        err = ?Report::new(err),
                        "Failed to disconnect from o!rdr websocket",
                    );
                }

                return;
            }
        };

        match event_res {
            Ok(RawEvent::RenderProgress(progress)) => {
                let render_id = progress.render_id;
                let guard = senders.read(&render_id).await;

                if let Some(senders) = guard.get() {
                    match progress.deserialize() {
                        Ok(progress) => {
                            let _ = senders.progress.send(progress).await;
                        }
                        Err(err) => warn!(
                            err = ?Report::new(err),
                            ?progress,
                            "Failed to deserialize o!rdr event"
                        ),
                    }
                }
            }
            Ok(RawEvent::RenderDone(done)) => {
                let render_id = done.render_id;
                let guard = senders.read(&render_id).await;

                if let Some(senders) = guard.get() {
                    match done.deserialize() {
                        Ok(done) => {
                            let _ = senders.done.send(done).await;
                        }
                        Err(err) => warn!(
                            err = ?Report::new(err),
                            ?done,
                            "Failed to deserialize o!rdr event"
                        ),
                    }
                }
            }
            Ok(RawEvent::RenderFailed(failed)) => {
                let render_id = failed.render_id;
                let guard = senders.read(&render_id).await;

                if let Some(senders) = guard.get() {
                    match failed.deserialize() {
                        Ok(failed) => {
                            let _ = senders.failed.send(failed).await;
                        }
                        Err(err) => warn!(
                            err = ?Report::new(err),
                            ?failed,
                            "Failed to deserialize o!rdr event"
                        ),
                    }
                }
            }
            Ok(_) => {}
            Err(err) => warn!(err = ?Report::new(err), "o!rdr websocket error"),
        }
    }
}
