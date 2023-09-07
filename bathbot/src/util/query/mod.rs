mod filter;
mod impls;
mod operator;
mod optional;
mod searchable;

pub use self::{filter::*, impls::*, searchable::*};

fn separate_content(content: &mut String) {
    if !content.is_empty() {
        content.push_str(" • ");
    }
}
