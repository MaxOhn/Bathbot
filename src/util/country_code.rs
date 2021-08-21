use crate::{util::CowUtils, Context};

use hashbrown::HashMap;
use smallstr::SmallString;
use std::{borrow::Borrow, fmt, ops::Deref};

lazy_static! {
    static ref COUNTRIES: HashMap<&'static str, SmallString<[u8; 2]>> = {
        let mut map = hashbrown::HashMap::with_capacity(300);

        map.insert("afghanistan", "AF".into());
        map.insert("albania", "AL".into());
        map.insert("algeria", "DZ".into());
        map.insert("american samoa", "AS".into());
        map.insert("andorra", "AD".into());
        map.insert("angola", "AO".into());
        map.insert("anguilla", "AI".into());
        map.insert("antarctica", "AQ".into());
        map.insert("antigua", "AG".into());
        map.insert("barbuda", "AG".into());
        map.insert("argentina", "AR".into());
        map.insert("armenia", "AM".into());
        map.insert("aruba", "AW".into());
        map.insert("australia", "AU".into());
        map.insert("austria", "AT".into());
        map.insert("azerbaijan", "AZ".into());
        map.insert("bahamas", "BS".into());
        map.insert("bahrain", "BH".into());
        map.insert("bangladesh", "BD".into());
        map.insert("barbados", "BB".into());
        map.insert("belarus", "BY".into());
        map.insert("belgium", "BE".into());
        map.insert("belize", "BZ".into());
        map.insert("benin", "BJ".into());
        map.insert("bermuda", "BM".into());
        map.insert("bhutan", "BT".into());
        map.insert("bolivia", "BO".into());
        map.insert("bonaire", "BQ".into());
        map.insert("sint eustatius", "BQ".into());
        map.insert("saba", "BQ".into());
        map.insert("bosnia and herzegovina", "BA".into());
        map.insert("botswana", "BW".into());
        map.insert("bouvet island", "BV".into());
        map.insert("brazil", "BR".into());
        map.insert("british indian ocean territory", "IO".into());
        map.insert("brunei darussalam", "BN".into());
        map.insert("bulgaria", "BG".into());
        map.insert("burkina faso", "BF".into());
        map.insert("burundi", "BI".into());
        map.insert("cabo verde", "CV".into());
        map.insert("cambodia", "KH".into());
        map.insert("cameroon", "CM".into());
        map.insert("canada", "CA".into());
        map.insert("cayman islands", "KY".into());
        map.insert("central african republic", "CF".into());
        map.insert("chad", "TD".into());
        map.insert("chile", "CL".into());
        map.insert("china", "CN".into());
        map.insert("christmas island", "CX".into());
        map.insert("cocos islands", "CC".into());
        map.insert("colombia", "CO".into());
        map.insert("comoros", "KM".into());
        map.insert("democratic republic of congo", "CD".into());
        map.insert("democratic republic of the congo", "CD".into());
        map.insert("congo", "CG".into());
        map.insert("cook islands", "CK".into());
        map.insert("costa rica", "CR".into());
        map.insert("croatia", "HR".into());
        map.insert("cuba", "CU".into());
        map.insert("curaçao", "CW".into());
        map.insert("cyprus", "CY".into());
        map.insert("czechia", "CZ".into());
        map.insert("côte d'ivoire", "CI".into());
        map.insert("denmark", "DK".into());
        map.insert("djibouti", "DJ".into());
        map.insert("dominica", "DM".into());
        map.insert("dominican republic", "DO".into());
        map.insert("ecuador", "EC".into());
        map.insert("egypt", "EG".into());
        map.insert("el salvador", "SV".into());
        map.insert("equatorial guinea", "GQ".into());
        map.insert("eritrea", "ER".into());
        map.insert("estonia", "EE".into());
        map.insert("eswatini", "SZ".into());
        map.insert("ethiopia", "ET".into());
        map.insert("falkland islands", "FK".into());
        map.insert("malvinas", "FK".into());
        map.insert("faroe islands", "FO".into());
        map.insert("fiji", "FJ".into());
        map.insert("finland", "FI".into());
        map.insert("france", "FR".into());
        map.insert("french guiana", "GF".into());
        map.insert("french polynesia", "PF".into());
        map.insert("french southern territories", "TF".into());
        map.insert("gabon", "GA".into());
        map.insert("gambia", "GM".into());
        map.insert("georgia", "GE".into());
        map.insert("germany", "DE".into());
        map.insert("ghana", "GH".into());
        map.insert("gibraltar", "GI".into());
        map.insert("greece", "GR".into());
        map.insert("greenland", "GL".into());
        map.insert("grenada", "GD".into());
        map.insert("guadeloupe", "GP".into());
        map.insert("guam", "GU".into());
        map.insert("guatemala", "GT".into());
        map.insert("guernsey", "GG".into());
        map.insert("guinea", "GN".into());
        map.insert("guinea-bissau", "GW".into());
        map.insert("guyana", "GY".into());
        map.insert("haiti", "HT".into());
        map.insert("heard island", "HM".into());
        map.insert("mcdonald islands", "HM".into());
        map.insert("holy see", "VA".into());
        map.insert("honduras", "HN".into());
        map.insert("hong Kong", "HK".into());
        map.insert("hungary", "HU".into());
        map.insert("iceland", "IS".into());
        map.insert("india", "IN".into());
        map.insert("indonesia", "ID".into());
        map.insert("iran", "IR".into());
        map.insert("iraq", "IQ".into());
        map.insert("ireland", "IE".into());
        map.insert("isle of man", "IM".into());
        map.insert("israel", "IL".into());
        map.insert("italy", "IT".into());
        map.insert("jamaica", "JM".into());
        map.insert("japan", "JP".into());
        map.insert("jersey", "JE".into());
        map.insert("jordan", "JO".into());
        map.insert("kazakhstan", "KZ".into());
        map.insert("kenya", "KE".into());
        map.insert("kiribati", "KI".into());
        map.insert("north korea", "KP".into());
        map.insert("south korea", "KR".into());
        map.insert("kuwait", "KW".into());
        map.insert("kyrgyzstan", "KG".into());
        map.insert("lao people's democratic republic", "LA".into());
        map.insert("latvia", "LV".into());
        map.insert("lebanon", "LB".into());
        map.insert("lesotho", "LS".into());
        map.insert("liberia", "LR".into());
        map.insert("libya", "LY".into());
        map.insert("liechtenstein", "LI".into());
        map.insert("lithuania", "LT".into());
        map.insert("luxembourg", "LU".into());
        map.insert("macao", "MO".into());
        map.insert("madagascar", "MG".into());
        map.insert("malawi", "MW".into());
        map.insert("malaysia", "MY".into());
        map.insert("maldives", "MV".into());
        map.insert("mali", "ML".into());
        map.insert("malta", "MT".into());
        map.insert("marshall islands", "MH".into());
        map.insert("martinique", "MQ".into());
        map.insert("mauritania", "MR".into());
        map.insert("mauritius", "MU".into());
        map.insert("mayotte", "YT".into());
        map.insert("mexico", "MX".into());
        map.insert("micronesia", "FM".into());
        map.insert("moldova", "MD".into());
        map.insert("monaco", "MC".into());
        map.insert("mongolia", "MN".into());
        map.insert("montenegro", "ME".into());
        map.insert("montserrat", "MS".into());
        map.insert("morocco", "MA".into());
        map.insert("mozambique", "MZ".into());
        map.insert("myanmar", "MM".into());
        map.insert("namibia", "NA".into());
        map.insert("nauru", "NR".into());
        map.insert("nepal", "NP".into());
        map.insert("the netherlands", "NL".into());
        map.insert("netherlands", "NL".into());
        map.insert("new caledonia", "NC".into());
        map.insert("new zealand", "NZ".into());
        map.insert("nicaragua", "NI".into());
        map.insert("niger", "NE".into());
        map.insert("nigeria", "NG".into());
        map.insert("niue", "NU".into());
        map.insert("norfolk island", "NF".into());
        map.insert("northern mariana islands", "MP".into());
        map.insert("norway", "NO".into());
        map.insert("oman", "OM".into());
        map.insert("pakistan", "PK".into());
        map.insert("palau", "PW".into());
        map.insert("palestine", "PS".into());
        map.insert("panama", "PA".into());
        map.insert("papua new guinea", "PG".into());
        map.insert("paraguay", "PY".into());
        map.insert("peru", "PE".into());
        map.insert("philippines", "PH".into());
        map.insert("pitcairn", "PN".into());
        map.insert("poland", "PL".into());
        map.insert("portugal", "PT".into());
        map.insert("puerto rico", "PR".into());
        map.insert("qatar", "QA".into());
        map.insert("north macedonia", "MK".into());
        map.insert("romania", "RO".into());
        map.insert("russian federation", "RU".into());
        map.insert("rwanda", "RW".into());
        map.insert("réunion", "RE".into());
        map.insert("saint barthélemy", "BL".into());
        map.insert("saint helena", "SH".into());
        map.insert("saint kitts and nevis", "KN".into());
        map.insert("saint lucia", "LC".into());
        map.insert("saint martin", "MF".into());
        map.insert("saint pierre", "PM".into());
        map.insert("miquelon", "PM".into());
        map.insert("saint vincent", "VC".into());
        map.insert("grenadines", "VC".into());
        map.insert("samoa", "WS".into());
        map.insert("san marino", "SM".into());
        map.insert("sao tome and principe", "ST".into());
        map.insert("saudi arabia", "SA".into());
        map.insert("senegal", "SN".into());
        map.insert("serbia", "RS".into());
        map.insert("seychelles", "SC".into());
        map.insert("sierra leone", "SL".into());
        map.insert("singapore", "SG".into());
        map.insert("sint maarten", "SX".into());
        map.insert("slovakia", "SK".into());
        map.insert("slovenia", "SI".into());
        map.insert("solomon islands", "SB".into());
        map.insert("somalia", "SO".into());
        map.insert("south africa", "ZA".into());
        map.insert("south georgia", "GS".into());
        map.insert("south sandwich islands", "GS".into());
        map.insert("south sudan", "SS".into());
        map.insert("spain", "ES".into());
        map.insert("sri lanka", "LK".into());
        map.insert("sudan", "SD".into());
        map.insert("suriname", "SR".into());
        map.insert("svalbard", "SJ".into());
        map.insert("jan mayen", "SJ".into());
        map.insert("sweden", "SE".into());
        map.insert("switzerland", "CH".into());
        map.insert("syrian arab republic", "SY".into());
        map.insert("taiwan", "TW".into());
        map.insert("tajikistan", "TJ".into());
        map.insert("tanzania", "TZ".into());
        map.insert("thailand", "TH".into());
        map.insert("timor-leste", "TL".into());
        map.insert("togo", "TG".into());
        map.insert("tokelau", "TK".into());
        map.insert("tonga", "TO".into());
        map.insert("trinidad and tobago", "TT".into());
        map.insert("tunisia", "TN".into());
        map.insert("turkey", "TR".into());
        map.insert("turkmenistan", "TM".into());
        map.insert("turks and caicos islands", "TC".into());
        map.insert("tuvalu", "TV".into());
        map.insert("uganda", "UG".into());
        map.insert("ukraine", "UA".into());
        map.insert("united arab emirates", "AE".into());
        map.insert("united kingdom", "GB".into());
        map.insert("uk", "GB".into());
        map.insert("great britain", "GB".into());
        map.insert("united states minor outlying islands", "UM".into());
        map.insert("united states of america", "US".into());
        map.insert("usa", "US".into());
        map.insert("united states", "US".into());
        map.insert("uruguay", "UY".into());
        map.insert("uzbekistan", "UZ".into());
        map.insert("vanuatu", "VU".into());
        map.insert("venezuela ", "VE".into());
        map.insert("viet nam", "VN".into());
        map.insert("virgin islands (british)", "VG".into());
        map.insert("virgin islands (u.s.)", "VI".into());
        map.insert("wallis and futuna", "WF".into());
        map.insert("western sahara", "EH".into());
        map.insert("yemen", "YE".into());
        map.insert("zambia", "ZM".into());
        map.insert("zimbabwe", "ZW".into());

        map
    };
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct CountryCode(SmallString<[u8; 2]>);

impl CountryCode {
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.cow_to_ascii_lowercase().as_ref();

        COUNTRIES.get(name).cloned().map(Self)
    }

    pub fn snipe_supported(&self, ctx: &Context) -> bool {
        ctx.contains_country(self.0.as_str())
    }
}

impl Deref for CountryCode {
    type Target = SmallString<[u8; 2]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for CountryCode {
    fn from(code: String) -> Self {
        Self(code.into())
    }
}

impl From<&str> for CountryCode {
    fn from(code: &str) -> Self {
        Self(code.into())
    }
}

impl Borrow<str> for CountryCode {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for CountryCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
