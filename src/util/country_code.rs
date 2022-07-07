use crate::{util::CowUtils, Context};

use chrono::FixedOffset;
use hashbrown::HashMap;
use once_cell::sync::OnceCell;
use rkyv::{
    string::{ArchivedString, StringResolver},
    Archive, Deserialize as RkyvDeserialize, Fallible, Serialize, SerializeUnsized,
};
use serde::Deserialize;
use smallstr::SmallString;
use std::{
    borrow::{Borrow, Cow},
    fmt,
    ops::Deref,
};

static TIMEZONES: OnceCell<HashMap<&'static str, i32>> = OnceCell::new();

fn timezones() -> &'static HashMap<&'static str, i32> {
    TIMEZONES.get_or_init(|| {
        const HOUR: i32 = 3600;
        const HALF_HOUR: i32 = 1800;

        let mut map = HashMap::with_capacity(250);

        // http://1min.in/content/international/time-zones
        map.insert("AF", 4 * HOUR + HALF_HOUR);
        map.insert("AL", 3 * HOUR);
        map.insert("DZ", HOUR);
        map.insert("AS", -11 * HOUR);
        map.insert("AD", 2 * HOUR);
        map.insert("AO", HOUR);
        map.insert("AI", -4 * HOUR);
        map.insert("AQ", 7 * HOUR);
        map.insert("AG", -4 * HOUR);
        map.insert("AR", -3 * HOUR);
        map.insert("AM", 4 * HOUR);
        map.insert("AW", -4 * HOUR);
        map.insert("AU", 10 * HOUR);
        map.insert("AT", 2 * HOUR);
        map.insert("AZ", 4 * HOUR);
        map.insert("BS", -4 * HOUR);
        map.insert("BH", 3 * HOUR);
        map.insert("BD", 6 * HOUR);
        map.insert("BB", -4 * HOUR);
        map.insert("BY", 3 * HOUR);
        map.insert("BE", 2 * HOUR);
        map.insert("BZ", -6 * HOUR);
        map.insert("BJ", HOUR);
        map.insert("BM", -3 * HOUR);
        map.insert("BT", 6 * HOUR);
        map.insert("BO", -4 * HOUR);
        map.insert("BQ", -4 * HOUR);
        map.insert("BA", 2 * HOUR);
        map.insert("BW", 2 * HOUR);
        // map.insert("BV", 0);
        map.insert("BR", -3 * HOUR);
        map.insert("IO", 6 * HOUR);
        map.insert("BN", 8 * HOUR);
        map.insert("BG", 3 * HOUR);
        map.insert("BF", 0);
        map.insert("BI", 2 * HOUR);
        map.insert("CV", -HOUR);
        map.insert("KH", 7 * HOUR);
        map.insert("CM", HOUR);
        map.insert("CA", -4 * HOUR);
        map.insert("KY", -5 * HOUR);
        map.insert("CF", HOUR);
        map.insert("TD", HOUR);
        map.insert("CL", -3 * HOUR);
        map.insert("CN", 8 * HOUR);
        map.insert("CX", 7 * HOUR);
        map.insert("CC", 6 * HOUR + HALF_HOUR);
        map.insert("CO", -5 * HOUR);
        map.insert("KM", 3 * HOUR);
        map.insert("CD", HOUR);
        map.insert("CG", HOUR);
        map.insert("CK", -10 * HOUR);
        map.insert("CR", -6 * HOUR);
        map.insert("HR", 2 * HOUR);
        map.insert("CU", -4 * HOUR);
        map.insert("CW", -4 * HOUR);
        map.insert("CY", 3 * HOUR);
        map.insert("CZ", 2 * HOUR);
        map.insert("CI", 0);
        map.insert("DK", 2 * HOUR);
        map.insert("DJ", 3 * HOUR);
        map.insert("DM", -4 * HOUR);
        map.insert("DO", -4 * HOUR);
        map.insert("EC", -5 * HOUR);
        map.insert("EG", 2 * HOUR);
        map.insert("SV", -6 * HOUR);
        map.insert("GQ", HOUR);
        map.insert("ER", 3 * HOUR);
        map.insert("EE", 3 * HOUR);
        map.insert("SZ", 2 * HOUR);
        map.insert("ET", 3 * HOUR);
        map.insert("FK", -3 * HOUR);
        map.insert("FO", HOUR);
        map.insert("FJ", 12 * HOUR);
        map.insert("FI", 3 * HOUR);
        map.insert("FR", 2 * HOUR);
        map.insert("GF", -3 * HOUR);
        map.insert("PF", -9 * HOUR);
        map.insert("TF", 5 * HOUR);
        map.insert("GA", HOUR);
        map.insert("GM", 0);
        map.insert("GE", 4 * HOUR);
        map.insert("DE", 2 * HOUR);
        map.insert("GH", 0);
        map.insert("GI", 2 * HOUR);
        map.insert("GR", 3 * HOUR);
        map.insert("GL", -2 * HOUR);
        map.insert("GD", -4 * HOUR);
        map.insert("GP", -4 * HOUR);
        map.insert("GU", 10 * HOUR);
        map.insert("GT", -6 * HOUR);
        map.insert("GG", HOUR);
        map.insert("GN", 0);
        map.insert("GW", 0);
        map.insert("GY", -4 * HOUR);
        map.insert("HT", -4 * HOUR);
        // map.insert("HM", HOUR);
        map.insert("VA", 2 * HOUR);
        map.insert("HN", -6 * HOUR);
        map.insert("HK", 8 * HOUR);
        map.insert("HU", 2 * HOUR);
        map.insert("IS", 0);
        map.insert("IN", 5 * HOUR + HALF_HOUR);
        map.insert("ID", 7 * HOUR);
        map.insert("IR", 4 * HOUR + HALF_HOUR);
        map.insert("IQ", 3 * HOUR);
        map.insert("IE", HOUR);
        map.insert("IM", HOUR);
        map.insert("IL", 3 * HOUR);
        map.insert("IT", 2 * HOUR);
        map.insert("JM", -5 * HOUR);
        map.insert("JP", 9 * HOUR);
        map.insert("JE", HOUR);
        map.insert("JO", 3 * HOUR);
        map.insert("KZ", 5 * HOUR);
        map.insert("KE", 3 * HOUR);
        map.insert("KI", 13 * HOUR);
        map.insert("KP", 8 * HOUR + HALF_HOUR);
        map.insert("KR", 9 * HOUR);
        map.insert("KW", 3 * HOUR);
        map.insert("KG", 6 * HOUR);
        map.insert("LA", 7 * HOUR);
        map.insert("LV", 3 * HOUR);
        map.insert("LB", 3 * HOUR);
        map.insert("LS", 2 * HOUR);
        map.insert("LR", 0);
        map.insert("LY", 2 * HOUR);
        map.insert("LI", 2 * HOUR);
        map.insert("LT", 3 * HOUR);
        map.insert("LU", 2 * HOUR);
        map.insert("MO", 8 * HOUR);
        map.insert("MG", 3 * HOUR);
        map.insert("MW", 2 * HOUR);
        map.insert("MY", 8 * HOUR);
        map.insert("MV", 5 * HOUR);
        map.insert("ML", 0);
        map.insert("MT", 2 * HOUR);
        map.insert("MH", 12 * HOUR);
        map.insert("MQ", -4 * HOUR);
        map.insert("MR", 0);
        map.insert("MU", 4 * HOUR);
        map.insert("YT", 3 * HOUR);
        map.insert("MX", -6 * HOUR);
        map.insert("FM", 11 * HOUR);
        map.insert("MD", 3 * HOUR);
        map.insert("MC", 2 * HOUR);
        map.insert("MN", 8 * HOUR);
        map.insert("ME", 2 * HOUR);
        map.insert("MS", -4 * HOUR);
        map.insert("MA", HOUR);
        map.insert("MZ", 2 * HOUR);
        map.insert("MM", 6 * HOUR + HALF_HOUR);
        map.insert("NA", 2 * HOUR);
        map.insert("NR", 12 * HOUR);
        map.insert("NP", 5 * HOUR + 2700);
        map.insert("NL", 2 * HOUR);
        map.insert("NC", 11 * HOUR);
        map.insert("NZ", 12 * HOUR);
        map.insert("NI", -6 * HOUR);
        map.insert("NE", HOUR);
        map.insert("NG", HOUR);
        map.insert("NU", -11 * HOUR);
        map.insert("NF", 11 * HOUR);
        map.insert("MP", 10 * HOUR);
        map.insert("NO", 2 * HOUR);
        map.insert("OM", 4 * HOUR);
        map.insert("PK", 5 * HOUR);
        map.insert("PW", 9 * HOUR);
        map.insert("PS", 3 * HOUR);
        map.insert("PA", -5 * HOUR);
        map.insert("PG", 10 * HOUR);
        map.insert("PY", -4 * HOUR);
        map.insert("PE", -5 * HOUR);
        map.insert("PH", 8 * HOUR);
        map.insert("PN", -8 * HOUR);
        map.insert("PL", 2 * HOUR);
        map.insert("PT", HOUR);
        map.insert("PR", -4 * HOUR);
        map.insert("QA", 3 * HOUR);
        map.insert("MK", 2 * HOUR);
        map.insert("RO", 3 * HOUR);
        map.insert("RU", 6 * HOUR);
        map.insert("RW", 2 * HOUR);
        map.insert("RE", 4 * HOUR);
        map.insert("BL", -4 * HOUR);
        map.insert("SH", 0);
        map.insert("KN", -4 * HOUR);
        map.insert("LC", -4 * HOUR);
        map.insert("MF", -4 * HOUR);
        map.insert("PM", -2 * HOUR);
        map.insert("PM", -2 * HOUR);
        map.insert("VC", -4 * HOUR);
        map.insert("WS", 13 * HOUR);
        map.insert("SM", 2 * HOUR);
        map.insert("ST", HOUR);
        map.insert("SA", 3 * HOUR);
        map.insert("SN", 0);
        map.insert("RS", 2 * HOUR);
        map.insert("SC", 4 * HOUR);
        map.insert("SL", 0);
        map.insert("SG", 8 * HOUR);
        map.insert("SX", -4 * HOUR);
        map.insert("SK", 2 * HOUR);
        map.insert("SI", 2 * HOUR);
        map.insert("SB", 11 * HOUR);
        map.insert("SO", 3 * HOUR);
        map.insert("ZA", 2 * HOUR);
        map.insert("GS", -2 * HOUR);
        map.insert("SS", 3 * HOUR);
        map.insert("ES", HOUR);
        map.insert("LK", 5 * HOUR + HALF_HOUR);
        map.insert("SD", 2 * HOUR);
        map.insert("SR", -3 * HOUR);
        map.insert("SJ", 2 * HOUR);
        map.insert("SE", 2 * HOUR);
        map.insert("CH", 2 * HOUR);
        map.insert("SY", 3 * HOUR);
        map.insert("TW", 8 * HOUR);
        map.insert("TJ", 5 * HOUR);
        map.insert("TZ", 3 * HOUR);
        map.insert("TH", 7 * HOUR);
        map.insert("TL", 9 * HOUR);
        map.insert("TG", 0);
        map.insert("TK", 13 * HOUR);
        map.insert("TO", 13 * HOUR);
        map.insert("TT", -4 * HOUR);
        map.insert("TN", HOUR);
        map.insert("TR", 3 * HOUR);
        map.insert("TM", 5 * HOUR);
        map.insert("TC", -4 * HOUR);
        map.insert("TV", 12 * HOUR);
        map.insert("UG", 3 * HOUR);
        map.insert("UA", 3 * HOUR);
        map.insert("AE", 4 * HOUR);
        map.insert("GB", HOUR);
        map.insert("UM", 12 * HOUR);
        map.insert("US", -5 * HOUR);
        map.insert("UY", -3 * HOUR);
        map.insert("UZ", 5 * HOUR);
        map.insert("VU", 11 * HOUR);
        map.insert("VE", -4 * HOUR);
        map.insert("VN", 7 * HOUR);
        map.insert("VG", -4 * HOUR);
        map.insert("VI", -4 * HOUR);
        map.insert("WF", 12 * HOUR);
        map.insert("EH", HOUR);
        map.insert("YE", 3 * HOUR);
        map.insert("ZM", 2 * HOUR);
        map.insert("ZW", 2 * HOUR);

        map
    })
}

