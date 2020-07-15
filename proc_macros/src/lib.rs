pub(crate) mod attributes;
pub(crate) mod structs;

#[macro_use]
pub(crate) mod util;

use attributes::*;
use structs::*;
use util::*;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Error, parse_macro_input, spanned::Spanned};

macro_rules! match_options {
    ($n:expr, $values:ident, $options:ident, $span:expr => [$($name:ident);*]) => {
        match $n {
            $(
                stringify!($name) => $options.$name = propagate_err!($crate::attributes::parse($values)),
            )*
            _ => {
                return Error::new($span, format_args!("invalid attribute: {}", $n))
                    .to_compile_error()
                    .into();
            },
        }
    };
}

#[proc_macro_attribute]
pub fn command(attr: TokenStream, input: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        panic!("expected `#[command]`, got #[command({})]", attr);
    }
    let mut fun = parse_macro_input!(input as CommandFun);
    let mut options = Options::default();
    for attribute in &fun.attributes {
        let values = propagate_err!(parse_values(attribute));
        let span = attribute.span();
        let name = values.name.to_string();
        let name = name.as_str();
        match name {
            "example" => {
                options
                    .examples
                    .push(propagate_err!(attributes::parse(values)));
            }
            _ => {
                match_options!(name, values, options, span => [
                    short_desc;
                    long_desc;
                    aliases;
                    usage;
                    sub_commands
                ]);
            }
        }
    }
    let Options {
        aliases,
        short_desc,
        long_desc,
        usage,
        examples,
        sub_commands,
    } = options;
    let short_desc = if let Some(short_desc) = short_desc {
        short_desc
    } else {
        panic!("require `#[short_desc(\"...\")]`")
    };
    create_declaration_validations(&mut fun);
    populate_fut_lifetimes_on_refs(&mut fun.args);
    let fun_name_str = fun.name.to_string();
    let fun_name = fun.name.clone();
    let sub_commands = sub_commands
        .into_iter()
        .map(|i| i.with_suffix("CMD"))
        .collect::<Vec<_>>();
    let cmd_name = fun.name.with_suffix("CMD");
    let command_path = quote!(crate::core::Command);
    let stream = quote! {
        pub static #cmd_name: #command_path = #command_path {
            names: &[#fun_name_str, #(#aliases),*],
            short_desc: #short_desc,
            long_desc: #long_desc,
            usage: #usage,
            examples: &[#(#examples),*],
            sub_commands: &[#(&#sub_commands),*],
            fun: #fun_name,
        };

        #fun
    };
    stream.into()
}
