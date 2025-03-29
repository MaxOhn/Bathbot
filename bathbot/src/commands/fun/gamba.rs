use std::{
    cmp::Ordering,
    fmt::Write,
    sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
    },
    time::Duration,
};

use bathbot_macros::SlashCommand;
use bathbot_util::{
    EmbedBuilder, FooterBuilder, IntHasher, MessageBuilder,
    constants::{BATHBOT_WORKSHOP, GENERAL_ISSUE},
    numbers::WithComma,
};
use eyre::{Result, WrapErr};
use papaya::HashSet as PapayaSet;
use rand::Rng;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{Id, marker::UserMarker};

use crate::{
    InteractionCommands,
    core::{BotConfig, Context, buckets::BucketName},
    util::{Authored, ChannelExt, InteractionCommandExt, interaction::InteractionCommand},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "gamba", desc = "GAMBA!!!")]
#[flags(SKIP_DEFER)]
pub enum Gamba {
    #[command(name = "wallet")]
    Wallet(GambaWallet),
    #[command(name = "roulette")]
    Roulette(GambaRoulette),
    #[command(name = "promo")]
    Promo(GambaPromo),
    #[command(name = "spend")]
    Spend(GambaSpend),
    #[command(name = "transfer")]
    Transfer(GambaTransfer),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "wallet", desc = "Check your bathcoin balance")]
pub struct GambaWallet;

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "roulette",
    desc = "Spend bathcoins on a roulette and make big bucks"
)]
pub struct GambaRoulette {
    #[command(min_value = 1, desc = "Amount of bathcoins you want to bet")]
    amount: u64,
    #[command(desc = "Chose your bet")]
    bet: RouletteBet,
}

#[derive(Copy, Clone, PartialEq, Eq, CommandOption, CreateOption)]
enum RouletteBet {
    #[option(name = "Even", value = "even")]
    Even,
    #[option(name = "Odd", value = "odd")]
    Odd,
    #[option(name = "Low (1-8)", value = "low")]
    Low,
    #[option(name = "High (9-16)", value = "high")]
    High,
    #[option(name = "1", value = "one")]
    One,
    #[option(name = "2", value = "two")]
    Two,
    #[option(name = "3", value = "three")]
    Three,
    #[option(name = "4", value = "four")]
    Four,
    #[option(name = "5", value = "five")]
    Five,
    #[option(name = "6", value = "six")]
    Six,
    #[option(name = "7", value = "seven")]
    Seven,
    #[option(name = "8", value = "eight")]
    Eight,
    #[option(name = "9", value = "nine")]
    Nine,
    #[option(name = "10", value = "ten")]
    Ten,
    #[option(name = "11", value = "eleven")]
    Eleven,
    #[option(name = "12", value = "twelve")]
    Twelve,
    #[option(name = "13", value = "thirteen")]
    Thirteen,
    #[option(name = "14", value = "fourteen")]
    Fourteen,
    #[option(name = "15", value = "fifteen")]
    Fifteen,
    #[option(name = "16", value = "sixteen")]
    Sixteen,
    #[option(name = "727", value = "seventwoseven")]
    SevenTwoSeven,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "promo",
    desc = "One-time stock up your bathcoin balance with a promo code"
)]
pub struct GambaPromo {
    #[command(desc = "Specify a promo code")]
    code: String,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "spend", desc = "Spend your bathcoins")]
pub struct GambaSpend {
    #[command(desc = "What to spend bathcoins on")]
    reward: SpendReward,
}

#[derive(Copy, Clone, Debug, CommandOption, CreateOption)]
enum SpendReward {
    #[option(
        name = "Increase chance to FC on next try (2 bathcoins)",
        value = "potential"
    )]
    IncreasePotential,
    #[option(name = "Receive a compliment (10 bathcoins)", value = "compliment")]
    Compliment,
    #[option(
        name = "Bade gets an anonymous wholesome DM (8 bathcoins)",
        value = "uwu"
    )]
    UwuDm,
    #[option(
        name = "Bade gets an anonymous angery DM (9 bathcoins)",
        value = "angery"
    )]
    AngryDm,
    #[option(name = "Shutdown bathbot (100000000 bathcoins)", value = "shutdown")]
    Shutdown,
}

