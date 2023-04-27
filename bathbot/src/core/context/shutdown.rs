use futures::stream::StreamExt;

use crate::{games::bg::GameState, util::ChannelExt, Context};

impl Context {
    #[cold]
    pub async fn stop_all_games(&self) -> usize {
        let mut active_games = Vec::new();
        let mut stream = self.bg_games().iter();

        while let Some(guard) = stream.next().await {
            let key = *guard.key();
            let value: GameState = guard.value().clone();

            active_games.push((key, value));
        }

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
                        warn!(?channel, ?err, "Error while stopping game");
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
