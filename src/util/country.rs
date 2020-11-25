use std::collections::HashMap;

macro_rules! country {
    ($countries:ident, $name:literal, $acronym:ident, $snipe:ident) => {
        let country = Country {
            name: $name,
            acronym: stringify!($acronym),
            snipe: stringify!($snipe).to_owned(),
        };
        $countries.insert(stringify!($acronym).to_owned(), country);
    };
    ($countries:ident, $name:literal, $acronym:ident) => {
        let country = Country {
            name: $name,
            acronym: stringify!($acronym),
            snipe: stringify!($acronym).to_lowercase(),
        };
        $countries.insert(stringify!($acronym).to_owned(), country);
    };
}

#[derive(Debug)]
pub struct Country {
    pub name: &'static str,
    pub acronym: &'static str,
    pub snipe: String,
}

lazy_static::lazy_static! {
    pub static ref SNIPE_COUNTRIES: HashMap<String, Country> = {
        let mut c = std::collections::HashMap::with_capacity(25);
        country!(c, "Australia", AU, aus);
        country!(c, "Austria", AT);
        country!(c, "Belarus", BY);
        country!(c, "Belgium", BE);
        country!(c, "Brazil", BR);
        country!(c, "Bulgaria", BG);
        country!(c, "Canada", CA);
        country!(c, "Chile", CL, chile);
        country!(c, "China", CN);
        country!(c, "Colombia", CO);
        country!(c, "Denmark", DK);
        country!(c, "Finland", FI);
        country!(c, "France", FR);
        country!(c, "Germany", DE);
        country!(c, "Greece", GR);
        country!(c, "Hong Kong", HK);
        country!(c, "Ireland", IE);
        country!(c, "Israel", IL);
        country!(c, "Japan", JP);
        country!(c, "Malaysia", MY);
        country!(c, "Netherlands", NL);
        country!(c, "Norway", NO);
        country!(c, "Poland", PL);
        country!(c, "Portugal", PT);
        country!(c, "Saudi Arabia", SA);
        country!(c, "Singarpore", SG);
        country!(c, "Slovakia", SK);
        country!(c, "South Korea", SK);
        country!(c, "Spain", ES, spain);
        country!(c, "Sweden", SE);
        country!(c, "Thailand", TH);
        country!(c, "Taiwan", TW);
        country!(c, "United Kingdom", GB, uk);
        country!(c, "United States", US, usa);
        c
    };
}
