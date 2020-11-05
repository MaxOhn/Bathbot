use crate::{
    util::{Matrix, MessageExt},
    Args, BotResult, Context,
};

use cow_utils::CowUtils;
use rand::RngCore;
use std::{
    fmt::{self, Write},
    sync::Arc,
};
use twilight_model::channel::Message;

#[command]
#[short_desc("Play a game of minesweeper")]
#[aliases("ms")]
#[usage("[Easy/Medium/Hard/Extreme]")]
async fn minesweeper(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let difficulty = match args.next().map(|arg| arg.cow_to_lowercase()).as_deref() {
        None | Some("easy") => Difficulty::Easy,
        Some("medium") => Difficulty::Medium,
        Some("hard") => Difficulty::Hard,
        Some("extreme") | Some("expert") => Difficulty::Extreme,
        _ => {
            let content = "The argument must be either `Easy`, `Medium`, `Hard`, or `Extreme`";
            return msg.error(&ctx, content).await;
        }
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
    msg.respond(&ctx, content).await?;
    Ok(())
}

enum Difficulty {
    Easy,
    Medium,
    Hard,
    Extreme,
}

impl Difficulty {
    fn create(&self) -> Minesweeper {
        match self {
            Difficulty::Easy => Minesweeper::new(6, 6, 6),
            Difficulty::Medium => Minesweeper::new(8, 8, 12),
            Difficulty::Hard => Minesweeper::new(10, 10, 20),
            Difficulty::Extreme => Minesweeper::new(13, 13, 40),
        }
    }
}

struct Minesweeper {
    pub field: Matrix<Cell>,
    pub mines: u8,
}

impl Minesweeper {
    fn new(width: usize, height: usize, mines: u8) -> Self {
        let mut field = Matrix::new(width, height);
        let mut rng = rand::thread_rng();
        let size = width * height;
        let mut new_mines = mines;
        // Place mines
        while new_mines > 0 {
            let r = rng.next_u32() as usize % size;
            let x = r % width;
            let y = r / height;
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
