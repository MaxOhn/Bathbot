/// Create or push onto a vector of embed fields.
#[macro_export]
macro_rules! fields {
    // Push fields to a vec
    ($fields:ident {
        $($name:expr, $value:expr, $inline:expr $(;)? )+
    }) => {{
        $(
            $fields.push(
                twilight_model::channel::message::embed::EmbedField {
                    name: $name.into(),
                    value: $value,
                    inline: $inline,
                }
            );
        )+
    }};

    // Create a new vec of fields
    ($($name:expr, $value:expr, $inline:expr);+) => {
        fields![$($name, $value, $inline;)+]
    };

    ($($name:expr, $value:expr, $inline:expr;)+) => {
        vec![
            $(
                twilight_model::channel::message::embed::EmbedField {
                    name: $name.into(),
                    value: $value,
                    inline: $inline,
                },
            )+
        ]
    };
}
