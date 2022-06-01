use twilight_model::{
    application::component::{text_input::TextInputStyle, ActionRow, Component, TextInput},
    http::interaction::InteractionResponseData,
};

pub struct ModalBuilder {
    custom_id: Option<String>,
    input: TextInput,
    title: Option<String>,
}

impl ModalBuilder {
    pub fn new(component_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            custom_id: None,
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
            title: None,
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

    pub fn modal_id(mut self, custom_id: impl Into<String>) -> Self {
        self.custom_id = Some(custom_id.into());

        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.input.placeholder = Some(placeholder.into());

        self
    }

    /// Defaults to `TextInputStyle::Short`
    #[allow(unused)]
    pub fn style(mut self, style: TextInputStyle) -> Self {
        self.input.style = style;

        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());

        self
    }

    #[allow(unused)]
    /// Use this as default input. Renders the placeholder useless.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.input.value = Some(value.into());

        self
    }

    pub fn build(self) -> InteractionResponseData {
        let custom_id = self.custom_id.expect("must use `ModalBuilder::modal_id`");
        let title = self.title.expect("must use `ModalBuilder::title`");

        let row = ActionRow {
            components: vec![Component::TextInput(self.input)],
        };

        InteractionResponseData {
            components: Some(vec![Component::ActionRow(row)]),
            custom_id: Some(custom_id),
            title: Some(title),
            ..Default::default()
        }
    }
}
