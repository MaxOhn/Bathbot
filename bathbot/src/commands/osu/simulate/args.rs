use std::{borrow::Cow, str::FromStr};

use nom::{
    branch::alt,
    bytes::complete as by,
    character::complete as ch,
    combinator::{all_consuming, map, map_parser, map_res, opt, recognize, success},
    error::{Error as NomError, ErrorKind as NomErrorKind},
    multi::many1_count,
    number::complete as num,
    sequence::{delimited, preceded, terminated, tuple},
    Err as NomErr, IResult, Parser,
};
use rosu_v2::prelude::GameMods;

#[derive(Debug, PartialEq)]
pub enum SimulateArg {
    Acc(f32),
    Combo(u32),
    ClockRate(f32),
    N300(u32),
    N100(u32),
    N50(u32),
    Geki(u32),
    Katu(u32),
    Miss(u32),
    Mods(GameMods),
    Ar(f32),
    Cs(f32),
    Hp(f32),
    Od(f32),
}

impl SimulateArg {
    pub fn parse(input: &str) -> Result<Self, ParseError<'_>> {
        let (rest, key_opt) = parse_key(input).map_err(|_| ParseError::Nom(input))?;

        match key_opt {
            None => parse_any(rest),
            Some("acc" | "a" | "accuracy") => parse_acc(rest).map(SimulateArg::Acc),
            Some("combo" | "c") => parse_combo(rest).map(SimulateArg::Combo),
            Some("clockrate" | "cr") => parse_clock_rate(rest).map(SimulateArg::ClockRate),
            Some("n300") => parse_n300(rest).map(SimulateArg::N300),
            Some("n100") => parse_n100(rest).map(SimulateArg::N100),
            Some("n50") => parse_n50(rest).map(SimulateArg::N50),
            Some("mods") => parse_mods(rest).map(SimulateArg::Mods),
            Some("ar") => parse_ar(rest).map(SimulateArg::Ar),
            Some("cs") => parse_cs(rest).map(SimulateArg::Cs),
            Some("hp") => parse_hp(rest).map(SimulateArg::Hp),
            Some("od") => parse_od(rest).map(SimulateArg::Od),
            Some(key) => {
                let (sub_n, _) = opt::<_, _, NomError<_>, _>(ch::char('n'))(key)
                    .map_err(|_| ParseError::Nom(input))?;

                match sub_n {
                    "miss" | "m" | "misses" => parse_miss(rest).map(SimulateArg::Miss),
                    "geki" | "gekis" | "320" => parse_geki(rest).map(SimulateArg::Geki),
                    "katu" | "katus" | "200" => parse_katu(rest).map(SimulateArg::Katu),
                    _ => Err(ParseError::Unknown(key)),
                }
            }
        }
    }
}

fn parse_key(input: &str) -> IResult<&str, Option<&str>> {
    opt(terminated(ch::alphanumeric1, ch::char('=')))(input)
}

