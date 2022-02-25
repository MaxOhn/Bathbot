use eyre::Report;

use crate::{commands::fun::GameState, Context};

impl Context {
    #[cold]
    pub async fn stop_all_games(&self) -> usize {
        let active_games: Vec<_> = self.bg_games().iter().map(|entry| *entry.key()).collect();

        if active_games.is_empty() {
            return 0;
        }

        let mut count = 0;

        let content = "I'll abort this game because I'm about to reboot, \
            you can start a new game again in just a moment...";

        let msg_fut = |channel| {
            self.http
                .create_message(channel)
                .content(content)
                .unwrap()
                .exec()
        };

        for channel in active_games {
            if let Some((_, state)) = self.bg_games().remove(&channel) {
                match state {
                    GameState::Running { game } => match game.stop() {
                        Ok(_) => {
                            let _ = msg_fut(channel).await;

                            count += 1;
                        }
                        Err(err) => {
                            let wrap = format!("error while stopping game in channel {channel}");
                            let report = Report::new(err).wrap_err(wrap);
                            warn!("{report:?}");
                        }
                    },
                    GameState::Setup { .. } => {
                        let _ = msg_fut(channel).await;

                        count += 1;
                    }
                }
            }
        }

        count
    }
}
