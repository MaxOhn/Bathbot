use crate::embeds::EmbedData;

#[derive(Clone)]
pub struct BGHelpEmbed {
    title: String,
    description: String,
    fields: Vec<(String, String, bool)>,
}

impl BGHelpEmbed {
    pub fn new() -> Self {
        let title = "Background guessing game".to_string();
        let description = "Given part of a map's background, \
            try to guess the **title** of the map's song.\n\
            Content in parentheses `(...)` or content after `ft.` or `feat.` \
            will be removed from the title you need to guess.\n\
            You don't need to guess spot on, it suffices to get close enough.\n\
            Use these subcommands to initiate with the game:"
            .to_owned();
        let fields = vec![
            (
                "start / s / skip / resolve / r".to_owned(),
                "Start the game in the current channel. \
                If a game is already running, \
                it will resolve the background and give a new one.\n\
                For the mania version, **start** a game with \
                the additional argument `mania` or just `m` e.g. `<bg s m`. \
                Once the mania game is running, you can skip with `<bg s`.\n\
                To go from STD to MNA or vice versa, make sure to `<bg stop` first."
                    .to_owned(),
                false,
            ),
            (
                "hint / h / tip".to_owned(),
                "Receive a hint (can be used multiple times)".to_owned(),
                true,
            ),
            (
                "bigger / b / enhance".to_owned(),
                "Increase the radius of the displayed image \
                (can be used multiple times)"
                    .to_owned(),
                true,
            ),
            (
                "stats".to_owned(),
                "Check out how many backgrounds you guessed correctly in total".to_owned(),
                true,
            ),
            (
                "ranking / leaderboard / lb".to_owned(),
                "Check out the leaderboard of this server.\n\
                Add the argument `global` or just `g` (e.g. `<bg lb g`) \
                to get the leaderboard across all servers"
                    .to_owned(),
                true,
            ),
            (
                "stop / end".to_owned(),
                "Resolve the last background and stop the game in this channel.\n\
                Not required to use since the game will end automatically \
                if no one guessed the background after __3 minutes__."
                    .to_owned(),
                true,
            ),
        ];
        Self {
            title,
            description,
            fields,
        }
    }
}

impl EmbedData for BGHelpEmbed {
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
}
