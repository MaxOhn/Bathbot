use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod bucket;
mod embed_data;
mod flags;
mod has_mods;
mod has_name;
mod message;
mod pagination;
mod prefix;
mod slash;
mod util;

/// Create a static SlashCommand `{uppercased_name}_SLASH`.
///
/// Make sure there is a function in scope with the signature
/// `async fn slash_{lowercased_name}(Arc<Context>, InteractionCommand) ->
/// Result<()>`
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

/// Macro to generate a corresponding builder type for pagination structs.
///
/// The last field must be `pages: Pages`.
///
/// One field needs to be denoted with `#[pagination(per_page = int)]` and next
/// to `per_page` one can also specify the attribute
/// `len = "expression that evaluates into a usize"`. If `len` is not specified,
/// it'll use `.len()` on the field that's denoted with the attribute.
///
/// The macro will provide the function `builder()`.
#[proc_macro_derive(PaginationBuilder, attributes(pagination))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match pagination::impl_derive(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
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

    let fun = parse_macro_input!(input as prefix::CommandFun);

    match prefix::fun(fun) {
        Ok(result) => result.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Create a static MessageCommand `{uppercased_name}_MSG`.
///
/// The function that's denoted with this attribute must have the signature
/// `async fn(Arc<Context>, InteractionCommand) -> Result<()>`.
///
/// Must specify `name = "..."` and optionally `dm_permission = ...` and
/// `flags(...)`.
#[proc_macro_attribute]
pub fn msg_command(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attrs = parse_macro_input!(attr as message::CommandAttrs);
    let fun = parse_macro_input!(input as message::CommandFun);

    match message::impl_cmd(attrs, fun) {
        Ok(result) => result.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
