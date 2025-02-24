use twilight_model::{
    channel::message::{
        Component,
        component::{ActionRow, TextInput, TextInputStyle},
    },
    http::interaction::InteractionResponseData,
};

pub struct ModalBuilder {
    custom_id: String,
    inputs: Vec<TextInputBuilder>,
    title: String,
}

impl ModalBuilder {
    pub fn new(custom_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            custom_id: custom_id.into(),
            inputs: Vec::new(),
            title: title.into(),
        }
    }

    pub fn input(mut self, input: TextInputBuilder) -> Self {
        self.inputs.push(input);

        self
    }

    pub fn build(self) -> InteractionResponseData {
        let components = self
            .inputs
            .into_iter()
            .map(TextInputBuilder::build)
            .map(Component::TextInput)
            .map(|component| ActionRow {
                components: vec![component],
            })
            .map(Component::ActionRow)
            .collect();

        InteractionResponseData {
            components: Some(components),
            custom_id: Some(self.custom_id),
            title: Some(self.title),
            ..Default::default()
        }
    }
}

pub struct TextInputBuilder {
    input: TextInput,
}

impl TextInputBuilder {
    pub fn new(component_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            input: TextInput {
                custom_id: component_id.into(),
                label: label.into(),
                max_length: None,
                min_length: None,
                placeholder: None,
                required: Some(true),
                style: TextInputStyle::Short,
                value: None,
            },
        }
    }

    pub fn max_len(mut self, len: u16) -> Self {
        self.input.max_length = Some(len);

        self
    }

    pub fn min_len(mut self, len: u16) -> Self {
        self.input.min_length = Some(len);

        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.input.required = Some(required);

        self
    }

    /// Defaults to `TextInputStyle::Short`
    #[allow(unused)]
    pub fn style(mut self, style: TextInputStyle) -> Self {
        self.input.style = style;

        self
    }

    #[allow(unused)]
    /// Use this as default input. Renders the placeholder useless.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.input.value = Some(value.into());

        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.input.placeholder = Some(placeholder.into());

        self
    }

    pub fn build(self) -> TextInput {
        self.input
    }
}