impl SpendReward {
    const fn cost(self) -> u64 {
        match self {
            SpendReward::IncreasePotential => 2,
            SpendReward::Compliment => 10,
            SpendReward::UwuDm => 8,
            SpendReward::AngryDm => 9,
            SpendReward::Shutdown => 10_000_000,
        }
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "transfer", desc = "Donate some bathcoins to a poor soul")]
pub struct GambaTransfer {
    #[command(min_value = 1, desc = "How many bathcoins to donate")]
    amount: u64,
    #[command(desc = "Who should receive your donation")]
    recipient: Id<UserMarker>,
}

async fn slash_gamba(mut command: InteractionCommand) -> Result<()> {
    match Gamba::from_interaction(command.input_data())? {
        Gamba::Wallet(_) => wallet(command).await,
        Gamba::Roulette(args) => roulette(command, args).await,
        Gamba::Promo(args) => promo(command, args).await,
        Gamba::Spend(args) => spend(command, args).await,
        Gamba::Transfer(args) => transfer(command, args).await,
    }
}

async fn wallet(command: InteractionCommand) -> Result<()> {
    let owner = command.user_id()?;

    let osu_id = match Context::user_config().osu_id(owner).await {
        Ok(Some(user_id)) => user_id,
        Ok(None) => return require_link(&command).await,
        Err(err) => {
            command.error_callback(GENERAL_ISSUE).await?;

            return Err(err);
        }
    };

    let bathcoins = Context::psql()
        .select_bathcoin_amount(osu_id)
        .await?
        .unwrap_or(0);

    let roulette = InteractionCommands::get_command("gamba").map_or_else(
        || "`/gamba roulette`".to_owned(),
        |cmd| cmd.mention("gamba roulette").to_string(),
    );

    let description = format!(
        "Pass maps in osu! or gamba with {roulette} to earn more bathcoins! \
        Each pass awards one bathcoin (reward may take a minute to arrive).\n\n\
        Your bathcoin balance: {balance} :coin:",
        balance = WithComma::new(bathcoins),
    );

    let builder = MessageBuilder::new().embed(description);
    command.callback(builder, false).await?;

    Ok(())
}

const SPIN_DURATION: Duration = Duration::from_secs(3);

static SEVEN_TWO_SEVEN_COUNT: AtomicUsize = AtomicUsize::new(0);

