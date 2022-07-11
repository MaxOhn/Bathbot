use eyre::Report;
use tokio_stream::StreamExt;

use crate::{games::bg::GameState, util::ChannelExt, Context};

impl Context {
    #[cold]
    pub async fn stop_all_games(&self) -> usize {
        let active_games: Vec<_> = self
            .bg_games()
            .iter()
            .await
            .map(|(id, state)| (*id, state.to_owned()))
            .collect()
            .await;

        if active_games.is_empty() {
            return 0;
        }

        let mut count = 0;

        let content = "I'll abort this game because I'm about to reboot, \
            you can start a new game again in just a moment...";

        for (channel, state) in active_games {
            match state {
                GameState::Running { game } => match game.stop() {
                    Ok(_) => {
                        let _ = channel.plain_message(self, content).await;
                        count += 1;
                    }
                    Err(err) => {
                        let wrap = format!("error while stopping game in channel {channel}");
                        let report = Report::new(err).wrap_err(wrap);
                        warn!("{report:?}");
                    }
                },
                GameState::Setup { .. } => {
                    let _ = channel.plain_message(self, content).await;
                    count += 1;
                }
            }
        }

        count
    }
}
