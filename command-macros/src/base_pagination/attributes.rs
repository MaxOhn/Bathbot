use syn::{spanned::Spanned, Attribute, Error, Ident};

#[derive(Default)]
pub struct Attributes {
    pub jump_idx: Option<Ident>,
    pub no_multi: bool,
}

impl TryFrom<Vec<Attribute>> for Attributes {
    type Error = Error;

    fn try_from(attrs: Vec<Attribute>) -> Result<Self, Self::Error> {
        let mut jump_idx = None;
        let mut no_multi = false;

        for attr in attrs {
            match attr.path.get_ident() {
                Some(ident) if ident == "jump_idx" => jump_idx = Some(attr.parse_args()?),
                Some(ident) if ident == "pagination" => {
                    if attr.parse_args::<Ident>()? == "no_multi" {
                        no_multi = true;
                    } else {
                        return Err(Error::new(ident.span(), "Expected \"no_multi\""));
                    }
                }
                _ => {
                    let message = "Expected \"jump_idx\" or \"pagination\"";

                    return Err(Error::new(attr.path.span(), message));
                }
            }
        }

        Ok(Self { jump_idx, no_multi })
    }
}
