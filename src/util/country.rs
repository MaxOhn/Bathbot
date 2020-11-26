use std::collections::HashMap;

#[derive(Debug)]
pub struct Country {
    pub name: &'static str,
    pub acronym: &'static str,
}

lazy_static::lazy_static! {
    pub static ref SNIPE_COUNTRIES: HashMap<&'static str, Country> = {
        let mut c = std::collections::HashMap::with_capacity(25);
        let mut add = |name, acronym| c.insert(acronym, Country { name, acronym });
        add("Australia", "AU");
        add("Austria", "AT");
        add("Belarus", "BY");
        add("Belgium", "BE");
        add("Brazil", "BR");
        add("Bulgaria", "BG");
        add("Canada", "CA");
        add("Chile", "CL");
        add("China", "CN");
        add("Colombia", "CO");
        add("Czech Republic", "CZ");
        add("Denmark", "DK");
        add("Finland", "FI");
        add("France", "FR");
        add("Germany", "DE");
        add("Greece", "GR");
        add("Hong Kong", "HK");
        add("Ireland", "IE");
        add("Israel", "IL");
        add("Japan", "JP");
        add("Malaysia", "MY");
        add("Netherlands", "NL");
        add("Norway", "NO");
        add("Poland", "PL");
        add("Portugal", "PT");
        add("Saudi Arabia", "SA");
        add("Singarpore", "SG");
        add("Slovakia", "SK");
        add("South Korea", "SK");
        add("Spain", "ES");
        add("Sweden", "SE");
        add("Thailand", "TH");
        add("Taiwan", "TW");
        add("United Kingdom", "GB");
        add("United States", "US");
        c
    };
}