fn parse_any(input: &str) -> Result<SimulateArg, ParseError<'_>> {
    fn inner(input: &str) -> IResult<&str, SimulateArg> {
        enum ParseAny {
            Float(f32),
            Int(u32),
            Mods(GameMods),
            Ar(f32),
            Cs(f32),
            Hp(f32),
            Od(f32),
        }

        let float = map(map_res(recognize_float, str::parse), ParseAny::Float);
        let int = map(ch::u32, ParseAny::Int);
        let mods = map(parse_mods_force_prefix, ParseAny::Mods);
        let ar = map(preceded(by::tag("ar"), num::float), ParseAny::Ar);
        let cs = map(preceded(by::tag("cs"), num::float), ParseAny::Cs);
        let hp = map(preceded(by::tag("hp"), num::float), ParseAny::Hp);
        let od = map(preceded(by::tag("od"), num::float), ParseAny::Od);
        let (rest, num) = alt((float, int, mods, ar, cs, hp, od))(input)?;

        match num {
            ParseAny::Float(n) => {
                let acc = map(recognize_acc, |_| SimulateArg::Acc(n));
                let clock_rate = map(recognize_clock_rate, |_| SimulateArg::ClockRate(n));

                all_consuming(alt((acc, clock_rate)))(rest)
            }
            ParseAny::Int(n) => {
                let acc = map(recognize_acc, |_| SimulateArg::Acc(n as f32));
                let combo = map(recognize_combo, |_| SimulateArg::Combo(n));
                let clock_rate = map(ch::char('*'), |_| SimulateArg::ClockRate(n as f32));
                let n300 = map(recognize_n300, |_| SimulateArg::N300(n));
                let n100 = map(recognize_n100, |_| SimulateArg::N100(n));
                let n50 = map(recognize_n50, |_| SimulateArg::N50(n));
                let geki = map(recognize_geki, |_| SimulateArg::Geki(n));
                let katu = map(recognize_katu, |_| SimulateArg::Katu(n));
                let miss = map(recognize_miss, |_| SimulateArg::Miss(n));
                let options = (acc, combo, clock_rate, n300, n100, n50, geki, katu, miss);

                all_consuming(alt(options))(rest)
            }
            ParseAny::Mods(mods) => Ok((rest, SimulateArg::Mods(mods))),
            ParseAny::Ar(n) => Ok((rest, SimulateArg::Ar(n))),
            ParseAny::Cs(n) => Ok((rest, SimulateArg::Cs(n))),
            ParseAny::Hp(n) => Ok((rest, SimulateArg::Hp(n))),
            ParseAny::Od(n) => Ok((rest, SimulateArg::Od(n))),
        }
    }

    inner(input)
        .map(|(_, val)| val)
        .map_err(|_| ParseError::Nom(input))
}

fn parse_int<'i, F>(input: &'i str, suffix: F) -> IResult<&'i str, u32>
where
    F: Parser<&'i str, (), NomError<&'i str>>,
{
    all_consuming(terminated(ch::u32, opt(suffix)))(input)
}

fn parse_float<'i, F>(input: &'i str, suffix: F) -> IResult<&'i str, f32>
where
    F: Parser<&'i str, (), NomError<&'i str>>,
{
    all_consuming(terminated(num::float, opt(suffix)))(input)
}

macro_rules! parse_arg {
    ( $( $fn:ident -> $ty:ty: $parse:ident, $recognize:ident $( or $x:literal )?, $err:ident; )* ) => {
        $(
            fn $fn(input: &str) -> Result<$ty, ParseError<'_>> {
                let recognize = alt((
                    map($recognize, |_| ()),
                    $( map(ch::char($x), |_| ()) )?
                ));

                $parse(input, recognize)
                    .map(|(_, val)| val)
                    .map_err(|_| ParseError::$err)
            }
        )*
    };
}

parse_arg! {
    parse_acc -> f32: parse_float, recognize_acc, Acc;
    parse_combo -> u32: parse_int, recognize_combo, Combo;
    parse_clock_rate -> f32: parse_float, recognize_clock_rate, ClockRate;
    parse_n300 -> u32: parse_int, recognize_n300 or 'x', N300;
    parse_n100 -> u32: parse_int, recognize_n100 or 'x', N100;
    parse_n50 -> u32: parse_int, recognize_n50 or 'x', N50;
    parse_miss -> u32: parse_int, recognize_miss or 'x', Miss;
    parse_geki -> u32: parse_int, recognize_geki or 'x', Geki;
    parse_katu -> u32: parse_int, recognize_katu or 'x', Katu;
}

macro_rules! parse_attr_arg {
    ( $( $fn:ident: $err:ident; ) *) => {
        $(
            fn $fn(input: &str) -> Result<f32, ParseError<'_>> {
                parse_float(input, success(()))
                    .map(|(_, val)| val)
                    .map_err(|_| ParseError::$err)
            }
        )*
    }
}

parse_attr_arg! {
    parse_ar: Ar;
    parse_cs: Cs;
    parse_hp: Hp;
    parse_od: Od;
}

fn is_some<T>(opt: Option<T>) -> bool {
    opt.is_some()
}

