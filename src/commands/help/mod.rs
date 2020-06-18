mod structs;

use structs::*;

use crate::util::{Matrix, MessageExt};

use serenity::{
    cache::Cache,
    client::Context,
    framework::standard::{
        macros::help, Args, CheckResult, Command as InternalCommand, CommandGroup, CommandOptions,
        CommandResult, CommonOptions, HelpBehaviour, HelpOptions, OnlyIn,
    },
    http::Http,
    model::{
        channel::Message,
        guild::{Member, Role},
        id::{ChannelId, RoleId, UserId},
    },
    utils::Colour,
    Error,
};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};

macro_rules! format_command_name {
    ($behaviour:expr, $command_name:expr) => {
        match $behaviour {
            HelpBehaviour::Strike => format!("~~`{}`~~", $command_name),
            HelpBehaviour::Nothing => format!("`{}`", $command_name),
            HelpBehaviour::Hide => continue,
            HelpBehaviour::__Nonexhaustive => unreachable!(),
        }
    };
}

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
#[lacking_conditions = "strike"]
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
    let formatted_help =
        create_customised_help_data(ctx, msg, &args, &groups, &owners, help_options).await;
    let result = match formatted_help {
        CustomisedHelpData::SuggestedCommands {
            ref help_description,
            ref suggestions,
        } => {
            let text = help_description.replace("{}", &suggestions.join("`, `"));
            msg.channel_id
                .send_message(ctx, |m| {
                    m.embed(|e| {
                        e.colour(help_options.embed_success_colour)
                            .description(text)
                    })
                })
                .await
        }
        CustomisedHelpData::NoCommandFound {
            ref help_error_message,
        } => {
            msg.channel_id
                .send_message(ctx, |m| {
                    m.embed(|e| {
                        e.colour(help_options.embed_error_colour)
                            .description(help_error_message)
                    })
                })
                .await
        }
        CustomisedHelpData::GroupedCommands {
            ref help_description,
            ref groups,
        } => {
            msg.channel_id
                .send_message(ctx, |m| {
                    m.embed(|e| {
                        e.colour(help_options.embed_success_colour)
                            .description(help_description);
                        for group in groups {
                            let mut embed_text = String::default();
                            flatten_group_to_string(&mut embed_text, &group, 0, &help_options);
                            e.field(group.name, &embed_text, true);
                        }
                        e
                    })
                })
                .await
        }
        CustomisedHelpData::SingleCommand { ref command } => {
            send_single_command_embed(
                &ctx.http,
                &help_options,
                msg.channel_id,
                &command,
                help_options.embed_success_colour,
            )
            .await
        }
    };
    // For these 4 lines I had to copy the WHOLE helper message creation code...
    match result {
        Ok(response) => {
            response.reaction_delete(ctx, msg.author.id).await;
        }
        Err(why) => warn!("Failed to send help message because: {:?}", why),
    }
    Ok(())
}

pub async fn create_customised_help_data<'a>(
    ctx: &Context,
    msg: &Message,
    args: &'a Args,
    groups: &[&'static CommandGroup],
    owners: &HashSet<UserId>,
    help_options: &'a HelpOptions,
) -> CustomisedHelpData<'a> {
    if !args.is_empty() {
        let name = args.message();
        return match fetch_single_command(ctx, msg, &groups, &name, &help_options, owners).await {
            Ok(single_command) => single_command,
            Err(suggestions) => {
                if suggestions.is_empty() {
                    CustomisedHelpData::NoCommandFound {
                        help_error_message: &help_options.no_help_available_text,
                    }
                } else {
                    CustomisedHelpData::SuggestedCommands {
                        help_description: help_options.suggestion_text.to_string(),
                        suggestions: Suggestions(suggestions),
                    }
                }
            }
        };
    }
    let strikethrough_command_tip = if msg.is_private() {
        &help_options.strikethrough_commands_tip_in_guild
    } else {
        &help_options.strikethrough_commands_tip_in_dm
    };
    let description = if let Some(ref strikethrough_command_text) = strikethrough_command_tip {
        format!(
            "{}\n{}",
            &help_options.individual_command_tip, &strikethrough_command_text
        )
    } else {
        help_options.individual_command_tip.to_string()
    };
    let mut listed_groups: Vec<GroupCommandsPair> = Vec::default();
    for group in groups {
        let group = *group;
        let group_with_cmds = create_single_group(ctx, msg, group, &owners, &help_options).await;
        if !group_with_cmds.command_names.is_empty() || !group_with_cmds.sub_groups.is_empty() {
            listed_groups.push(group_with_cmds);
        }
    }
    if listed_groups.is_empty() {
        CustomisedHelpData::NoCommandFound {
            help_error_message: &help_options.no_help_available_text,
        }
    } else {
        CustomisedHelpData::GroupedCommands {
            help_description: description,
            groups: listed_groups,
        }
    }
}

