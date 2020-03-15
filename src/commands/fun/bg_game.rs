use crate::{BgListenerKey, DispatchEvent, DispatcherKey};

use hey_listen::{
    sync::{ParallelDispatcherRequest as DispatcherRequest, ParallelListener as Listener},
    RwLock,
};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::sync::Arc;

#[command]
#[description = "Background game coming eventually:tm:"]
#[aliases("bg")]
fn backgroundgame(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    if args.is_empty() {
        let new_listener = Arc::new(RwLock::new(BgListener::new()));
        let dispatcher = {
            let mut data = ctx.data.write();
            let listeners = data
                .get_mut::<BgListenerKey>()
                .expect("Could not get ListenerKey");
            listeners.insert(msg.channel_id, new_listener.clone());
            data.get_mut::<DispatcherKey>()
                .expect("Could not get DispatcherKey")
                .clone()
        };
        dispatcher.write().add_listener(
            DispatchEvent::BgMsgEvent {
                channel: msg.channel_id,
                user: msg.author.id,
                content: String::new(),
            },
            &new_listener,
        );
        msg.channel_id.say(&ctx.http, "started listening")?;
    } else {
        let dispatcher = {
            let mut data = ctx.data.write();
            data.get_mut::<DispatcherKey>()
                .expect("Could not get DispatcherKey")
                .clone()
        };
        dispatcher
            .write()
            .dispatch_event(&DispatchEvent::BgMsgEvent {
                channel: msg.channel_id,
                user: msg.author.id,
                content: String::from("stop"),
            });
        {
            let mut data = ctx.data.write();
            let listeners = data
                .get_mut::<BgListenerKey>()
                .expect("Could not get ListenerKey");
            let _ = listeners.remove(&msg.channel_id);
        }
        msg.channel_id.say(&ctx.http, "stopped listening (?)")?;
    }
    Ok(())
}

pub struct BgListener {
    _title: String,
    _artist: String,
    _hint_level: u8,
}

impl Listener<DispatchEvent> for BgListener {
    fn on_event(&mut self, event: &DispatchEvent) -> Option<DispatcherRequest> {
        match event {
            DispatchEvent::BgMsgEvent {
                channel,
                user,
                content,
            } => {
                println!("> got event {} ~ {} ~ {}", channel, user, content);
                // TODO: Check content
                if content.as_str() == "stop" {
                    println!("stop listening");
                    Some(DispatcherRequest::StopListening)
                } else {
                    println!("event content: {}", content);
                    None
                }
            }
        }
    }
}

impl BgListener {
    fn new() -> Self {
        Self {
            _title: String::new(),
            _artist: String::new(),
            _hint_level: 0,
        }
    }
}