fn parse_mods_force_prefix(input: &str) -> IResult<&str, GameMods> {
    let (rest, (prefixed, mods, _)) = parse_mods_raw(input)?;

    if prefixed {
        Ok((rest, mods))
    } else {
        Err(NomErr::Error(NomError::new(input, NomErrorKind::Char)))
    }
}

fn parse_mods(input: &str) -> Result<GameMods, ParseError<'_>> {
    let (_, (prefixed, mods, suffixed)) = parse_mods_raw(input).map_err(|_| ParseError::Mods)?;

    if prefixed || !suffixed {
        Ok(mods)
    } else {
        Err(ParseError::Mods)
    }
}

fn parse_mods_raw(input: &str) -> IResult<&str, (bool, GameMods, bool)> {
    let prefixed = map(opt(ch::char('+')), is_some);
    let suffixed = map(opt(ch::char('!')), is_some);

    let single_mod = map_parser(by::take(2_usize), all_consuming(ch::alpha1));
    let mods_str = recognize(many1_count(single_mod));
    let mods = map_res(mods_str, GameMods::from_str);

    tuple((prefixed, mods, all_consuming(suffixed)))(input)
}

fn recognize_float(input: &str) -> IResult<&str, &str> {
    let comma = alt((ch::char('.'), ch::char(',')));

    recognize(tuple((ch::digit0, comma, ch::digit1)))(input)
}

fn recognize_acc(input: &str) -> IResult<&str, &str> {
    recognize(ch::char('%'))(input)
}

fn recognize_combo(input: &str) -> IResult<&str, &str> {
    recognize(all_consuming(ch::char('x')))(input)
}

fn recognize_clock_rate(input: &str) -> IResult<&str, &str> {
    recognize(alt((ch::char('*'), all_consuming(ch::char('x')))))(input)
}

fn recognize_n300(input: &str) -> IResult<&str, &str> {
    recognize(by::tag("x300"))(input)
}

fn recognize_n100(input: &str) -> IResult<&str, &str> {
    recognize(by::tag("x100"))(input)
}

fn recognize_n50(input: &str) -> IResult<&str, &str> {
    recognize(by::tag("x50"))(input)
}

fn recognize_geki(input: &str) -> IResult<&str, &str> {
    let options = (
        delimited(opt(ch::char('x')), by::tag("geki"), opt(ch::char('s'))),
        by::tag("x320"),
    );

    recognize(alt(options))(input)
}

fn recognize_katu(input: &str) -> IResult<&str, &str> {
    let options = (
        delimited(opt(ch::char('x')), by::tag("katu"), opt(ch::char('s'))),
        by::tag("x200"),
    );

    recognize(alt(options))(input)
}

fn recognize_miss(input: &str) -> IResult<&str, &str> {
    recognize(preceded(
        opt(ch::char('x')),
        preceded(
            ch::char('m'),
            opt(preceded(by::tag("iss"), opt(by::tag("es")))),
        ),
    ))(input)
}