async fn send_single_command_embed(
    http: impl AsRef<Http>,
    help_options: &HelpOptions,
    channel_id: ChannelId,
    command: &CommandSimple<'_>,
    colour: Colour,
) -> Result<Message, Error> {
    channel_id
        .send_message(&http, |m| {
            m.embed(|embed| {
                embed.title(&command.name);
                embed.colour(colour);
                if let Some(ref desc) = command.description {
                    embed.description(desc);
                }
                if let Some(ref usage) = command.usage {
                    let full_usage_text = format!("`{} {}`", command.name, usage);
                    embed.field(&help_options.usage_label, full_usage_text, true);
                }
                if !command.usage_sample.is_empty() {
                    let full_example_text = command
                        .usage_sample
                        .iter()
                        .map(|example| format!("`{} {}`\n", command.name, example))
                        .collect::<String>();
                    embed.field(&help_options.usage_sample_label, full_example_text, true);
                }
                embed.field(&help_options.grouped_label, command.group_name, true);
                if !command.aliases.is_empty() {
                    embed.field(
                        &help_options.aliases_label,
                        format!("`{}`", command.aliases.join("`, `")),
                        true,
                    );
                }
                embed.field(&help_options.available_text, &command.availability, true);
                if !command.checks.is_empty() {
                    embed.field(
                        &help_options.checks_label,
                        format!("`{}`", command.checks.join("`, `")),
                        true,
                    );
                }
                if !command.sub_commands.is_empty() {
                    embed.field(
                        &help_options.sub_commands_label,
                        format!("`{}`", command.sub_commands.join("`, `")),
                        true,
                    );
                }
                embed
            });
            m
        })
        .await
}

async fn check_common_behaviour<'a>(
    cache: impl AsRef<Cache>,
    msg: &Message,
    options: &impl CommonOptions,
    owners: &HashSet<UserId>,
    help_options: &HelpOptions,
) -> HelpBehaviour {
    if !options.help_available() {
        return HelpBehaviour::Hide;
    }
    if options.only_in() == OnlyIn::Dm && !msg.is_private()
        || options.only_in() == OnlyIn::Guild && msg.is_private()
    {
        return help_options.wrong_channel;
    }
    if options.owners_only() && !owners.contains(&msg.author.id) {
        return help_options.lacking_ownership;
    }
    if options.owner_privilege() && owners.contains(&msg.author.id) {
        return HelpBehaviour::Nothing;
    }
    if !has_correct_permissions(&cache, options, msg).await {
        return help_options.lacking_permissions;
    }
    if let Some(guild) = msg.guild(&cache).await {
        if let Some(member) = guild.members.get(&msg.author.id) {
            if !has_correct_roles(options, &guild.roles, &member) {
                return help_options.lacking_role;
            }
        }
    }
    HelpBehaviour::Nothing
}

