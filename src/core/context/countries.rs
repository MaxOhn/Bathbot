use crate::{
    util::{CountryCode, CowUtils},
    Context,
};

impl Context {
    pub fn add_country(&self, country: String, code: CountryCode) {
        self.data.snipe_countries.insert(code, country);
    }

    pub fn contains_country(&self, code: &str) -> bool {
        self.data
            .snipe_countries
            .contains_key(code.cow_to_ascii_uppercase().as_ref())
    }

    pub fn get_country(&self, code: &str) -> Option<String> {
        self.data
            .snipe_countries
            .get(code)
            .map(|entry| entry.value().to_owned())
    }
}