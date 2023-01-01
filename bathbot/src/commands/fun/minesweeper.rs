use std::{
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::{command, SlashCommand};
use eyre::Result;
use rand::RngCore;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};

use crate::{
    core::{
        commands::{prefix::Args, CommandOrigin},
        Context,
    },
    util::{
        builder::MessageBuilder, interaction::InteractionCommand, ChannelExt, CowUtils,
        InteractionCommandExt, Matrix,
    },
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "minesweeper",
    help = "Play a game of minesweeper.\n\
        In case you don't know how it works: Each number indicates the amount of neighboring bombs."
)]
#[flags(SKIP_DEFER)]
/// Play a game of minesweeper
pub struct Minesweeper {
    /// Choose a difficulty
    difficulty: Difficulty,
}

#[derive(CommandOption, CreateOption)]
enum Difficulty {
    #[option(name = "easy", value = "easy")]
    Easy,
    #[option(name = "medium", value = "medium")]
    Medium,
    #[option(name = "hard", value = "hard")]
    Hard,
    // #[option(name = "expert", value = "expert")]
    // Expert,
}

pub async fn slash_minesweeper(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Minesweeper::from_interaction(command.input_data())?;

    minesweeper(ctx, (&mut command).into(), args.difficulty).await
}

#[command]
#[desc("Play a game of minesweeper")]
#[help(
    "Play a game of minesweeper.\n\
    The available arguments are:\n\
    - `easy`: 6x6 grid\n\
    - `medium`: 8x8 grid\n\
    - `hard`: 9x11 grid"
)]
#[usage("[easy / medium / hard]")]
#[flags(SKIP_DEFER)]
#[group(Games)]
async fn prefix_minesweeper(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
    let difficulty = match Difficulty::args(&mut args) {
        Ok(difficulty) => difficulty,
        Err(content) => {
            msg.error(&ctx, content).await?;

            return Ok(());
        }
    };

    minesweeper(ctx, msg.into(), difficulty).await
}

async fn minesweeper(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    difficulty: Difficulty,
) -> Result<()> {
    let game = difficulty.create();
    let (w, h) = game.dim();
    let mut field = String::with_capacity(w * h * 9);

    for x in 0..w {
        for y in 0..h {
            let _ = write!(field, "||:{}:||", game.field[(x, y)]);
        }

        field.push('\n');
    }

    field.pop();

    let content = format!("Here's a {w}x{h} game with {} mines:\n{field}", game.mines);
    let builder = MessageBuilder::new().content(content);
    orig.callback(&ctx, builder).await?;

    Ok(())
}

impl Difficulty {
    fn args(args: &mut Args<'_>) -> Result<Self, &'static str> {
        match args
            .next()
            .map(|arg| arg.cow_to_ascii_lowercase())
            .as_deref()
        {
            None | Some("easy") => Ok(Self::Easy),
            Some("medium") => Ok(Self::Medium),
            Some("hard") => Ok(Self::Hard),
            // Some("expert") => Ok(Self::Expert),
            _ => Err("The argument must be either `easy`, `medium`, or `hard`"),
        }
    }

    fn create(&self) -> Game {
        match self {
            Self::Easy => Game::new(6, 6, 6),
            Self::Medium => Game::new(8, 8, 12),
            Self::Hard => Game::new(11, 9, 20),
            // Self::Expert => Game::new(13, 13, 40),
        }
    }
}

struct Game {
    pub field: Matrix<Cell>,
    pub mines: u8,
}

impl Game {
    fn new(height: usize, width: usize, mines: u8) -> Self {
        let mut field = Matrix::new(width, height);
        let mut rng = rand::thread_rng();
        let size = width * height;
        let mut new_mines = mines;

        // Place mines
        while new_mines > 0 {
            let r = rng.next_u32() as usize % size;
            let x = r % width;
            let y = r / width;
            if field[(x, y)] == Cell::None {
                field[(x, y)] = Cell::Mine;
                new_mines -= 1;
            }
        }

        // Place numbers
        for x in 0..width {
            for y in 0..height {
                if field[(x, y)] == Cell::None {
                    let mines = field.count_neighbors(x, y, Cell::Mine);
                    field[(x, y)] = Cell::Num(mines);
                }
            }
        }

        Self { field, mines }
    }

    fn dim(&self) -> (usize, usize) {
        (self.field.width(), self.field.height())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Cell {
    Num(u8),
    Mine,
    None,
}

impl Display for Cell {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Num(0) => f.write_str("zero"),
            Self::Num(1) => f.write_str("one"),
            Self::Num(2) => f.write_str("two"),
            Self::Num(3) => f.write_str("three"),
            Self::Num(4) => f.write_str("four"),
            Self::Num(5) => f.write_str("five"),
            Self::Num(6) => f.write_str("six"),
            Self::Num(7) => f.write_str("seven"),
            Self::Num(8) => f.write_str("eight"),
            Self::Mine => f.write_str("bomb"),
            Self::None | Self::Num(_) => unreachable!(),
        }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::None
    }
}
