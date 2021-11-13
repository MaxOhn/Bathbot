mod current;
mod mapper;
mod nochoke;
mod top_if;
mod top_old;

pub use current::*;
pub use mapper::*;
pub use nochoke::*;
pub use top_if::*;
pub use top_old::*;

use std::sync::Arc;

use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{application_command::CommandOptionValue, ApplicationCommand},
};

use crate::{
    commands::{
        osu::{option_discord, option_mode, option_mods_explicit, option_name},
        DoubleResultCow, MyCommand, MyCommandOption,
    },
    util::{
        constants::common_literals::{
            ACC, ACCURACY, COMBO, CONSIDER_GRADE, CTB, GRADE, INDEX, MANIA, MODE, MODS, OSU,
            REVERSE, SORT, SPECIFY_MODE, TAIKO,
        },
        MessageExt,
    },
    BotResult, Context, Error,
};

use super::{prepare_scores, request_user, require_link, ErrorType, GradeArg};

enum TopCommandKind {
    If(IfArgs),
    Mapper(MapperArgs),
    Nochoke(NochokeArgs),
    Old(OldArgs),
    Top(TopArgs),
}

impl TopCommandKind {
    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let option = command
            .data
            .options
            .pop()
            .ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "current" => match TopArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(TopCommandKind::Top(args))),
                    Err(content) => Ok(Err(content)),
                },
                "if" => match IfArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(TopCommandKind::If(args))),
                    Err(content) => Ok(Err(content)),
                },
                "mapper" => match MapperArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(TopCommandKind::Mapper(args))),
                    Err(content) => Ok(Err(content)),
                },
                "nochoke" => match NochokeArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(TopCommandKind::Nochoke(args))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            CommandOptionValue::SubCommandGroup(options) => match option.name.as_str() {
                "old" => match OldArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(TopCommandKind::Old(args))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }
}

