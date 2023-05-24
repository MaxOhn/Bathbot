use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
    fmt::{Display, Formatter, Result as FmtResult},
};

use bathbot_util::CowUtils;
use once_cell::sync::OnceCell;
use time::UtcOffset;

static COUNTRIES: OnceCell<Countries> = OnceCell::new();

pub struct Countries {
    name_to_code: HashMap<&'static str, &'static str>,
    code_to_name: HashMap<&'static str, &'static str>,
    code_to_timezone: HashMap<&'static str, i32>,
}

impl Countries {
    pub fn code(country_code: &str) -> Code<'_> {
        Code(country_code)
    }

    pub fn name(country_name: &str) -> Name<'_> {
        Name(country_name)
    }

    pub fn init() {
        let mut countries = Countries {
            name_to_code: HashMap::with_capacity(256),
            code_to_name: HashMap::with_capacity(256),
            code_to_timezone: HashMap::with_capacity(256),
        };

        macro_rules! insert_country {
            ( $(
                $name:literal,
                $code:literal,
                $tz:literal $( + $minutes:literal )?;
            )* ) => {
                $(
                    countries.name_to_code.insert($name, $code);

                    if let Entry::Vacant(e) = countries.code_to_name.entry($code) {
                        e.insert($name);
                        #[allow(clippy::neg_multiply)]
                        countries.code_to_timezone.insert($code, ($tz * 3600) $( + 60 * $minutes )?);
                    }
                )*
            };
        }

        insert_country! {
            "afghanistan", "AF", 4 + 30;
            "albania", "AL", 3;
            "algeria", "DZ", 1;
            "american samoa", "AS", -11;
            "andorra", "AD", 2;
            "angola", "AO", 1;
            "anguilla", "AI", -4;
            "antarctica", "AQ", 7;
            "antigua", "AG", -4;
            "barbuda", "AG", -4;
            "argentina", "AR", -3;
            "armenia", "AM", 4;
            "aruba", "AW", -4;
            "australia", "AU", 10;
            "austria", "AT", 2;
            "azerbaijan", "AZ", 4;
            "bahamas", "BS", -4;
            "bahrain", "BH", 3;
            "bangladesh", "BD", 6;
            "barbados", "BB", -4;
            "belarus", "BY", 3;
            "belgium", "BE", 2;
            "belize", "BZ", -6;
            "benin", "BJ", 1;
            "bermuda", "BM", -3;
            "bhutan", "BT", 6;
            "bolivia", "BO", -4;
            "bonaire", "BQ", -4;
            "sint eustatius", "BQ", -4;
            "saba", "BQ", -4;
            "bosnia and herzegovina", "BA", 2;
            "botswana", "BW", 2;
            "bouvet island", "BV", 0;
            "brazil", "BR", -3;
            "british indian ocean territory", "IO", 6;
            "brunei darussalam", "BN", 8;
            "bulgaria", "BG", 3;
            "burkina faso", "BF", 0;
            "burundi", "BI", 2;
            "cabo verde", "CV", -1;
            "cambodia", "KH", 7;
            "cameroon", "CM", 1;
            "canada", "CA", -4;
            "cayman islands", "KY", -5;
            "central african republic", "CF", 1;
            "chad", "TD", 1;
            "chile", "CL", -3;
            "china", "CN", 8;
            "christmas island", "CX", 7;
            "cocos islands", "CC", 6 + 30;
            "colombia", "CO", -5;
            "comoros", "KM", 3;
            "democratic republic of congo", "CD", 1;
            "democratic republic of the congo", "CD", 1;
            "congo", "CG", 1;
            "cook islands", "CK", -10;
            "costa rica", "CR", -6;
            "croatia", "HR", 2;
            "cuba", "CU", -4;
            "curaçao", "CW", -4;
            "cyprus", "CY", 3;
            "czechia", "CZ", 2;
            "côte d'ivoire", "CI", 0;
            "denmark", "DK", 2;
            "djibouti", "DJ", 3;
            "dominica", "DM", -4;
            "dominican republic", "DO", -4;
            "ecuador", "EC", -5;
            "egypt", "EG", 2;
            "el salvador", "SV", -6;
            "equatorial guinea", "GQ", 1;
            "eritrea", "ER", 3;
            "estonia", "EE", 3;
            "eswatini", "SZ", 2;
            "ethiopia", "ET", 3;
            "falkland islands", "FK", -3;
            "malvinas", "FK", -3;
            "faroe islands", "FO", 1;
            "fiji", "FJ", 12;
            "finland", "FI", 2;
            "france", "FR", 2;
            "french guiana", "GF", -3;
            "french polynesia", "PF", -9;
            "french southern territories", "TF", 5;
            "gabon", "GA", 1;
            "gambia", "GM", 0;
            "georgia", "GE", 4;
            "germany", "DE", 2;
            "ghana", "GH", 0;
            "gibraltar", "GI", 2;
            "greece", "GR", 3;
            "greenland", "GL", -2;
            "grenada", "GD", -4;
            "guadeloupe", "GP", -4;
            "guam", "GU", 10;
            "guatemala", "GT", -6;
            "guernsey", "GG", 1;
            "guinea", "GN", 0;
            "guinea-bissau", "GW", 0;
            "guyana", "GY", -4;
            "haiti", "HT", -4;
            "heard island", "HM", 1;
            "mcdonald islands", "HM", 1;
            "holy see", "VA", 2;
            "honduras", "HN", -6;
            "hong Kong", "HK", 8;
            "hungary", "HU", 2;
            "iceland", "IS", 0;
            "india", "IN", 5 + 30;
            "indonesia", "ID", 7;
            "iran", "IR", 4 + 30;
            "iraq", "IQ", 3;
            "ireland", "IE", 1;
            "isle of man", "IM", 1;
            "israel", "IL", 3;
            "italy", "IT", 2;
            "jamaica", "JM", -5;
            "japan", "JP", 9;
            "jersey", "JE", 1;
            "jordan", "JO", 3;
            "kazakhstan", "KZ", 5;
            "kenya", "KE", 3;
            "kiribati", "KI", 13;
            "north korea", "KP", 8 + 30;
            "south korea", "KR", 9;
            "kuwait", "KW", 3;
            "kyrgyzstan", "KG", 6;
            "lao people's democratic republic", "LA", 7;
            "latvia", "LV", 3;
            "lebanon", "LB", 3;
            "lesotho", "LS", 2;
            "liberia", "LR", 0;
            "libya", "LY", 2;
            "liechtenstein", "LI", 2;
            "lithuania", "LT", 3;
            "luxembourg", "LU", 2;
            "macao", "MO", 8;
            "madagascar", "MG", 3;
            "malawi", "MW", 2;
            "malaysia", "MY", 8;
            "maldives", "MV", 5;
            "mali", "ML", 0;
            "malta", "MT", 2;
            "marshall islands", "MH", 12;
            "martinique", "MQ", -4;
            "mauritania", "MR", 0;
            "mauritius", "MU", 4;
            "mayotte", "YT", 3;
            "mexico", "MX", -6;
            "micronesia", "FM", 11;
            "moldova", "MD", 3;
            "monaco", "MC", 2;
            "mongolia", "MN", 8;
            "montenegro", "ME", 2;
            "montserrat", "MS", -4;
            "morocco", "MA", 1;
            "mozambique", "MZ", 2;
            "myanmar", "MM", 6 + 30;
            "namibia", "NA", 2;
            "nauru", "NR", 12;
            "nepal", "NP", 5 + 45;
            "the netherlands", "NL", 2;
            "netherlands", "NL", 2;
            "new caledonia", "NC", 11;
            "new zealand", "NZ", 12;
            "nicaragua", "NI", -6;
            "niger", "NE", 1;
            "nigeria", "NG", 1;
            "niue", "NU", -11;
            "norfolk island", "NF", 11;
            "northern mariana islands", "MP", 10;
            "norway", "NO", 2;
            "oman", "OM", 4;
            "pakistan", "PK", 5;
            "palau", "PW", 9;
            "palestine", "PS", 3;
            "panama", "PA", -5;
            "papua new guinea", "PG", 10;
            "paraguay", "PY", -4;
            "peru", "PE", -5;
            "philippines", "PH", 8;
            "pitcairn", "PN", -8;
            "poland", "PL", 2;
            "portugal", "PT", 1;
            "puerto rico", "PR", -4;
            "qatar", "QA", 3;
            "north macedonia", "MK", 2;
            "romania", "RO", 3;
            "russia", "RU", 6;
            "russian federation", "RU", 6;
            "rwanda", "RW", 2;
            "réunion", "RE", 4;
            "reunion", "RE", 4;
            "saint barthélemy", "BL", -4;
            "saint helena", "SH", 0;
            "saint kitts and nevis", "KN", -4;
            "saint lucia", "LC", -4;
            "saint martin", "MF", -4;
            "saint pierre", "PM", -2;
            "miquelon", "PM", -2;
            "saint vincent", "VC", -4;
            "grenadines", "VC", -4;
            "samoa", "WS", 13;
            "san marino", "SM", 2;
            "sao tome and principe", "ST", 1;
            "saudi arabia", "SA", 3;
            "senegal", "SN", 0;
            "serbia", "RS", 2;
            "seychelles", "SC", 4;
            "sierra leone", "SL", 0;
            "singapore", "SG", 8;
            "sint maarten", "SX", -4;
            "slovakia", "SK", 2;
            "slovenia", "SI", 2;
            "solomon islands", "SB", 11;
            "somalia", "SO", 3;
            "south africa", "ZA", 2;
            "south georgia", "GS", -2;
            "south sandwich islands", "GS", -2;
            "south sudan", "SS", 3;
            "spain", "ES", 1;
            "sri lanka", "LK", 5 + 30;
            "sudan", "SD", 2;
            "suriname", "SR", -3;
            "svalbard", "SJ", 2;
            "jan mayen", "SJ", 2;
            "sweden", "SE", 2;
            "switzerland", "CH", 2;
            "syrian arab republic", "SY", 3;
            "taiwan", "TW", 8;
            "tajikistan", "TJ", 5;
            "tanzania", "TZ", 3;
            "thailand", "TH", 7;
            "timor-leste", "TL", 9;
            "togo", "TG", 0;
            "tokelau", "TK", 13;
            "tonga", "TO", 13;
            "trinidad and tobago", "TT", -4;
            "tunisia", "TN", 1;
            "turkey", "TR", 3;
            "turkmenistan", "TM", 5;
            "turks and caicos islands", "TC", -4;
            "tuvalu", "TV", 12;
            "uganda", "UG", 3;
            "ukraine", "UA", 3;
            "united arab emirates", "AE", 4;
            "united kingdom", "GB", 1;
            "uk", "GB", 1;
            "great britain", "GB", 1;
            "united states minor outlying islands", "UM", 12;
            "united states of america", "US", -5;
            "usa", "US", -5;
            "united states", "US", -5;
            "uruguay", "UY", -3;
            "uzbekistan", "UZ", 5;
            "vanuatu", "VU", 11;
            "venezuela ", "VE", -4;
            "vietnam", "VN", 7;
            "virgin islands (british)", "VG", -4;
            "virgin islands (u.s.)", "VI", -4;
            "wallis and futuna", "WF", 12;
            "western sahara", "EH", 1;
            "yemen", "YE", 3;
            "zambia", "ZM", 2;
            "zimbabwe", "ZW", 2;
        }

        if COUNTRIES.set(countries).is_err() {
            panic!("Countries were already set");
        }
    }
}

