use std::{cmp::Ordering, fmt::Write};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use bathbot_macros::command;
use bathbot_psql::model::configs::GuildConfig;
use bathbot_util::{constants::GENERAL_ISSUE, matcher, MessageBuilder};
use eyre::Result;
use once_cell::sync::OnceCell;
use twilight_model::guild::Permissions;

use crate::{core::commands::checks::check_authority, util::ChannelExt, Context};

#[command]
#[desc("Change my prefixes for a server")]
#[help(
    "Change my prefixes for a server.\n\
    To check the current prefixes for this server, \
    don't pass any arguments.\n\
    Otherwise, the first argument must be either `add` or `remove`.\n\
    Following that must be a space-separated list of \
    characters or strings you want to add or remove as prefix.\n\
    Servers must have between one and five prefixes."
)]
#[usage("[add / remove] [prefix]")]
#[example("add $ 🍆 new_pref", "remove < !!")]
#[alias("prefixes")]
#[flags(ONLY_GUILDS, SKIP_DEFER)] // authority check is done manually
#[group(Utility)]
async fn prefix_prefix(
    msg: &Message,
    mut args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let guild_id = msg.guild_id.unwrap();

    let Some(action) = args.next() else {
        let mut content = String::new();

        let f = |config: &GuildConfig| current_prefixes(&mut content, &config.prefixes);
        Context::guild_config().peek(guild_id, f).await;

        let builder = MessageBuilder::new().embed(content);
        msg.create_message(builder, permissions).await?;

        return Ok(());
    };

    match check_authority(msg.author.id, msg.guild_id).await {
        Ok(None) => {}
        Ok(Some(content)) => {
            msg.error(content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = msg.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to check authority status"));
        }
    }

    let action = match action {
        "add" | "a" => Action::Add,
        "remove" | "r" => Action::Remove,
        other => {
            let content = format!(
                "If any arguments are provided, the first one \
                must be either `add` or `remove`, not `{other}`"
            );

            msg.error(content).await?;

            return Ok(());
        }
    };

    let mut args: Vec<String> = args.take(5).map(String::from).collect();

    if args.is_empty() {
        let content = "After the first argument you should specify some prefix(es)";
        msg.error(content).await?;

        return Ok(());
    }

    if args.iter().any(|arg| matcher::is_custom_emote(arg)) {
        let content = "Does not work with custom emotes unfortunately \\:(";
        msg.error(content).await?;

        return Ok(());
    }

    enum UpdateResult {
        Ok,
        FullCapacity,
    }

    let update_fut = Context::guild_config().update(guild_id, |config| match action {
        Action::Add => {
            args.retain(|prefix| PrefixValidator::is_valid(prefix));

            let remaining_len = PREFIX_LIMIT - config.prefixes.len();

            if remaining_len == 0 {
                return UpdateResult::FullCapacity;
            }

            args.truncate(remaining_len);
            config.prefixes.extend(args);

            config.prefixes.sort_unstable_by(|a, b| {
                if a == GuildConfig::DEFAULT_PREFIX {
                    Ordering::Less
                } else if b == GuildConfig::DEFAULT_PREFIX {
                    Ordering::Greater
                } else {
                    a.cmp(b)
                }
            });

            config.prefixes.dedup();

            UpdateResult::Ok
        }
        Action::Remove => {
            for arg in args {
                config.prefixes.retain(|p| p != &arg);
            }

            if config.prefixes.is_empty() {
                config.prefixes.push(GuildConfig::DEFAULT_PREFIX.into());
            }

            UpdateResult::Ok
        }
    });

    match update_fut.await {
        Ok(UpdateResult::Ok) => {
            let mut content = "Prefixes updated!\n".to_owned();

            let f = |config: &GuildConfig| current_prefixes(&mut content, &config.prefixes);

            Context::guild_config().peek(guild_id, f).await;

            let builder = MessageBuilder::new().embed(content);
            msg.create_message(builder, permissions).await?;

            Ok(())
        }
        Ok(UpdateResult::FullCapacity) => {
            let content = format!(
                "Cannot add more prefixes, the limit of {PREFIX_LIMIT} is already reached",
            );
            msg.error(content).await?;

            Ok(())
        }
        Err(err) => {
            let _ = msg.error(GENERAL_ISSUE).await;

            Err(err.wrap_err("failed to update guild config"))
        }
    }
}

enum Action {
    Add,
    Remove,
}

fn current_prefixes(content: &mut String, prefixes: &[String]) {
    content.push_str("Prefixes for this server: ");
    let len = prefixes.iter().map(|p| p.len() + 4).sum();
    content.reserve_exact(len);
    let mut prefixes = prefixes.iter();

    if let Some(first) = prefixes.next() {
        let _ = write!(content, "`{first}`");

        for prefix in prefixes {
            let _ = write!(content, ", `{prefix}`");
        }
    }
}

struct PrefixValidator;

impl PrefixValidator {
    fn is_valid(prefix: &str) -> bool {
        !VALIDATOR
            .get_or_init(|| {
                let needles = ["ojhhf", "gbhhpu", "ijumf"]
                    .into_iter()
                    .map(String::from)
                    .map(|mut needle| {
                        unsafe { needle.as_bytes_mut() }
                            .iter_mut()
                            .for_each(|byte| *byte -= 1);

                        needle
                    });

                AhoCorasickBuilder::new()
                    .ascii_case_insensitive(true)
                    .build(needles)
                    .unwrap()
            })
            .is_match(prefix)
    }
}

static VALIDATOR: OnceCell<AhoCorasick> = OnceCell::new();

const PREFIX_LIMIT: usize = 5;