async fn nested_group_command_search<'rec, 'a: 'rec>(
    ctx: &'rec Context,
    msg: &'rec Message,
    groups: &'rec [&'static CommandGroup],
    name: &'rec mut String,
    help_options: &'a HelpOptions,
    similar_commands: &'rec mut Vec<SuggestedCommandName>,
    owners: &'rec HashSet<UserId>,
) -> Option<CustomisedHelpData<'a>> {
    for group in groups {
        let group = *group;
        let mut found: Option<&'static InternalCommand> = None;
        let group_behaviour =
            check_common_behaviour(&ctx, msg, &group.options, &owners, &help_options).await;
        match &group_behaviour {
            HelpBehaviour::Nothing => (),
            _ => continue,
        }
        for command in group.options.commands {
            let command = *command;
            let search_command_name_matched = {
                if starts_with_whole_word(&name, &group.name) {
                    name.drain(..=group.name.len());
                }
                command.options.names.iter().find(|n| **n == name).cloned()
            };
            if search_command_name_matched.is_some() {
                match check_cmd_behaviour(ctx, msg, &command.options, &owners, &help_options).await
                {
                    HelpBehaviour::Nothing => found = Some(command),
                    _ => break,
                }
            } else if help_options.max_levenshtein_distance > 0 {
                let command_name = command.options.names[0].to_string();
                let levenshtein_distance = levenshtein_distance(&command_name, &name);
                if levenshtein_distance <= help_options.max_levenshtein_distance
                    && HelpBehaviour::Nothing
                        == check_cmd_behaviour(ctx, msg, &command.options, &owners, &help_options)
                            .await
                {
                    similar_commands.push(SuggestedCommandName {
                        name: command_name,
                        levenshtein_distance,
                    });
                }
            }
        }
        if let Some(command) = found {
            let options = &command.options;
            if !options.help_available {
                return Some(CustomisedHelpData::NoCommandFound {
                    help_error_message: &help_options.no_help_available_text,
                });
            }
            let available_text = if options.only_in == OnlyIn::Dm {
                &help_options.dm_only_text
            } else if options.only_in == OnlyIn::Guild {
                &help_options.guild_only_text
            } else {
                &help_options.dm_and_guild_text
            };
            similar_commands
                .sort_unstable_by(|a, b| a.levenshtein_distance.cmp(&b.levenshtein_distance));
            let check_names: Vec<String> = command
                .options
                .checks
                .iter()
                .chain(group.options.checks.iter())
                .filter_map(|check| {
                    if check.display_in_help {
                        Some(check.name.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            let sub_command_names: Vec<String> = options
                .sub_commands
                .iter()
                .filter_map(|cmd| {
                    if (*cmd).options.help_available {
                        Some((*cmd).options.names[0].to_string())
                    } else {
                        None
                    }
                })
                .collect();
            return Some(CustomisedHelpData::SingleCommand {
                command: CommandSimple {
                    name: options.names[0],
                    description: options.desc,
                    group_name: group.name,
                    checks: check_names,
                    aliases: options.names[1..].to_vec(),
                    availability: available_text,
                    usage: options.usage,
                    usage_sample: options.examples.to_vec(),
                    sub_commands: sub_command_names,
                },
            });
        }
    }
    None
}

fn levenshtein_distance(word_a: &str, word_b: &str) -> usize {
    let len_a = word_a.chars().count();
    let len_b = word_b.chars().count();
    if len_a == 0 {
        return len_b;
    } else if len_b == 0 {
        return len_a;
    }
    let mut matrix: Matrix<usize> = Matrix::new(len_a + 1, len_b + 1);
    for x in 0..len_a {
        matrix[(x + 1, 0)] = matrix[(x, 0)] + 1;
    }
    for y in 0..len_b {
        matrix[(0, y + 1)] = matrix[(0, y)] + 1;
    }
    for (x, char_a) in word_a.chars().enumerate() {
        for (y, char_b) in word_b.chars().enumerate() {
            matrix[(x + 1, y + 1)] = (matrix[(x, y + 1)] + 1)
                .min(matrix[(x + 1, y)] + 1)
                .min(matrix[(x, y)] + if char_a == char_b { 0 } else { 1 });
        }
    }
    matrix[(len_a, len_b)]
}

async fn fetch_single_command<'a>(
    ctx: &Context,
    msg: &Message,
    groups: &[&'static CommandGroup],
    name: &'a str,
    help_options: &'a HelpOptions,
    owners: &HashSet<UserId>,
) -> Result<CustomisedHelpData<'a>, Vec<SuggestedCommandName>> {
    let mut similar_commands: Vec<SuggestedCommandName> = Vec::new();
    let mut name = name.to_string();
    nested_group_command_search(
        ctx,
        msg,
        &groups,
        &mut name,
        &help_options,
        &mut similar_commands,
        &owners,
    )
    .await
    .ok_or(similar_commands)
}

async fn create_single_group(
    ctx: &Context,
    msg: &Message,
    group: &CommandGroup,
    owners: &HashSet<UserId>,
    help_options: &HelpOptions,
) -> GroupCommandsPair {
    let mut group_with_cmds = fetch_all_eligible_commands_in_group(
        ctx,
        &msg,
        &group.options.commands,
        &owners,
        &help_options,
        &group,
        HelpBehaviour::Nothing,
    )
    .await;
    group_with_cmds.name = group.name;
    group_with_cmds
}

async fn fetch_all_eligible_commands_in_group<'rec, 'a: 'rec>(
    ctx: &'rec Context,
    msg: &'rec Message,
    commands: &'rec [&'static InternalCommand],
    owners: &'rec HashSet<UserId>,
    help_options: &'a HelpOptions,
    group: &'a CommandGroup,
    highest_formatter: HelpBehaviour,
) -> GroupCommandsPair {
    let mut group_with_cmds = GroupCommandsPair::default();
    group_with_cmds.name = group.name;
    group_with_cmds.prefixes = group.options.prefixes.to_vec();
    let group_behaviour = {
        if let HelpBehaviour::Hide = highest_formatter {
            HelpBehaviour::Hide
        } else {
            std::cmp::max(
                highest_formatter,
                check_common_behaviour(&ctx, msg, &group.options, owners, help_options).await,
            )
        }
    };
    for command in commands {
        let command = *command;
        let options = &command.options;
        let name = &options.names[0];
        match &group_behaviour {
            HelpBehaviour::Nothing => (),
            _ => {
                let name = format_command_name!(&group_behaviour, &name);
                group_with_cmds.command_names.push(name);

                continue;
            }
        }
        let command_behaviour =
            check_cmd_behaviour(ctx, msg, &command.options, owners, help_options).await;
        let name = format_command_name!(command_behaviour, &name);
        group_with_cmds.command_names.push(name);
    }
    group_with_cmds
}