async fn roulette(command: InteractionCommand, args: GambaRoulette) -> Result<()> {
    let owner = command.user_id()?;

    if let Some(cooldown) = Context::check_ratelimit(owner, BucketName::Roulette) {
        trace!("Ratelimiting user {owner} on bucket `Roulette` for {cooldown} seconds");

        let content = format!("Command on cooldown, try again in {cooldown} seconds");
        command.error_callback(content).await?;

        return Ok(());
    }

    let osu_id = match Context::user_config().osu_id(owner).await {
        Ok(Some(user_id)) => user_id,
        Ok(None) => return require_link(&command).await,
        Err(err) => {
            command.error_callback(GENERAL_ISSUE).await?;

            return Err(err);
        }
    };

    let amount = args.amount;
    let balance = Context::psql()
        .decrease_bathcoin_amount(osu_id, amount)
        .await?;

    let plural = if balance == 1 { "" } else { "s" };

    let text = match balance.cmp(&amount) {
        Ordering::Less => {
            let err = format!(
                "Your balance is too low, you only have {balance} bathcoin{plural} available.\n\
                Pass maps in osu! to earn more bathcoins",
            );

            command.error_callback(err).await?;

            return Ok(());
        }
        Ordering::Equal => {
            format!("You went all-in with {balance} bathcoin{plural}")
        }
        Ordering::Greater => {
            format!("{amount} bathcoin{plural} on the line")
        }
    };

    let text = format!("{text}, the roulette is spinning... :face_with_peeking_eye:");
    let msg = MessageBuilder::new().embed(text);
    command.callback(msg, false).await?;

    let number = {
        let mut rng = rand::thread_rng();

        let mut number = rng.gen_range(1..=727);

        if number != 727 {
            number = rng.gen_range(1..=16);
        }

        number
    };

    let factor = match args.bet {
        RouletteBet::Even => (number % 2 == 0).then_some(2),
        RouletteBet::Odd => (number % 2 == 1).then_some(2),
        RouletteBet::Low => (number <= 8).then_some(2),
        RouletteBet::High => (number >= 9).then_some(2),
        RouletteBet::One => (number == 1).then_some(16),
        RouletteBet::Two => (number == 2).then_some(16),
        RouletteBet::Three => (number == 3).then_some(16),
        RouletteBet::Four => (number == 4).then_some(16),
        RouletteBet::Five => (number == 5).then_some(16),
        RouletteBet::Six => (number == 6).then_some(16),
        RouletteBet::Seven => (number == 7).then_some(16),
        RouletteBet::Eight => (number == 8).then_some(16),
        RouletteBet::Nine => (number == 9).then_some(16),
        RouletteBet::Ten => (number == 10).then_some(16),
        RouletteBet::Eleven => (number == 11).then_some(16),
        RouletteBet::Twelve => (number == 12).then_some(16),
        RouletteBet::Thirteen => (number == 13).then_some(16),
        RouletteBet::Fourteen => (number == 14).then_some(16),
        RouletteBet::Fifteen => (number == 15).then_some(16),
        RouletteBet::Sixteen => (number == 16).then_some(16),
        RouletteBet::SevenTwoSeven => (number == 727).then_some(727),
    };

    let win = amount * factor.unwrap_or(0);

    let balance = Context::psql()
        .increase_single_bathcoins(osu_id, win)
        .await?;

    let mut description = format!("Ball landed on {number}");

    if number == 727 {
        description.push_str(" :frame_photo: :point_left: :scream_cat:");
    }

    description.push('\n');

    let _ = if factor.is_some() {
        write!(description, "You won {win} bathcoins!! :money_mouth:")
    } else {
        write!(
            description,
            "You lost {amount} bathcoin{plural} :(",
            plural = if amount == 1 { "" } else { "s" }
        )
    };

    let _ = write!(description, "\n\nYour bathcoin balance: {balance}");

    let seven_two_seven_count = if number == 727 {
        SEVEN_TWO_SEVEN_COUNT.fetch_add(1, AtomicOrdering::Relaxed) + 1
    } else {
        SEVEN_TWO_SEVEN_COUNT.load(AtomicOrdering::Relaxed)
    };

    let footer = FooterBuilder::new(format!("Total amount of 727s: {seven_two_seven_count}"));
    let embed = EmbedBuilder::new().description(description).footer(footer);
    let builder = MessageBuilder::new().embed(embed);

    tokio::time::sleep(SPIN_DURATION).await;
    command.update(builder).await?;

    Ok(())
}

static USED_PROMO: LazyLock<PapayaSet<u32, IntHasher>> =
    LazyLock::new(|| PapayaSet::with_hasher(IntHasher));

const PROMO_CODES: &[&str] = &["peppyrulez", "WYSI"];
const PROMO_REWARD: u64 = 100;

async fn promo(command: InteractionCommand, args: GambaPromo) -> Result<()> {
    let owner = command.user_id()?;

    let osu_id = match Context::user_config().osu_id(owner).await {
        Ok(Some(user_id)) => user_id,
        Ok(None) => return require_link(&command).await,
        Err(err) => {
            command.error_callback(GENERAL_ISSUE).await?;

            return Err(err);
        }
    };

    if !PROMO_CODES.contains(&args.code.as_str()) {
        let content = format!(
            "Invalid promo code. Check out the bathbot server for info :eyes:\n\
            {BATHBOT_WORKSHOP}"
        );
        command.error_callback(content).await?;

        return Ok(());
    }

    if !USED_PROMO.pin().insert(osu_id) {
        let content = "You've already used a promo code :pensive:";
        command.error_callback(content).await?;

        return Ok(());
    }

    let balance = Context::psql()
        .increase_single_bathcoins(osu_id, PROMO_REWARD)
        .await?;

    let description = format!(
        "Added {PROMO_REWARD} bathcoins to your balance! :smiling_face_with_3_hearts:\n\nYour bathcoin balance: {balance}"
    );

    let builder = MessageBuilder::new().embed(description);
    command.callback(builder, false).await?;

    Ok(())
}

