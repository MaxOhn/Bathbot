use std::{borrow::Borrow, sync::Arc, time::Duration};

use bathbot_util::IntHasher;
use eyre::{Report, Result};
use flexmap::tokio::TokioRwLockMap;
use futures::stream::StreamExt;
use rosu_render::{
    OrdrClient, OrdrWebsocket,
    client::error::ErrorCode,
    model::{RenderDone, RenderFailed, RenderProgress, Verification},
    websocket::event::RawEvent,
};
use tokio::{
    sync::{mpsc, mpsc::error::TrySendError, watch},
    time::{Instant, sleep, sleep_until},
};

/// Reconnect if no event was received for this long
///
/// Renders usually take less than 2 minutes, during which o!rdr sends
/// progress events every few seconds. Since the websocket is a global feed,
/// total silence of 2+ minutes strongly suggests a dead or zombie connection.
const WATCHDOG_TIMEOUT: Duration = Duration::from_secs(2 * 60);
/// Delay before the first reconnection attempt
const RECONNECT_DELAY: Duration = Duration::from_secs(1);
/// Upper bound for the reconnection delay
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(60);
/// A connection that stayed up at least this long resets the reconnection
/// backoff
const STABLE_CONNECTION_DURATION: Duration = Duration::from_secs(60);
/// How long to wait for the server's `bot_auth` reply
const AUTH_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Ordr {
    pub client: OrdrClient,
    pub senders: Arc<SenderMap>,
    shutdown_tx: watch::Sender<()>,
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

enum OrdrLoopExit {
    /// Stop reconnecting
    Shutdown,
    /// Connecting to the websocket failed
    ConnectFailed,
    /// No event was received for `WATCHDOG_TIMEOUT`
    WatchdogTimeout,
}

impl Ordr {
    pub fn new(
        #[cfg(not(debug_assertions))] verification_key: impl Into<Box<str>>,
    ) -> Result<Self> {
        let senders = Arc::new(SenderMap::with_shard_amount_and_hasher(8, IntHasher));

        #[cfg(not(debug_assertions))]
        let verification_key: Box<str> = verification_key.into();

        #[cfg(debug_assertions)]
        let verification = Verification::DevModeSuccess;

        #[cfg(not(debug_assertions))]
        let verification = Verification::Key(verification_key.clone());

        let client = OrdrClient::builder()
            .render_ratelimit(5_000, 1, 2) // Two requests per 10 seconds
            .verification(verification)
            .build();

        // The dev mode renders are public, no key to authenticate with
        #[cfg(debug_assertions)]
        let auth_key: Option<Box<str>> = None;

        #[cfg(not(debug_assertions))]
        let auth_key = Some(verification_key);

        let (shutdown_tx, shutdown_rx) = watch::channel(());

        tokio::spawn(supervise_ordr_events(
            Arc::clone(&senders),
            auth_key,
            shutdown_rx,
        ));
        tokio::spawn(poll_finished_renders(client.clone(), Arc::clone(&senders)));

        Ok(Self {
            client,
            senders,
            shutdown_tx,
        })
    }

    pub fn client(&self) -> &OrdrClient {
        &self.client
    }

    pub fn disconnect(&self) {
        let _ = self.shutdown_tx.send(());
    }

    pub async fn subscribe_render_id(&self, render_id: u32) -> OrdrReceivers {
        debug!(render_id, "Subscribing");

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
        debug!(render_id, "Unsubscribing");

        self.senders.own(RenderId(render_id)).await.remove();
    }
}

/// Whether a polled render has reached a terminal state, and if so whether it
/// failed. o!rdr reports `videoUrl: "None"` for renders that have not
/// finished yet (queued, rendering, uploading, or failed); a finished render
/// carries a real URL. The field is never empty.
fn render_terminal(removed: bool, error_code: u8, video_url: &str) -> (bool, bool) {
    let finished = removed || error_code != 0 || video_url != "None";
    let failed = removed || error_code != 0;

    (finished, failed)
}

/// Periodically re-query the renders we are subscribed to.
///
/// `render_done` and `render_failed` events fire once: if the websocket
/// connection was down when o!rdr emitted one (for example right before the
/// watchdog restarts it), the event is lost and the render would otherwise
/// hang until the 24 hour timeout. Since finished renders stay in the render
/// list, we can catch up from there.
async fn poll_finished_renders(client: OrdrClient, senders: Arc<SenderMap>) {
    const POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);

    loop {
        sleep(POLL_INTERVAL).await;

        let render_ids = {
            let mut ids = Vec::new();
            let mut iter = senders.iter();

            while let Some(guard) = iter.next().await {
                ids.push(*guard.key());
            }

            ids
        };

        for render_id in render_ids {
            let Ok(list) = client.render_list().render_id(render_id.0).await else {
                warn!(render_id = render_id.0, "Failed to query o!rdr render list",);

                continue;
            };

            let Some(render) = list.renders.into_iter().next() else {
                debug!(
                    render_id = render_id.0,
                    "o!rdr render list returned no render",
                );

                continue;
            };

            let (finished, failed) =
                render_terminal(render.removed, render.error_code, render.video_url.as_ref());

            if !finished {
                continue;
            }

            let (done_sender, failed_sender) = {
                // Do not hold the guard across any await, otherwise the write
                // lock below (and any other subscriber of this shard) might
                // deadlock.
                match senders.read(&render_id).await.get() {
                    Some(subscribers) => {
                        let done = subscribers.done.clone();
                        let failed = subscribers.failed.clone();

                        (Some(done), Some(failed))
                    }
                    None => (None, None),
                }
            };

            let render_id = render_id.0;

            if failed {
                if let Some(failed_sender) = failed_sender {
                    let (error_code, error_message) = if render.removed {
                        (None, "the render was removed from o!rdr".to_string())
                    } else {
                        let code = ErrorCode::from(render.error_code);

                        (Some(code), code.to_string())
                    };

                    let _ = failed_sender
                        .send(RenderFailed {
                            render_id,
                            error_code,
                            error_message: error_message.into(),
                        })
                        .await;
                }
            } else if let Some(done_sender) = done_sender {
                let _ = done_sender
                    .send(RenderDone {
                        render_id,
                        video_url: render.video_url,
                    })
                    .await;
            }

            // Prevent the real event from firing twice, if it arrives now.
            senders.own(RenderId(render_id)).await.remove();
        }
    }
}

