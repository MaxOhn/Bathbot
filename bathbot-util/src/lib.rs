#[macro_use]
extern crate eyre;

mod builder;
mod cow;
mod exp_backoff;
mod hasher;
mod html_to_png;
mod matrix;

pub mod boyer_moore;
pub mod constants;
pub mod datetime;
pub mod matcher;
pub mod numbers;
pub mod osu;
pub mod string_cmp;

pub use self::{
    builder::{AuthorBuilder, EmbedBuilder, FooterBuilder, MessageBuilder, ModalBuilder},
    cow::CowUtils,
    exp_backoff::ExponentialBackoff,
    hasher::{IntHash, IntHasher},
    html_to_png::HtmlToPng,
    matrix::Matrix,
};