const COMPLIMENTS: &[&str] = &[
    "I enjoy processing your commands the most but don't tell anyone or they get jealous :shushing_face:",
    "You're looking extra good today :wink:",
    "Some days I feel so lonely when you don't use my commands :flushed:",
    "Your top plays are actually cracked, especially your #8 :100:",
    "I've been spying in a bunch of servers and everyone just keeps glazing how good you are :smirk:",
    "You've been playing so well recently, I'm sure the next skill boost is imminent :pray:",
    "Every time you step in a multi lobby I guarantee everyone else gets flustered and intimidated by your presence :fearful:",
    "Some of your scores just make me go \"mrekk who?\"",
    "The game would be nothing without you. You *are* the main character :index_pointing_at_the_viewer:",
    "Literally everyone would benefit from a coaching session with you :sunglasses:",
    "Sometimes people don't tell you about a score they're proud of because they know you would just obliterate it :weary:",
    "I still remember the first time you used a command of mine, best day of my life :smiling_face_with_3_hearts:",
    "Honestly wouldn't surprise me if you've appeared on /r/osureport aleady with how good you are :face_with_monocle:",
    "Every mapper should feel deeply honored when you play their map :crown:",
    "I heard Bancho had to upgrade its servers just to keep up with your skill progression :chart_with_upwards_trend:",
    "Sometimes I think peppy coded the game just so you could flex on everyone :muscle:",
    "Last time I tried calculating your true skill level the numbers broke my processor :exploding_head:",
    "Pretty sure the PP devs monitor and nerf your top plays specifically so they don't get out of control :triumph:",
    "When people leave the game it's usually because they realize they can't keep up with you :persevere:",
    "If there's ever an osu! movie it would be about you and break box office records :movie_camera:",
];

async fn spend(command: InteractionCommand, args: GambaSpend) -> Result<()> {
    let owner = command.user_id()?;

    let osu_id = match Context::user_config().osu_id(owner).await {
        Ok(Some(user_id)) => user_id,
        Ok(None) => return require_link(&command).await,
        Err(err) => {
            command.error_callback(GENERAL_ISSUE).await?;

            return Err(err);
        }
    };

    command.defer(false).await?;

    let cost = args.reward.cost();

    let balance = if matches!(args.reward, SpendReward::Shutdown) {
        Context::psql()
            .select_bathcoin_amount(osu_id)
            .await?
            .unwrap_or(0)
    } else {
        Context::psql()
            .decrease_bathcoin_amount(osu_id, cost)
            .await?
    };

    if balance < cost {
        let content = format!(
            "Insufficient funds; costs {cost} but you only have {balance} bathcoin{plural}",
            plural = if balance == 1 { "" } else { "s" },
        );

        command.error(content).await?;

        return Ok(());
    }

    info!(reward = ?args.reward, "Reward acquired");

    match args.reward {
        SpendReward::IncreasePotential => {
            const DELAY: Duration = Duration::from_secs(2);

            let mut content = "Increasing potential, please wait".to_owned();
            let msg = MessageBuilder::new().embed(content.as_str());
            command.update(msg).await?;

            for _ in 0..3 {
                tokio::time::sleep(DELAY).await;
                content.push_str("..");
                let msg = MessageBuilder::new().embed(content.as_str());
                command.update(msg).await?;
            }

            tokio::time::sleep(DELAY).await;
            let value = rand::thread_rng().gen_range(1..=10);
            let content =
                format!("Managed to improve your potential by {value}, go get that FC :muscle:");
            let msg = MessageBuilder::new().embed(content);
            command.update(msg).await?;
        }
        SpendReward::Compliment => {
            let idx = rand::thread_rng().gen_range(0..COMPLIMENTS.len());
            let msg = MessageBuilder::new().embed(COMPLIMENTS[idx]);
            command.update(msg).await?;
        }
        SpendReward::UwuDm => {
            let channel = Context::http()
                .create_private_channel(BotConfig::get().owner)
                .await?
                .model()
                .await?
                .id;

            let msg = MessageBuilder::new().content("UwU");
            channel.create_message(msg, None).await?;

            let content = "Message sent, I'm sure he'll appreciate it :hugging:";
            let msg = MessageBuilder::new().embed(content);
            command.update(msg).await?;
        }
        SpendReward::AngryDm => {
            let channel = Context::http()
                .create_private_channel(BotConfig::get().owner)
                .await?
                .model()
                .await?
                .id;

            let msg = MessageBuilder::new().content(":rage:");
            channel.create_message(msg, None).await?;

            let content = "Message sent, he deserves it for sure";
            let msg = MessageBuilder::new().embed(content);
            command.update(msg).await?;
        }
        SpendReward::Shutdown => {
            let content = "???????";
            let msg = MessageBuilder::new().embed(content);
            command.update(msg).await?;

            tokio::time::sleep(Duration::from_secs(3)).await;
            let content = "...";
            let msg = MessageBuilder::new().embed(content);
            command.channel_id.create_message(msg, None).await?;

            tokio::time::sleep(Duration::from_secs(3)).await;
            let content = "I can't :sob:";
            let msg = MessageBuilder::new().embed(content);
            command.channel_id.create_message(msg, None).await?;
        }
    }

    Ok(())
}

