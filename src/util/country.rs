use std::collections::HashMap;

#[derive(Debug)]
pub struct Country {
    pub name: &'static str,
    pub acronym: &'static str,
}

lazy_static::lazy_static! {
    pub static ref SNIPE_COUNTRIES: HashMap<&'static str, Country> = {
        let mut countries = HashMap::with_capacity(64);
        let mut add = |name, acronym| countries.insert(acronym, Country { name, acronym });

        add("Argentina", "AR");
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
        add("Cyprus", "CY");
        add("Czech Republic", "CZ");
        add("Ecuador", "EC");
        add("Denmark", "DK");
        add("Finland", "FI");
        add("France", "FR");
        add("Germany", "DE");
        add("Greece", "GR");
        add("Hong Kong", "HK");
        add("Indonesia", "ID");
        add("Iraq", "IQ");
        add("Ireland", "IE");
        add("Italy", "IT");
        add("Israel", "IL");
        add("Japan", "JP");
        add("Lithuania", "LT");
        add("Malaysia", "MY");
        add("Netherlands", "NL");
        add("Norway", "NO");
        add("Peru", "PE");
        add("Philippines", "PH");
        add("Poland", "PL");
        add("Portugal", "PT");
        add("Reunion", "RE");
        add("Saudi Arabia", "SA");
        add("Serbia", "RS");
        add("Singarpore", "SG");
        add("Slovakia", "SK");
        add("South Africa", "ZA");
        add("South Korea", "KR");
        add("Spain", "ES");
        add("Sweden", "SE");
        add("Taiwan", "TW");
        add("Thailand", "TH");
        add("Ukraine", "UA");
        add("United Kingdom", "GB");
        add("United States", "US");
        add("Uruguay", "UY");
        add("Venezuela", "VE");

        countries
    };
}
