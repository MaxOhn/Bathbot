use serenity::{
    framework::standard::{
        help_commands, macros::help, Args, CommandGroup, CommandResult, HelpOptions,
    },
    model::prelude::{Message, UserId},
    prelude::Context,
};
use std::collections::HashSet;

#[help]
#[individual_command_tip = "Prefix: `<` or `!!`\n\
If you want more information about a specific command, \
just pass the command as argument i.e. `<help command`.\n\
Commands can also be used in private messages to me, \
no need for any prefix in pms.\n\
If you want to specify a username that contains spaces, \
you must encapsulate the name with `\"` i.e. `\"nathan on osu\"`.\n\
Many commands allow you to specify mods. You can do so with `+mods` \
for included mods, `+mods!` for exact mods, or `-mods!` for excluded mods.\n\
If you react with :x: to my response to your command, I will delete it.
Further help on the spreadsheet: http://bit.ly/badecoms"]
#[command_not_found_text = "Could not find command: `{}`."]
#[max_levenshtein_distance(3)]
#[lacking_permissions = "strike"]
#[lacking_role = "strike"]
#[embed_success_colour("DARK_GREEN")]
#[strikethrough_commands_tip_in_dm(
    "~~`Strikethrough commands`~~ indicate you're lacking permissions or roles"
)]
#[strikethrough_commands_tip_in_guild("~~`Strikethrough commands`~~ can only be used in servers")]
fn help(
    context: &mut Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    help_commands::with_embeds(context, msg, args, help_options, groups, owners)
}
