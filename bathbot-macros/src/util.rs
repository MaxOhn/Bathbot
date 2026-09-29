use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::{
    GenericArgument, Ident, LitStr, Path, PathArguments, Result, Token, Type,
    parse::{Parse, ParseStream},
};

pub struct AsOption<T>(pub Option<T>);

impl<T: ToTokens> ToTokens for AsOption<T> {
    fn to_tokens(&self, stream: &mut TokenStream2) {
        match &self.0 {
            Some(o) => stream.extend(quote!(Some(#o))),
            None => stream.extend(quote!(None)),
        }
    }
}

pub enum LitOrConst {
    Lit(LitStr),
    Const(Path),
}

impl Parse for LitOrConst {
    fn parse(input: ParseStream) -> Result<Self> {
        let lookahead = input.lookahead1();

        if lookahead.peek(LitStr) {
            input.parse().map(Self::Lit)
        } else if lookahead.peek(Ident) || lookahead.peek(Token![::]) {
            input.parse().map(Self::Const)
        } else {
            Err(lookahead.error())
        }
    }
}

impl ToTokens for LitOrConst {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self {
            LitOrConst::Lit(lit) => lit.to_tokens(tokens),
            LitOrConst::Const(path) => tokens.extend(quote! {
                {
                    const _: &str = #path;

                    #path
                }
            }),
        }
    }
}

/// Whether `ty` is `Option<String>` or `Option<Cow<'_, str>>` (ident match, not
/// resolution).
pub(crate) fn is_option_string_or_cow(ty: &Type) -> bool {
    let Type::Path(ty_path) = ty else {
        return false;
    };

    let Some(segment) = ty_path.path.segments.last() else {
        return false;
    };

    if segment.ident != "Option" {
        return false;
    }

    let PathArguments::AngleBracketed(ref args) = segment.arguments else {
        return false;
    };

    let Some(GenericArgument::Type(Type::Path(path))) = args.args.first() else {
        return false;
    };

    matches!(
        path.path.segments.first(),
        Some(seg) if seg.ident == "String" || seg.ident == "Cow"
    )
}
