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
fn about(ctx: &mut Context, msg: &Message) -> CommandResult {
    let owner = ctx.http.get_current_application_info()?.owner;

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

    let cache = &ctx.cache.read();
    let name = cache.user.name.clone();
    let shards = cache.shard_count.to_string();
    let avatar = cache.user.avatar_url().unwrap();
    let users = with_comma_u64(cache.users.len() as u64);
    let guilds = with_comma_u64(cache.guilds.len() as u64);
    let channels = with_comma_u64(cache.channels.len() as u64);

    let response = {
        let data = ctx.data.read();
        let boot_time: &DateTime<Utc> = data.get::<BootTime>().expect("Could not get BootTime");
        msg.channel_id.send_message(&ctx.http, |m| {
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
                    ])
                    .footer(|f| {
                        f.text(format!("Owner: {}", owner.tag()))
                            .icon_url(owner.avatar_url().unwrap())
                    })
            })
        })?
    };

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}
