use std::{collections::BTreeMap, fmt::Write};

pub use self::{
    interaction::{slash_help, Help},
    message::HELP_PREFIX,
};

mod interaction;
mod message;

fn failed_message_content(dists: BTreeMap<usize, &'static str>) -> String {
    let mut names = dists.iter().take(5).map(|(_, &name)| name);

    if let Some(name) = names.next() {
        let count = dists.len().min(5);
        let mut content = String::with_capacity(14 + count * (5 + 2) + (count - 1) * 2);
        content.push_str("Did you mean ");
        let _ = write!(content, "`{name}`");

        for name in names {
            let _ = write!(content, ", `{name}`");
        }

        content.push('?');

        content
    } else {
        "There is no such command".to_owned()
    }
}
