use std::borrow::Borrow;

#[derive(Clone, Debug)]
pub enum CustomisedHelpData<'a> {
    SuggestedCommands {
        help_description: String,
        suggestions: Suggestions,
    },
    GroupedCommands {
        help_description: String,
        groups: Vec<GroupCommandsPair>,
    },
    SingleCommand {
        command: CommandSimple<'a>,
    },
    NoCommandFound {
        help_error_message: &'a str,
    },
}

#[derive(Clone, Debug, Default)]
pub struct SuggestedCommandName {
    pub name: String,
    pub levenshtein_distance: usize,
}

#[derive(Clone, Debug)]
pub struct CommandSimple<'a> {
    pub name: &'static str,
    pub group_name: &'static str,
    pub sub_commands: Vec<String>,
    pub aliases: Vec<&'static str>,
    pub availability: &'a str,
    pub description: Option<&'static str>,
    pub usage: Option<&'static str>,
    pub usage_sample: Vec<&'static str>,
    pub checks: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct GroupCommandsPair {
    pub name: &'static str,
    pub prefixes: Vec<&'static str>,
    pub command_names: Vec<String>,
    pub sub_groups: Vec<GroupCommandsPair>,
}

#[derive(Clone, Debug, Default)]
pub struct Suggestions(pub Vec<SuggestedCommandName>);

impl Suggestions {
    #[inline]
    pub fn as_vec(&self) -> &Vec<SuggestedCommandName> {
        &self.0
    }

    pub fn join(&self, separator: &str) -> String {
        let mut iter = self.as_vec().iter();
        let first_iter_element = match iter.next() {
            Some(first_iter_element) => first_iter_element,
            None => return String::new(),
        };
        let size = self
            .as_vec()
            .iter()
            .fold(0, |total_size, size| total_size + size.name.len());
        let byte_len_of_sep = self.as_vec().len().saturating_sub(1) * separator.len();
        let mut result = String::with_capacity(size + byte_len_of_sep);
        result.push_str(first_iter_element.name.borrow());
        for element in iter {
            result.push_str(&*separator);
            result.push_str(element.name.borrow());
        }
        result
    }
}