#[derive(Debug, PartialEq)]
pub enum ParseError<'s> {
    Acc,
    Combo,
    ClockRate,
    N300,
    N100,
    N50,
    Geki,
    Katu,
    Miss,
    Mods,
    Ar,
    Cs,
    Hp,
    Od,
    Nom(&'s str),
    Unknown(&'s str),
}

impl ParseError<'_> {
    pub fn into_str(self) -> Cow<'static, str> {
        match self {
            Self::Acc => "Failed to parse accuracy, must be a number".into(),
            Self::Combo => "Failed to parse combo, must be an integer".into(),
            Self::ClockRate => "Failed to parse clock rate, must be a number".into(),
            Self::N300 => "Failed to parse n300, must be an interger".into(),
            Self::N100 => "Failed to parse n100, must be an interger".into(),
            Self::N50 => "Failed to parse n50, must be an interger".into(),
            Self::Geki => "Failed to parse gekis, must be an interger".into(),
            Self::Katu => "Failed to parse katus, must be an interger".into(),
            Self::Miss => "Failed to parse misses, must be an interger".into(),
            Self::Mods => "Failed to parse mods, must be an acronym of a mod combination".into(),
            Self::Ar => "Failed to parsed AR, must be a number".into(),
            Self::Cs => "Failed to parsed CS, must be a number".into(),
            Self::Hp => "Failed to parsed HP, must be a number".into(),
            Self::Od => "Failed to parsed OD, must be a number".into(),
            Self::Nom(input) => format!("Failed to parse argument `{input}`").into(),
            Self::Unknown(input) => format!(
                "Unknown key `{input}`. Must be `mods`, `acc`, `combo`, `clockrate`, \
                `n300`, `n100`, `n50`, `miss`, `geki`, `katu`, `ar`, `cs`, `hp`, or `od`"
            )
            .into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acc() {
        assert_eq!(
            SimulateArg::parse("acc=123.0%"),
            Ok(SimulateArg::Acc(123.0))
        );
        assert_eq!(
            SimulateArg::parse("accuracy=123"),
            Ok(SimulateArg::Acc(123.0))
        );
        assert_eq!(SimulateArg::parse("a=123%"), Ok(SimulateArg::Acc(123.0)));
        assert_eq!(SimulateArg::parse("123.0%"), Ok(SimulateArg::Acc(123.0)));
        assert_eq!(SimulateArg::parse("acc=123x"), Err(ParseError::Acc));
    }

    #[test]
    fn combo() {
        assert_eq!(
            SimulateArg::parse("combo=123x"),
            Ok(SimulateArg::Combo(123))
        );
        assert_eq!(SimulateArg::parse("c=123"), Ok(SimulateArg::Combo(123)));
        assert_eq!(SimulateArg::parse("123x"), Ok(SimulateArg::Combo(123)));
        assert_eq!(SimulateArg::parse("c=123%"), Err(ParseError::Combo));
        assert_eq!(SimulateArg::parse("combo=123x300"), Err(ParseError::Combo));
        assert_eq!(SimulateArg::parse("c=123.0x"), Err(ParseError::Combo));
    }

    #[test]
    fn clock_rate() {
        assert_eq!(
            SimulateArg::parse("clockrate=123*"),
            Ok(SimulateArg::ClockRate(123.0))
        );
        assert_eq!(
            SimulateArg::parse("cr=123.0x"),
            Ok(SimulateArg::ClockRate(123.0))
        );
        assert_eq!(
            SimulateArg::parse("cr=123.0"),
            Ok(SimulateArg::ClockRate(123.0))
        );
        assert_eq!(
            SimulateArg::parse("123.0*"),
            Ok(SimulateArg::ClockRate(123.0))
        );
        assert_eq!(
            SimulateArg::parse("123.0x"),
            Ok(SimulateArg::ClockRate(123.0))
        );
        assert_eq!(
            SimulateArg::parse("123*"),
            Ok(SimulateArg::ClockRate(123.0))
        );
        assert_eq!(SimulateArg::parse("cr=123%"), Err(ParseError::ClockRate));
    }

    #[test]
    fn n300() {
        assert_eq!(
            SimulateArg::parse("n300=123x300"),
            Ok(SimulateArg::N300(123))
        );
        assert_eq!(SimulateArg::parse("123x300"), Ok(SimulateArg::N300(123)));
        assert_eq!(SimulateArg::parse("n300=123"), Ok(SimulateArg::N300(123)));
        assert_eq!(SimulateArg::parse("n300=123x100"), Err(ParseError::N300));
    }

    #[test]
    fn n100() {
        assert_eq!(
            SimulateArg::parse("n100=123x100"),
            Ok(SimulateArg::N100(123))
        );
        assert_eq!(SimulateArg::parse("123x100"), Ok(SimulateArg::N100(123)));
        assert_eq!(SimulateArg::parse("n100=123"), Ok(SimulateArg::N100(123)));
        assert_eq!(SimulateArg::parse("n100=123x300"), Err(ParseError::N100));
    }

    #[test]
    fn n50() {
        assert_eq!(SimulateArg::parse("n50=123x50"), Ok(SimulateArg::N50(123)));
        assert_eq!(SimulateArg::parse("123x50"), Ok(SimulateArg::N50(123)));
        assert_eq!(SimulateArg::parse("n50=123"), Ok(SimulateArg::N50(123)));
        assert_eq!(SimulateArg::parse("n50=123x100"), Err(ParseError::N50));
    }

    #[test]
    fn gekis() {
        assert_eq!(
            SimulateArg::parse("ngekis=123x320"),
            Ok(SimulateArg::Geki(123))
        );
        assert_eq!(
            SimulateArg::parse("ngeki=123xgeki"),
            Ok(SimulateArg::Geki(123))
        );
        assert_eq!(
            SimulateArg::parse("gekis=123gekis"),
            Ok(SimulateArg::Geki(123))
        );
        assert_eq!(SimulateArg::parse("123x320"), Ok(SimulateArg::Geki(123)));
        assert_eq!(SimulateArg::parse("123xgekis"), Ok(SimulateArg::Geki(123)));
        assert_eq!(SimulateArg::parse("123geki"), Ok(SimulateArg::Geki(123)));
        assert_eq!(SimulateArg::parse("ngeki=123x100"), Err(ParseError::Geki));
    }

    #[test]
    fn katus() {
        assert_eq!(
            SimulateArg::parse("nkatus=123x200"),
            Ok(SimulateArg::Katu(123))
        );
        assert_eq!(
            SimulateArg::parse("nkatu=123xkatu"),
            Ok(SimulateArg::Katu(123))
        );
        assert_eq!(
            SimulateArg::parse("katus=123katus"),
            Ok(SimulateArg::Katu(123))
        );
        assert_eq!(SimulateArg::parse("123x200"), Ok(SimulateArg::Katu(123)));
        assert_eq!(SimulateArg::parse("123xkatus"), Ok(SimulateArg::Katu(123)));
        assert_eq!(SimulateArg::parse("123katu"), Ok(SimulateArg::Katu(123)));
        assert_eq!(SimulateArg::parse("nkatu=123x100"), Err(ParseError::Katu));
    }

    #[test]
    fn misses() {
        assert_eq!(
            SimulateArg::parse("misses=123xmisses"),
            Ok(SimulateArg::Miss(123))
        );
        assert_eq!(SimulateArg::parse("m=123m"), Ok(SimulateArg::Miss(123)));
        assert_eq!(SimulateArg::parse("123m"), Ok(SimulateArg::Miss(123)));
        assert_eq!(SimulateArg::parse("123xm"), Ok(SimulateArg::Miss(123)));
        assert_eq!(
            SimulateArg::parse("miss=123xmiss"),
            Ok(SimulateArg::Miss(123))
        );
        assert_eq!(SimulateArg::parse("m=123x100"), Err(ParseError::Miss));
    }

    #[test]
    fn mods() {
        let hdhr = GameMods::Hidden | GameMods::HardRock;

        assert_eq!(
            SimulateArg::parse("mods=+hdhr!"),
            Ok(SimulateArg::Mods(hdhr))
        );
        assert_eq!(
            SimulateArg::parse("mods=+hdhr"),
            Ok(SimulateArg::Mods(hdhr))
        );
        assert_eq!(SimulateArg::parse("mods=hdhr"), Ok(SimulateArg::Mods(hdhr)));
        assert_eq!(SimulateArg::parse("+hdhr!"), Ok(SimulateArg::Mods(hdhr)));
        assert_eq!(SimulateArg::parse("+hdhr"), Ok(SimulateArg::Mods(hdhr)));

        assert_eq!(SimulateArg::parse("mods=+hdr!"), Err(ParseError::Mods));
        assert_eq!(SimulateArg::parse("mods=-hdhr!"), Err(ParseError::Mods));
        assert_eq!(SimulateArg::parse("mods=hdhr!"), Err(ParseError::Mods));
        assert_eq!(SimulateArg::parse("-hdhr!"), Err(ParseError::Nom("-hdhr!")));
        assert_eq!(SimulateArg::parse("-hdhr"), Err(ParseError::Nom("-hdhr")));
        assert_eq!(SimulateArg::parse("hdhr!"), Err(ParseError::Nom("hdhr!")));
    }
}