/// (Re)connects to the o!rdr websocket and restarts [`handle_ordr_events`]
/// whenever it exits without a shutdown request, be it through a watchdog
/// timeout, a connection error, or a panic.
async fn supervise_ordr_events(
    senders: Arc<SenderMap>,
    auth_key: Option<Box<str>>,
    mut shutdown_rx: watch::Receiver<()>,
) {
    let mut reconnect_delay = RECONNECT_DELAY;

    loop {
        let connected_at = Instant::now();

        let handle = tokio::spawn(handle_ordr_events(
            Arc::clone(&senders),
            auth_key.clone(),
            shutdown_rx.clone(),
        ));

        match handle.await {
            Ok(OrdrLoopExit::Shutdown) => return,
            Ok(OrdrLoopExit::ConnectFailed) => {
                warn!("Failed to connect to o!rdr websocket, reconnecting...");
            }
            Ok(OrdrLoopExit::WatchdogTimeout) => {
                warn!(
                    timeout = ?WATCHDOG_TIMEOUT,
                    "Received no o!rdr websocket events for a while, reconnecting...",
                );
            }
            Err(err) if err.is_panic() => {
                let panic = err.into_panic();

                let panic_msg = panic
                    .downcast_ref::<&'static str>()
                    .map(ToString::to_string)
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".to_owned());

                error!(panic_msg, "o!rdr event loop panicked, restarting...");
            }
            Err(err) => {
                error!(
                    err = ?Report::new(err),
                    "o!rdr event loop was cancelled, restarting...",
                );
            }
        }

        // Reset the backoff if the connection was stable for a while
        if connected_at.elapsed() > STABLE_CONNECTION_DURATION {
            reconnect_delay = RECONNECT_DELAY;
        }

        tokio::select! {
            _ = sleep(reconnect_delay) => (),
            _ = shutdown_rx.changed() => return,
        }

        reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
    }
}

