use serenity::{
    framework::standard::{
        help_commands, macros::help, Args, CommandGroup, CommandResult, HelpOptions,
    },
    model::prelude::{Message, UserId},
    prelude::Context,
};
use std::collections::HashSet;

#[help]
#[individual_command_tip = "Prefix: `<` or `!!` (none required in DMs)\n\
Most commands have (shorter) alternative aliases, e.g. `<rm` instead of `<recentmania`, \
so to check those out or get more information about a command in general, \
just pass the command as argument i.e. __**`<help command`**__.\n\
If you want to specify an argument, e.g. a username, that contains \
spaces, you must encapsulate it with `\"` i.e. `\"nathan on osu\"`.\n\
If you used `<link osuname`, you can ommit the osu username for any command that needs one.\n\
Many commands allow you to specify mods. You can do so with `+mods` \
for included mods, `+mods!` for exact mods, or `-mods!` for excluded mods.\n\
If you react with :x: to my response, I will delete it.
Further help on the spreadsheet: http://bit.ly/badecoms"]
#[command_not_found_text = "Could not find command: `{}`."]
#[max_levenshtein_distance(3)]
#[lacking_permissions = "strike"]
#[lacking_role = "strike"]
#[usage_label("How to use")]
#[usage_sample_label("Example")]
#[embed_success_colour("DARK_GREEN")]
#[strikethrough_commands_tip_in_dm(
    "~~`Strikethrough commands`~~ indicate you're lacking permissions or roles"
)]
#[strikethrough_commands_tip_in_guild("~~`Strikethrough commands`~~ can only be used in servers")]
async fn help(
    ctx: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    help_commands::with_embeds(ctx, msg, args, help_options, groups, owners).await
}
