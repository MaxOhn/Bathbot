use crate::{
    util::{
        datetime::how_long_ago,
        discord,
        numbers::{round, with_comma_u64},
    },
    BootTime,
};

use chrono::{DateTime, Utc};
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
    utils::Colour,
};
use sysinfo::{get_current_pid, ProcessExt, ProcessorExt, System, SystemExt};

#[command]
#[description = "Displaying some information about this bot"]
#[aliases("info")]
async fn about(ctx: &Context, msg: &Message) -> CommandResult {
    let owner = ctx.http.get_current_application_info().await?.owner;

    let (process_cpu, process_ram, total_cpu, used_ram, total_ram) = {
        let mut system = System::new_all();
        system.refresh_all();
        let pid = get_current_pid()?;
        let process = system.get_process(pid).unwrap();
        let process_cpu = round(process.cpu_usage());
        let process_ram = process.memory() / 1000;
        let processors = system.get_processors();
        let total_cpu: f32 = round(
            processors
                .iter()
                .map(ProcessorExt::get_cpu_usage)
                .sum::<f32>()
                / processors.len() as f32,
        );
        let used_ram = (system.get_used_memory() + system.get_used_swap()) / 1000;
        let total_ram = (system.get_total_memory() + system.get_total_swap()) / 1000;
        (process_cpu, process_ram, total_cpu, used_ram, total_ram)
    };

    let cache = &ctx.cache;
    let name = cache.current_user_field(|user| user.name.clone()).await;
    let shards = cache.shard_count().await.to_string();
    let avatar = cache
        .current_user_field(|user| user.avatar_url())
        .await
        .unwrap();
    let users = with_comma_u64(cache.user_count().await as u64);
    let guilds = with_comma_u64(cache.guild_count().await as u64);
    let channels = with_comma_u64(cache.guild_channel_count().await as u64);

    let response = {
        let data = ctx.data.read().await;
        let boot_time: &DateTime<Utc> = data.get::<BootTime>().unwrap();
        msg.channel_id
            .send_message(&ctx.http, |m| {
                m.embed(|e| {
                    e.title(format!("About {}", name))
                        .color(Colour::DARK_GREEN)
                        .thumbnail(avatar)
                        .fields(vec![
                            ("Guilds", guilds, true),
                            ("Users", users, true),
                            ("Channels", channels, true),
                            ("Shards", shards, true),
                            ("Process CPU", format!("{}%", process_cpu), true),
                            ("Total CPU", format!("{}%", total_cpu), true),
                            ("Boot time", how_long_ago(&boot_time), true),
                            ("Process RAM", format!("{} MB", process_ram), true),
                            ("Total RAM", format!("{}/{} MB", used_ram, total_ram), true),
                            (
                                "Github",
                                "https://github.com/MaxOhn/Bathbot".to_string(),
                                false,
                            ),
                            (
                                "Invite link",
                                "https://discordapp.com/api/oauth2/authorize?scope=bot&\
                            client_id=297073686916366336&permissions=268823616"
                                    .to_string(),
                                false,
                            ),
                        ])
                        .footer(|f| {
                            f.text(format!("Owner: {}", owner.tag()))
                                .icon_url(owner.avatar_url().unwrap())
                        })
                })
            })
            .await?
    };
    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}