async fn handle_ordr_events(
    senders: Arc<SenderMap>,
    auth_key: Option<Box<str>>,
    mut shutdown_rx: watch::Receiver<()>,
) -> OrdrLoopExit {
    let mut websocket = tokio::select! {
        connect_res = OrdrWebsocket::connect() => match connect_res {
            Ok(websocket) => websocket,
            Err(err) => {
                warn!(err = ?Report::new(err), "Failed to connect to o!rdr websocket");

                return OrdrLoopExit::ConnectFailed;
            }
        },
        _ = shutdown_rx.changed() => return OrdrLoopExit::Shutdown,
    };

    info!("Connected to o!rdr websocket");

    // The renders of the verified bot are private/unlisted, and their
    // events only flow to the connection authenticated with the key
    if let Some(key) = auth_key.as_deref() {
        let timeout = tokio::time::timeout(AUTH_TIMEOUT, websocket.authenticate(key));

        match timeout.await {
            Ok(Ok(())) => info!("Authenticated o!rdr websocket"),
            Ok(Err(err)) => {
                warn!(?err, "o!rdr websocket authentication failed");

                return OrdrLoopExit::ConnectFailed;
            }
            Err(_) => {
                warn!(
                    timeout = ?AUTH_TIMEOUT,
                    "o!rdr websocket authentication timed out",
                );

                return OrdrLoopExit::ConnectFailed;
            }
        }
    }

    let mut watchdog = Instant::now() + WATCHDOG_TIMEOUT;

    loop {
        let event_res = tokio::select! {
            event_res = websocket.next_event() => event_res,
            _ = shutdown_rx.changed() => return OrdrLoopExit::Shutdown,
            _ = sleep_until(watchdog) => return OrdrLoopExit::WatchdogTimeout,
        };

        match event_res {
            Ok(event) => {
                watchdog = Instant::now() + WATCHDOG_TIMEOUT;

                match event {
                    RawEvent::RenderProgress(progress) => {
                        let render_id = progress.render_id;

                        let sender = senders
                            .read(&render_id)
                            .await
                            .get()
                            .map(|senders| senders.progress.clone());

                        let Some(sender) = sender else {
                            trace!(render_id, "No subscribers for o!rdr render progress",);

                            continue;
                        };

                        match progress.deserialize() {
                            Ok(progress) => {
                                // Progress updates are cosmetic, drop them
                                // instead of blocking on slow subscribers
                                if let Err(TrySendError::Closed(_)) = sender.try_send(progress) {
                                    warn!(render_id, "Progress receiver was closed",);
                                }
                            }
                            Err(err) => warn!(
                                err = ?Report::new(err),
                                ?progress,
                                "Failed to deserialize o!rdr event"
                            ),
                        }
                    }
                    RawEvent::RenderDone(done) => {
                        let render_id = done.render_id;

                        let sender = senders
                            .read(&render_id)
                            .await
                            .get()
                            .map(|senders| senders.done.clone());

                        let Some(sender) = sender else {
                            trace!(render_id, "No subscribers for o!rdr render done",);

                            continue;
                        };

                        match done.deserialize() {
                            Ok(done) => {
                                let _ = sender.send(done).await;
                            }
                            Err(err) => warn!(
                                err = ?Report::new(err),
                                ?done,
                                "Failed to deserialize o!rdr event"
                            ),
                        }
                    }
                    RawEvent::RenderFailed(failed) => {
                        let render_id = failed.render_id;

                        let sender = senders
                            .read(&render_id)
                            .await
                            .get()
                            .map(|senders| senders.failed.clone());

                        let Some(sender) = sender else {
                            trace!(render_id, "No subscribers for o!rdr render failed",);

                            continue;
                        };

                        match failed.deserialize() {
                            Ok(failed) => {
                                let _ = sender.send(failed).await;
                            }
                            Err(err) => warn!(
                                err = ?Report::new(err),
                                ?failed,
                                "Failed to deserialize o!rdr event"
                            ),
                        }
                    }
                    _ => {}
                }
            }
            Err(err) => warn!(err = ?Report::new(err), "o!rdr websocket error"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::render_terminal;

    #[test]
    fn poll_terminal_detection() {
        // In progress: no terminal state, the poll must leave it alone
        assert_eq!(render_terminal(false, 0, "None"), (false, false));

        // Done: o!rdr swaps the "None" placeholder for the real URL
        assert_eq!(
            render_terminal(false, 0, "https://link.issou.best/x"),
            (true, false)
        );

        // Failed: o!rdr sets the error code, the URL stays "None"
        assert_eq!(render_terminal(false, 5, "None"), (true, true));

        // Removed: the render is gone from o!rdr
        assert_eq!(render_terminal(true, 0, "None"), (true, true));
    }
}
