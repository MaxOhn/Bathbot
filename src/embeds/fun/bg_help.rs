use crate::embeds::EmbedData;

pub struct BGHelpEmbed {
    title: &'static str,
    description: &'static str,
    fields: Vec<(String, String, bool)>,
}

impl BGHelpEmbed {
    pub fn new(prefix: String) -> Self {
        let description = "Given part of a map's background, \
            try to guess the **title** of the map's song.\n\
            You don't need to guess content in parentheses `(...)` \
            or content after `ft.` or `feat.`.\n\
            Use these subcommands to initiate with the game:";
        let fields = vec![
            (
                "start / s / skip / resolve / r".to_owned(),
                format!(
                    "__If no game is running yet:__\n\
                    Start the game in the current channel.\n\
                    First, you get to choose which kind of backgrounds \
                    you will need to guess.\n\
                    React to require a tag, or react-unreact to exclude a tag.\n\
                    If no tag is chosen, all backgrounds will be selected.\n\
                    __If the game is already going:__\n\
                    Resolve the current background and give a new one \
                    with the same tag specs.\n\
                    To change mode or tags, be sure to `{prefix}bg stop` first.\n\
                    __Mania:__\n\
                    *Start* the game with the additional argument \
                    `mania` or just `m` e.g. `{prefix}bg s m`. \
                    Once the mania game is running, you can skip with `{prefix}bg s`.",
                    prefix = prefix
                ),
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
                "stop / end / quit".to_owned(),
                "Resolve the current background and stop the game in this channel".to_owned(),
                true,
            ),
            (
                "ranking / leaderboard / lb / stats".to_owned(),
                format!(
                    "Check out the leaderboard of this server.\n\
                    Add the argument `global` or just `g` (e.g. `{prefix}bg lb g`) \
                    to get the leaderboard across all servers",
                    prefix = prefix
                ),
                false,
            ),
        ];
        Self {
            fields,
            description,
            title: "Background guessing game",
        }
    }
}

impl EmbedData for BGHelpEmbed {
    fn title(&self) -> Option<&str> {
        Some(self.title)
    }
    fn description(&self) -> Option<&str> {
        Some(self.description)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
}
