use prefix::CommandFun;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod bucket;
mod flags;
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
