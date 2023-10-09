use std::{fmt::Write, sync::Arc};

use bathbot_macros::PaginationBuilder;
use bathbot_util::EmbedBuilder;
use eyre::Result;
use futures::future::BoxFuture;
use twilight_model::application::command::CommandOptionType;

use crate::{
    active::{pagination::Pages, BuildPage, IActiveMessage},
    core::{commands::interaction::InteractionCommands, Context},
};

type Counts = Box<[(Box<str>, u32)]>;

#[derive(PaginationBuilder)]
pub struct SlashCommandsPagination {
    #[pagination(per_page = 10)]
    counts: Counts,
    pages: Pages,
}

impl IActiveMessage for SlashCommandsPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let idx = self.pages.index();
        let counts = &self.counts[idx..self.counts.len().min(idx + self.pages.per_page())];

        let mut description = String::with_capacity(1024);
        let cmds = InteractionCommands::get();

        for (full_name, count) in counts {
            let mut cmd_iter = full_name.split(' ');

            let Some(cmd_kind) = cmd_iter.next().and_then(|name| cmds.command(name)) else {
                continue;
            };

            let cmd = cmd_kind.create();

            let cmd_desc = match cmd_iter.next() {
                Some(group_or_sub) => {
                    let Some(option) = cmd
                        .options
                        .iter()
                        .find(|option| option.name == group_or_sub)
                    else {
                        error!(group_or_sub, command = cmd.name, "Missing group or sub");

                        continue;
                    };

                    match option.kind {
                        CommandOptionType::SubCommand => option.description.as_str(),
                        CommandOptionType::SubCommandGroup => {
                            let Some(sub_name) = cmd_iter.next() else {
                                error!(
                                    full_name,
                                    command = cmd.name,
                                    "Expected subcommand in group"
                                );

                                continue;
                            };

                            let Some(sub) = option
                                .options
                                .iter()
                                .flatten()
                                .find(|option| option.name == sub_name)
                            else {
                                error!(sub_name, command = cmd.name, "Missing subcommand");

                                continue;
                            };

                            sub.description.as_str()
                        }
                        _ => {
                            error!(kind = ?option.kind, command = cmd.name, "Invalid option");

                            continue;
                        }
                    }
                }
                None => cmd.description.as_str(),
            };

            let _ = writeln!(
                description,
                "- `{count}` {mention} {cmd_desc}",
                mention = cmd_kind.mention(full_name),
            );
        }

        let embed = EmbedBuilder::new()
            .description(description)
            .title("Popular slash commands");

        BuildPage::new(embed, false).boxed()
    }
}
