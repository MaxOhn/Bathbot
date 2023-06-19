use std::{borrow::Borrow, sync::Arc};

use bathbot_util::IntHasher;
use eyre::{Result, WrapErr};
use flexmap::tokio::TokioRwLockMap;
use rosu_render::{
    model::{RenderDone, RenderFail, RenderProgress, Verification},
    Ordr as Client,
};
use tokio::sync::mpsc;

pub struct Ordr {
    pub client: Client,
    pub senders: Arc<SenderMap>,
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
    pub failed: mpsc::Sender<RenderFail>,
    pub progress: mpsc::Sender<RenderProgress>,
}

pub struct OrdrReceivers {
    pub done: mpsc::Receiver<RenderDone>,
    pub failed: mpsc::Receiver<RenderFail>,
    pub progress: mpsc::Receiver<RenderProgress>,
}

impl Ordr {
    pub async fn new(
        #[cfg(not(debug_assertions))] verification_key: impl Into<Box<str>>,
    ) -> Result<Self> {
        let senders = Arc::new(SenderMap::with_shard_amount_and_hasher(8, IntHasher));

        let done_clone = Arc::clone(&senders);
        let fail_clone = Arc::clone(&senders);
        let progress_clone = Arc::clone(&senders);

        #[cfg(debug_assertions)]
        let verification = Verification::DevModeSuccess;

        #[cfg(not(debug_assertions))]
        let verification = Verification::Key(verification_key.into());

        let client = Client::builder()
            .verification(verification)
            .with_websocket(true)
            .on_render_done(move |msg| {
                let done_clone = Arc::clone(&done_clone);

                Box::pin(async move {
                    let render_id = msg.render_id;
                    let guard = done_clone.read(&render_id).await;

                    if let Some(senders) = guard.get() {
                        let _ = senders.done.send(msg).await;
                    }
                })
            })
            .on_render_failed(move |msg| {
                let fail_clone = Arc::clone(&fail_clone);

                Box::pin(async move {
                    let render_id = msg.render_id;
                    let guard = fail_clone.read(&render_id).await;

                    if let Some(senders) = guard.get() {
                        let _ = senders.failed.send(msg).await;
                    }
                })
            })
            .on_render_progress(move |msg| {
                let progress_clone = Arc::clone(&progress_clone);

                Box::pin(async move {
                    let render_id = msg.render_id;
                    let guard = progress_clone.read(&render_id).await;

                    if let Some(senders) = guard.get() {
                        let _ = senders.progress.send(msg).await;
                    }
                })
            })
            .build()
            .await
            .wrap_err("Failed to connect ordr websocket")?;

        Ok(Self { client, senders })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn subscribe_render_id(&self, render_id: u32) -> OrdrReceivers {
        let (done_tx, done_rx) = mpsc::channel(1);
        let (failed_tx, failed_rx) = mpsc::channel(1);
        let (progress_tx, progress_rx) = mpsc::channel(8);

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