pub async fn slash_top(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match TopCommandKind::slash(&ctx, &mut command).await? {
        Ok(TopCommandKind::If(args)) => _topif(ctx, command.into(), args).await,
        Ok(TopCommandKind::Mapper(args)) => _mapper(ctx, command.into(), args).await,
        Ok(TopCommandKind::Nochoke(args)) => _nochokes(ctx, command.into(), args).await,
        Ok(TopCommandKind::Old(args)) => _topold(ctx, command.into(), args).await,
        Ok(TopCommandKind::Top(args)) => _top(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

fn subcommand_current() -> MyCommandOption {
    let mode = option_mode();
    let name = option_name();

    let sort_choices = vec![
        CommandOptionChoice::String {
            name: "pp".to_owned(),
            value: "pp".to_owned(),
        },
        CommandOptionChoice::String {
            name: "date".to_owned(),
            value: "date".to_owned(),
        },
        CommandOptionChoice::String {
            name: ACCURACY.to_owned(),
            value: ACC.to_owned(),
        },
        CommandOptionChoice::String {
            name: COMBO.to_owned(),
            value: COMBO.to_owned(),
        },
        CommandOptionChoice::String {
            name: "length".to_owned(),
            value: "len".to_owned(),
        },
    ];

    let sort = MyCommandOption::builder(SORT, "Choose how the scores should be ordered")
        .help("Choose how the scores should be ordered, defaults to `pp`.")
        .string(sort_choices, false);

    let mods = option_mods_explicit();

    let index = MyCommandOption::builder(INDEX, "Choose a specific score index between 1 and 100")
        .integer(Vec::new(), false);

    let discord = option_discord();

    let reverse =
        MyCommandOption::builder(REVERSE, "Reverse the resulting score list").boolean(false);

    let grade_choices = vec![
        CommandOptionChoice::String {
            name: "SS".to_owned(),
            value: "SS".to_owned(),
        },
        CommandOptionChoice::String {
            name: "S".to_owned(),
            value: "S".to_owned(),
        },
        CommandOptionChoice::String {
            name: "A".to_owned(),
            value: "A".to_owned(),
        },
        CommandOptionChoice::String {
            name: "B".to_owned(),
            value: "B".to_owned(),
        },
        CommandOptionChoice::String {
            name: "C".to_owned(),
            value: "C".to_owned(),
        },
        CommandOptionChoice::String {
            name: "D".to_owned(),
            value: "D".to_owned(),
        },
    ];

    let grade = MyCommandOption::builder(GRADE, CONSIDER_GRADE).string(grade_choices, false);

    MyCommandOption::builder("current", "Display the user's current top100")
        .subcommand(vec![mode, name, sort, mods, index, discord, reverse, grade])
}

fn subcommand_if() -> MyCommandOption {
    let mods_description =
        "Specify mods (`+mods` to insert them, `+mods!` to replace, `-mods!` to remove)";

    let mods_help = "Specify how the top score mods should be adjusted.\n\
        Mods must be given as `+mods` to included them everywhere, `+mods!` to replace them exactly, \
        or `-mods!` to excluded them everywhere.\n\
        Examples:\n\
        - `+hd`: Add `HD` to all scores\n\
        - `+hdhr!`: Make all scores `HDHR` scores\n\
        - `+nm!`: Make all scores nomod scores\n\
        - `-ezhd!`: Remove both `EZ` and `HD` from all scores";

    let mods = MyCommandOption::builder(MODS, mods_description)
        .help(mods_help)
        .string(Vec::new(), false);

    let name = option_name();
    let discord = option_discord();

    let if_description = "How the top plays would look like with different mods";

    MyCommandOption::builder("if", if_description).subcommand(vec![mods, name, discord])
}

fn subcommand_mapper() -> MyCommandOption {
    let mapper =
        MyCommandOption::builder("mapper", "Specify a mapper username").string(Vec::new(), false);

    let mode = option_mode();
    let name = option_name();
    let discord = option_discord();

    let mapper_help = "Count the top plays on maps of the given mapper.\n\
        It will try to consider guest difficulties so that if a map was created by someone else \
        but the given mapper made the guest diff, it will count.\n\
        Similarly, if the given mapper created the mapset but someone else guest diff'd, \
        it will not count.\n\
        This does not always work perfectly, like when mappers renamed or when guest difficulties don't have \
        common difficulty labels like `X's Insane`";

    MyCommandOption::builder("mapper", "Count the top plays on maps of the given mapper")
        .help(mapper_help)
        .subcommand(vec![mapper, mode, name, discord])
}

fn subcommand_nochoke() -> MyCommandOption {
    let mode_choices = vec![
        CommandOptionChoice::String {
            name: OSU.to_owned(),
            value: OSU.to_owned(),
        },
        CommandOptionChoice::String {
            name: TAIKO.to_owned(),
            value: TAIKO.to_owned(),
        },
        CommandOptionChoice::String {
            name: CTB.to_owned(),
            value: CTB.to_owned(),
        },
    ];

    let mode_help = "Specify a gamemode. \
        Since combo does not matter in mania, its scores can't be unchoked.";

    let mode = MyCommandOption::builder(MODE, SPECIFY_MODE)
        .help(mode_help)
        .string(mode_choices, false);

    let name = option_name();
    let discord = option_discord();

    let miss_limit_description = "Only unchoke scores with at most this many misses";

    let miss_limit =
        MyCommandOption::builder("miss_limit", miss_limit_description).integer(Vec::new(), false);

    let nochoke_description = "How the top plays would look like with only full combos";

    let nochoke_help = "Remove all misses from top scores and make them full combos.\n\
        Then after recalculating their pp, check how many total pp a user could have had.";

    MyCommandOption::builder("nochoke", nochoke_description)
        .help(nochoke_help)
        .subcommand(vec![mode, name, miss_limit, discord])
}

const VERSION: &str = "version";
const VERSION_DESCRIPTION: &str = "Choose which version should replace the current pp system";

fn subcommand_old() -> MyCommandOption {
    let version_choices = vec![
        CommandOptionChoice::String {
            name: "april 2015 - may 2018".to_owned(),
            value: "april15_may18".to_owned(),
        },
        CommandOptionChoice::String {
            name: "may 2018 - february 2019".to_owned(),
            value: "may18_february19".to_owned(),
        },
        CommandOptionChoice::String {
            name: "february 2019 - january 2021".to_owned(),
            value: "february19_january21".to_owned(),
        },
        CommandOptionChoice::String {
            name: "january 2021 - july 2021".to_owned(),
            value: "january21_july21".to_owned(),
        },
    ];

    let version =
        MyCommandOption::builder(VERSION, VERSION_DESCRIPTION).string(version_choices, true);
    let name = option_name();
    let discord = option_discord();

    let osu_description =
        "How the current osu!standard top plays would look like on a previous pp system";

    let osu_help = "The osu!standard pp history looks roughly like this:\n\
        - 2012: ppv1 (can't be implemented)\n\
        - 2014: ppv2 (unavailable)\n\
        - 2015: High CS nerf(?)\n\
        - 2018: [HD adjustment](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)\n\
        - 2019: [Angles, speed, spaced streams](https://osu.ppy.sh/home/news/2019-02-05-new-changes-to-star-rating-performance-points)\n\
        - 2021: [High AR nerf, NF & SO buff, speed & acc adjustment](https://osu.ppy.sh/home/news/2021-01-14-performance-points-updates)\n\
        - 2021: [Diff spike nerf, AR buff, FL-AR adjust](https://osu.ppy.sh/home/news/2021-07-27-performance-points-star-rating-updates)";

    let osu = MyCommandOption::builder(OSU, osu_description)
        .help(osu_help)
        .subcommand(vec![version, name, discord]);

    let version_choices = vec![CommandOptionChoice::String {
        name: "march 2014 - september 2020".to_owned(),
        value: "march14_september20".to_owned(),
    }];

    let version =
        MyCommandOption::builder(VERSION, VERSION_DESCRIPTION).string(version_choices, true);
    let name = option_name();
    let discord = option_discord();

    let taiko_description =
        "How the current osu!taiko top plays would look like on a previous pp system";

    let taiko_help = "The osu!taiko pp history looks roughly like this:\n\
        - 2014: ppv1\n\
        - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-09-15-changes-to-osutaiko-star-rating)";

    let taiko = MyCommandOption::builder(TAIKO, taiko_description)
        .help(taiko_help)
        .subcommand(vec![version, name, discord]);

    let version_choices = vec![CommandOptionChoice::String {
        name: "march 2014 - may 2020".to_owned(),
        value: "march14_may20".to_owned(),
    }];

    let version =
        MyCommandOption::builder(VERSION, VERSION_DESCRIPTION).string(version_choices, true);
    let name = option_name();
    let discord = option_discord();

    let ctb_description =
        "How the current osu!ctb top plays would look like on a previous pp system";

    let ctb_help = "The osu!ctb pp history looks roughly like this:\n\
        - 2014: ppv1\n\
        - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-05-14-osucatch-scoring-updates)";

    let ctb = MyCommandOption::builder(CTB, ctb_description)
        .help(ctb_help)
        .subcommand(vec![version, name, discord]);

    let version_choices = vec![CommandOptionChoice::String {
        name: "march 2014 - may 2018".to_owned(),
        value: "march14_may18".to_owned(),
    }];

    let version =
        MyCommandOption::builder(VERSION, VERSION_DESCRIPTION).string(version_choices, true);
    let name = option_name();
    let discord = option_discord();

    let mania_description =
        "How the current osu!mania top plays would look like on a previous pp system";

    let mania_help = "The osu!mania pp history looks roughly like this:\n\
        - 2014: ppv1\n\
        - 2018: [ppv2](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)";

    let mania = MyCommandOption::builder(MANIA, mania_description)
        .help(mania_help)
        .subcommand(vec![version, name, discord]);

    let old_description = "How the current top plays would look like on a previous pp system";

    MyCommandOption::builder("old", old_description)
        .help("Check a user's **current** top plays if their pp would be based on a previous pp system")
        .subcommandgroup(vec![osu, taiko, ctb, mania])
}

pub fn define_top() -> MyCommand {
    let description = "Display a user's top plays through various modifications";

    let options = vec![
        subcommand_current(),
        subcommand_if(),
        subcommand_mapper(),
        subcommand_nochoke(),
        subcommand_old(),
    ];

    MyCommand::new("top", description).options(options)
}