#[inline]
fn starts_with_whole_word(search_on: &str, word: &str) -> bool {
    search_on.starts_with(word)
        && search_on
            .get(word.len()..=word.len())
            .map_or(false, |slice| slice == " ")
}

fn flatten_group_to_string(
    group_text: &mut String,
    group: &GroupCommandsPair,
    nest_level: usize,
    help_options: &HelpOptions,
) {
    if nest_level > 0 {
        let _ = writeln!(group_text, "__**{}**__", group.name,);
    }
    for name in group.command_names.iter() {
        let _ = writeln!(group_text, "{}", name);
    }
    for sub_group in &group.sub_groups {
        if !(sub_group.command_names.is_empty() && sub_group.sub_groups.is_empty()) {
            let mut sub_group_text = String::default();
            flatten_group_to_string(
                &mut sub_group_text,
                &sub_group,
                nest_level + 1,
                &help_options,
            );
            let _ = write!(group_text, "{}", sub_group_text);
        }
    }
}

async fn check_cmd_behaviour<'a>(
    ctx: &'a Context,
    msg: &'a Message,
    options: &'a CommandOptions,
    owners: &'a HashSet<UserId>,
    help_options: &'a HelpOptions,
) -> HelpBehaviour {
    let b = check_common_behaviour(&ctx.cache, msg, &options, owners, help_options).await;
    if b == HelpBehaviour::Nothing {
        for check in options.checks {
            if !check.check_in_help {
                break;
            }
            let mut args = Args::new("", &[]);
            if let CheckResult::Failure(_) = (check.function)(ctx, msg, &mut args, options).await {
                return help_options.lacking_conditions;
            }
        }
    }
    b
}

async fn has_correct_permissions(
    cache: impl AsRef<Cache>,
    options: &impl CommonOptions,
    message: &Message,
) -> bool {
    if options.required_permissions().is_empty() {
        true
    } else if let Some(guild) = message.guild(&cache).await {
        let perms = guild
            .user_permissions_in(message.channel_id, message.author.id)
            .await;
        perms.contains(*options.required_permissions())
    } else {
        false
    }
}

fn has_correct_roles(
    options: &impl CommonOptions,
    roles: &HashMap<RoleId, Role>,
    member: &Member,
) -> bool {
    if options.allowed_roles().is_empty() {
        true
    } else {
        options
            .allowed_roles()
            .iter()
            .flat_map(|r| roles.values().find(|role| *r == role.name))
            .any(|g| member.roles.contains(&g.id))
    }
}
