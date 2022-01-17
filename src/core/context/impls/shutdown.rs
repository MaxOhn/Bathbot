use eyre::Report;

use crate::Context;

impl Context {
    #[cold]
    pub async fn stop_all_games(&self) -> usize {
        let active_games = self.game_channels();

        if active_games.is_empty() {
            return 0;
        }

        let mut count = 0;

        let content = "I'll abort this game because I'm about to reboot, \
            you can start a new game again in just a moment...";

        for channel in active_games {
            match self.stop_game(channel) {
                Ok(true) => {
                    let _ = self
                        .http
                        .create_message(channel)
                        .content(content)
                        .unwrap()
                        .exec()
                        .await;

                    count += 1;
                }
                Ok(false) => {}
                Err(why) => {
                    let wrap = format!("error while stopping bg game in channel {channel}");
                    let report = Report::new(why).wrap_err(wrap);
                    warn!("{:?}", report);
                }
            }
        }

        count
    }
}