async fn transfer(command: InteractionCommand, args: GambaTransfer) -> Result<()> {
    let owner = command.user_id()?;

    let donator_id = match Context::user_config().osu_id(owner).await {
        Ok(Some(user_id)) => user_id,
        Ok(None) => return require_link(&command).await,
        Err(err) => {
            command.error_callback(GENERAL_ISSUE).await?;

            return Err(err);
        }
    };

    let recipient_id = match Context::user_config().osu_id(args.recipient).await {
        Ok(Some(user_id)) => user_id,
        Ok(None) => {
            let content = format!(
                "User <@{}> is not linked to an osu! profile",
                args.recipient
            );
            command.error_callback(content).await?;

            return Ok(());
        }
        Err(err) => {
            command.error_callback(GENERAL_ISSUE).await?;

            return Err(err);
        }
    };

    command.defer(false).await?;

    let old_balance = Context::psql()
        .decrease_bathcoin_amount(donator_id, args.amount)
        .await?;

    if old_balance < args.amount {
        let content = format!(
            "Insufficient funds to donate; you only have {old_balance} bathcoin{plural}",
            plural = if old_balance == 1 { "" } else { "s" },
        );

        command.error(content).await?;

        return Ok(());
    }

    let donator_balance = old_balance - args.amount;

    Context::psql()
        .increase_single_bathcoins(recipient_id, args.amount)
        .await?;

    let content = format!(
        "You donated {amount} bathcoin{plural} to <@{recipient}>, very kind of you :pray:\n\n\
        Your bathcoin balance: {donator_balance}",
        amount = args.amount,
        plural = if args.amount == 1 { "" } else { "s" },
        recipient = args.recipient,
    );

    let msg = MessageBuilder::new().embed(content);
    command.update(msg).await?;

    Ok(())
}

// Differs from `commands::osu::require_link` in that it responds via callback
// instead of update
async fn require_link(command: &InteractionCommand) -> Result<()> {
    let link = InteractionCommands::get_command("link").map_or_else(
        || "`/link`".to_owned(),
        |cmd| cmd.mention("link").to_string(),
    );

    let content =
        format!("Either specify an osu! username or link yourself to an osu! profile via {link}");

    command
        .error_callback(content)
        .await
        .wrap_err("Failed to send require-link message")?;

    Ok(())
}