static COUNTRIES: OnceCell<HashMap<&'static str, SmallString<[u8; 2]>>> = OnceCell::new();

fn countries() -> &'static HashMap<&'static str, SmallString<[u8; 2]>> {
    COUNTRIES.get_or_init(|| {
        let mut map = HashMap::with_capacity(300);

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
        map.insert("russia", "RU".into());
        map.insert("russian federation", "RU".into());
        map.insert("rwanda", "RW".into());
        map.insert("réunion", "RE".into());
        map.insert("reunion", "RE".into());
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
    })
}

#[derive(Clone, Debug, Deserialize, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct CountryCode(rosu_v2::prelude::CountryCode);

impl CountryCode {
    pub fn from_name(name: &str) -> Option<Self> {
        countries()
            .get(name.cow_to_ascii_lowercase().as_ref())
            .cloned()
            .map(Self)
    }

    pub fn snipe_supported(&self, ctx: &Context) -> bool {
        ctx.contains_country(self.0.as_str())
    }

    pub fn timezone(&self) -> FixedOffset {
        let offset = match timezones().get(self.0.as_str()) {
            Some(offset) => *offset,
            None => {
                warn!("missing timezone for country code {self}");

                0
            }
        };

        FixedOffset::east(offset)
    }
}

impl Deref for CountryCode {
    type Target = SmallString<[u8; 2]>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<rosu_v2::prelude::CountryCode> for CountryCode {
    #[inline]
    fn from(country_code: rosu_v2::prelude::CountryCode) -> Self {
        Self(country_code)
    }
}

impl From<String> for CountryCode {
    #[inline]
    fn from(code: String) -> Self {
        Self(code.into())
    }
}

impl From<&str> for CountryCode {
    #[inline]
    fn from(code: &str) -> Self {
        Self(code.into())
    }
}

impl<'a> From<Cow<'a, str>> for CountryCode {
    #[inline]
    fn from(code: Cow<'a, str>) -> Self {
        match code {
            Cow::Borrowed(code) => code.into(),
            Cow::Owned(code) => code.into(),
        }
    }
}

impl Borrow<str> for CountryCode {
    #[inline]
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for CountryCode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialEq<str> for CountryCode {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.0.eq(other)
    }
}

impl PartialEq<String> for CountryCode {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.0.eq(other)
    }
}

impl Archive for CountryCode {
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    #[inline]
    unsafe fn resolve(&self, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        ArchivedString::resolve_from_str(self.0.as_str(), pos, resolver, out);
    }
}

impl<S> Serialize<S> for CountryCode
where
    S: Fallible,
    str: SerializeUnsized<S>,
{
    #[inline]
    fn serialize(&self, s: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedString::serialize_from_str(self.0.as_str(), s)
    }
}

impl<D: Fallible> RkyvDeserialize<CountryCode, D> for ArchivedString {
    #[inline]
    fn deserialize(&self, _: &mut D) -> Result<CountryCode, <D as Fallible>::Error> {
        let inner = rosu_v2::prelude::CountryCode::from_str(self.as_str());

        Ok(CountryCode(inner))
    }
}
