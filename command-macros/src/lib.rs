use prefix::CommandFun;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod base_pagination;
mod bucket;
mod embed_data;
mod flags;
mod has_mods;
mod has_name;
mod prefix;
mod slash;
mod util;

/// Create a static SlashCommand `{uppercased_name}_SLASH`.
///
/// Make sure there is a function in scope with the signature
/// `async fn slash_{lowercased_name}(Arc<Context>, Box<ApplicationCommand>) -> BotResult<()>`
#[proc_macro_derive(SlashCommand, attributes(bucket, flags))]
pub fn slash_command(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);

    match slash::derive(derive_input) {
        Ok(result) => result.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derive the `HasName` trait which provides a `username` method.
///
/// Can only be derived on structs containing the following named fields:
/// - `name`: `Option<String>` or `Option<Cow<'_, str>>`
/// - `discord`: `Option<Id<UserMarker>>`
#[proc_macro_derive(HasName)]
pub fn has_name(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);

    match has_name::derive(derive_input) {
        Ok(result) => result.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derive the `HasMods` trait which provides a `mods` method.
///
/// Can only be derived on structs containing the following named fields:
/// - `mods`: `Option<String>` or `Option<Cow<'_, str>>`
#[proc_macro_derive(HasMods)]
pub fn has_mods(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);

    match has_mods::derive(derive_input) {
        Ok(result) => result.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derive the `EmbedData` trait which provides a `build` method.
///
/// Can only be derived on structs with any of the following field names:
/// - `author`
/// - `color`
/// - `description`
/// - `fields`
/// - `footer`
/// - `image`
/// - `timestamp`
/// - `title`
/// - `thumbnail`
/// - `url`
#[proc_macro_derive(EmbedData)]
pub fn embed_data(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);

    match embed_data::derive(derive_input) {
        Ok(result) => result.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derive the `BasePagination` trait.
///
/// Accepts the `#[pagination(...)]` and `#[jump_idx(...)]` attributes.
/// - `pagination` only accepts one argument: `no_multi`. If it is specified,
/// even if there are sufficiently many pages, it won't show the "multi_step_back"
/// and "multi_step" reactions.
/// - `jump_idx` takes one argument namely the name of a field of type `Option<usize>`.
/// If it is specified, it'll add a "my_position" reaction that jumps to the value
/// of the given field if available.
#[proc_macro_derive(BasePagination, attributes(jump_idx, pagination))]
pub fn base_pagination(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match base_pagination::derive(input) {
        Ok(result) => result.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Available attributes:
/// - `desc`: string (required)
/// - `group`: `PrefixCommandGroup` (required)
/// - `help`: string
/// - `usage`: string
/// - `aliases`: list of strings
/// - `example`: list of strings
/// - `bucket`: `BucketName`
/// - `flags`: list of  `CommandFlags`
#[proc_macro_attribute]
pub fn command(attr: TokenStream, input: TokenStream) -> TokenStream {
    if let Err(err) = prefix::attr(attr) {
        return err.into_compile_error().into();
    }

    let fun = parse_macro_input!(input as CommandFun);

    match prefix::fun(fun) {
        Ok(result) => result.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
