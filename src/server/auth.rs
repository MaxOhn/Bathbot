use futures::future::FutureExt;
use hashbrown::HashMap;
use rosu_v2::prelude::User;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU8, Ordering},
        Mutex,
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    sync::oneshot::{self, Receiver, Sender},
    time::{self, Timeout},
};

use crate::custom_client::TwitchUser;

const DEADLINE: Duration = Duration::from_secs(120);

pub enum AuthenticationStandbyError {
    Canceled,
    Timeout,
}

#[derive(Default)]
pub struct AuthenticationStandby {
    // u8 is sufficient for 256 concurrent authorization awaitings within two minutes
    current_state: AtomicU8,
    osu: Mutex<HashMap<u8, Sender<User>>>,
    twitch: Mutex<HashMap<u8, Sender<TwitchUser>>>,
}

impl AuthenticationStandby {
    /// Wait for an osu! username to be authenticated.
    pub fn wait_for_osu(&self) -> WaitForOsuAuth {
        let (tx, rx) = oneshot::channel();
        let state = self.generate_state();
        let fut = Box::pin(time::timeout(DEADLINE, rx));
        self.osu.lock().unwrap().insert(state, tx);

        WaitForOsuAuth { state, fut }
    }

    pub fn is_osu_empty(&self) -> bool {
        self.osu.lock().unwrap().is_empty()
    }

    /// Wait for a twitch channel name to be authenticated.
    pub fn wait_for_twitch(&self) -> WaitForTwitchAuth {
        let (tx, rx) = oneshot::channel();
        let state = self.generate_state();
        let fut = Box::pin(time::timeout(DEADLINE, rx));
        self.twitch.lock().unwrap().insert(state, tx);

        WaitForTwitchAuth { state, fut }
    }

    pub fn is_twitch_empty(&self) -> bool {
        self.twitch.lock().unwrap().is_empty()
    }

    fn generate_state(&self) -> u8 {
        self.current_state.fetch_add(1, Ordering::SeqCst)
    }

    pub(super) fn process_osu(&self, user: User, id: u8) {
        if let Some(tx) = self.osu.lock().unwrap().remove(&id) {
            let _ = tx.send(user);
        }
    }

    pub(super) fn process_twitch(&self, user: TwitchUser, id: u8) {
        if let Some(tx) = self.twitch.lock().unwrap().remove(&id) {
            let _ = tx.send(user);
        }
    }
}

pub struct WaitForOsuAuth {
    pub state: u8,
    fut: Pin<Box<Timeout<Receiver<User>>>>,
}

impl Future for WaitForOsuAuth {
    type Output = Result<User, AuthenticationStandbyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.fut.poll_unpin(cx) {
            Poll::Ready(Ok(Ok(user))) => Poll::Ready(Ok(user)),
            Poll::Ready(Ok(Err(_))) => Poll::Ready(Err(AuthenticationStandbyError::Canceled)),
            Poll::Ready(Err(_)) => Poll::Ready(Err(AuthenticationStandbyError::Timeout)),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct WaitForTwitchAuth {
    pub state: u8,
    fut: Pin<Box<Timeout<Receiver<TwitchUser>>>>,
}

impl Future for WaitForTwitchAuth {
    type Output = Result<TwitchUser, AuthenticationStandbyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.fut.poll_unpin(cx) {
            Poll::Ready(Ok(Ok(user))) => Poll::Ready(Ok(user)),
            Poll::Ready(Ok(Err(_))) => Poll::Ready(Err(AuthenticationStandbyError::Canceled)),
            Poll::Ready(Err(_)) => Poll::Ready(Err(AuthenticationStandbyError::Timeout)),
            Poll::Pending => Poll::Pending,
        }
    }
}
