use crate::{
    util::{ApplicationCommandExt, CowUtils, Matrix, MessageExt},
    Args, BotResult, CommandData, Context, Error, MessageBuilder,
};

use rand::RngCore;
use std::{
    fmt::{self, Write},
    sync::Arc,
};
use twilight_model::application::{
    command::{ChoiceCommandOptionData, Command, CommandOption, CommandOptionChoice},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

#[command]
#[short_desc("Play a game of minesweeper")]
#[long_desc(
    "Play a game of minesweeper.\n\
    The available arguments are:\n \
    - `easy`: 6x6 grid\n \
    - `medium`: 8x8 grid\n \
    - `hard`: 9x11 grid"
)]
#[usage("[easy / medium / hard]")]
async fn minesweeper(ctx: Arc<Context>, mut data: CommandData) -> BotResult<()> {
    let difficulty = match &mut data {
        CommandData::Message { args, msg, .. } => match Difficulty::args(args) {
            Ok(difficulty) => difficulty,
            Err(content) => {
                let builder = MessageBuilder::new().content(content);
                msg.create_message(&ctx, builder).await?;

                return Ok(());
            }
        },
        CommandData::Interaction { command } => Difficulty::slash(command)?,
    };

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

    let content = format!(
        "Here's a {}x{} game with {} mines:\n{}",
        w, h, game.mines, field
    );

    let builder = MessageBuilder::new().content(content);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

enum Difficulty {
    Easy,
    Medium,
    Hard,
    Expert,
}

impl Difficulty {
    fn args(args: &mut Args) -> Result<Self, &'static str> {
        match args.next().map(CowUtils::cow_to_ascii_lowercase).as_deref() {
            None | Some("easy") => Ok(Self::Easy),
            Some("medium") => Ok(Self::Medium),
            Some("hard") => Ok(Self::Hard),
            // Some("expert") => Ok(Self::Expert),
            _ => return Err("The argument must be either `easy`, `medium`, or `hard`"),
        }
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let mut difficulty = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "difficulty" => match value.as_str() {
                        "Easy" => difficulty = Some(Self::Easy),
                        "Medium" => difficulty = Some(Self::Medium),
                        "Hard" => difficulty = Some(Self::Hard),
                        "Expert" => difficulty = Some(Self::Expert),
                        _ => bail_cmd_option!("minesweeper", string, value),
                    },
                    _ => bail_cmd_option!("minesweeper", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("minesweeper", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("minesweeper", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("minesweeper", subcommand, name)
                }
            }
        }

        difficulty.ok_or(Error::InvalidCommandOptions)
    }

    fn create(&self) -> Minesweeper {
        match self {
            Difficulty::Easy => Minesweeper::new(6, 6, 6),
            Difficulty::Medium => Minesweeper::new(8, 8, 12),
            Difficulty::Hard => Minesweeper::new(11, 9, 20),
            Difficulty::Expert => Minesweeper::new(13, 13, 40),
        }
    }
}

struct Minesweeper {
    pub field: Matrix<Cell>,
    pub mines: u8,
}

impl Minesweeper {
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

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

pub async fn slash_minesweeper(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    minesweeper(ctx, command.into()).await
}

pub fn slash_minesweeper_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "minesweeper".to_owned(),
        default_permission: None,
        description: "Play a game of minesweeper".to_owned(),
        id: None,
        options: vec![CommandOption::String(ChoiceCommandOptionData {
            choices: vec![
                CommandOptionChoice::String {
                    name: "Easy".to_owned(),
                    value: "Easy".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "Medium".to_owned(),
                    value: "Medium".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "Hard".to_owned(),
                    value: "Hard".to_owned(),
                },
            ],
            description: "Choose a difficulty".to_owned(),
            name: "difficulty".to_owned(),
            required: true,
        })],
    }
}
