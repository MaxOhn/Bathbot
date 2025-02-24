use std::fmt::Write;

use bathbot_macros::PaginationBuilder;
use bathbot_util::{EmbedBuilder, FooterBuilder, datetime::HowLongAgoText};
use eyre::Result;
use futures::future::BoxFuture;
use time::OffsetDateTime;
use twilight_model::{
    application::command::CommandOptionType,
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    core::commands::interaction::InteractionCommands,
    util::interaction::{InteractionComponent, InteractionModal},
};

type Counts = Box<[(Box<str>, u32)]>;

#[derive(PaginationBuilder)]
pub struct SlashCommandsPagination {
    #[pagination(per_page = 10)]
    counts: Counts,
    start_time: OffsetDateTime,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for SlashCommandsPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let idx = self.pages.index();
        let counts = &self.counts[idx..self.counts.len().min(idx + self.pages.per_page())];

        let mut description = String::with_capacity(1024);
        let cmds = InteractionCommands::get();

        for ((full_name, count), idx) in counts.iter().zip(idx + 1..) {
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
                "{idx}. `{count}` {mention} {cmd_desc}",
                mention = cmd_kind.mention(full_name),
            );
        }

        let footer = format!(
            "Page {}/{} • Started counting {}",
            self.pages.curr_page(),
            self.pages.last_page(),
            HowLongAgoText::new(&self.start_time)
        );

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(FooterBuilder::new(footer))
            .title("Most popular slash commands:");

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(modal, self.msg_owner, false, &mut self.pages)
    }
}