#[derive(Copy, Clone)]
pub struct Code<'a>(&'a str);

impl<'a> Code<'a> {
    pub fn to_name(self) -> Option<CountryName> {
        unsafe { COUNTRIES.get_unchecked() }
            .code_to_name
            .get(self.uppercase().as_ref())
            .copied()
            .map(CountryName)
    }

    pub fn to_timezone(self) -> UtcOffset {
        let offset = unsafe { COUNTRIES.get_unchecked() }
            .code_to_timezone
            .get(self.uppercase().as_ref())
            .copied()
            .unwrap_or(0);

        UtcOffset::from_whole_seconds(offset).unwrap()
    }

    fn uppercase(self) -> Cow<'a, str> {
        let Self(country_code) = self;

        country_code.cow_to_ascii_uppercase()
    }
}

pub struct CountryName(&'static str);

impl CountryName {
    pub fn ends_with(&self, c: char) -> bool {
        self.0.ends_with(c)
    }
}

impl Display for CountryName {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mut words = self.0.split(' ');

        let Some(word) = words.next() else {
            return Ok(());
        };

        let Some(first) = word.chars().next() else {
            return Ok(());
        };

        Display::fmt(&first.to_uppercase(), f)?;
        f.write_str(&word[first.len_utf8()..])?;

        for word in words {
            let Some(first) = word.chars().next() else { continue };
            write!(f, " {}", first.to_uppercase())?;
            f.write_str(&word[first.len_utf8()..])?;
        }

        Ok(())
    }
}

#[derive(Copy, Clone)]
pub struct Name<'a>(&'a str);

impl<'a> Name<'a> {
    pub fn to_code(self) -> Option<&'static str> {
        unsafe { COUNTRIES.get_unchecked() }
            .name_to_code
            .get(self.lowercase().as_ref())
            .copied()
    }

    fn lowercase(self) -> Cow<'a, str> {
        let Self(country_name) = self;

        country_name.cow_to_ascii_lowercase()
    }
}
