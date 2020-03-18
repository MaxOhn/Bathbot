use rand::seq::SliceRandom;

#[derive(Default)]
pub struct Hints {
    hint_level: u8,
    title_mask: Vec<bool>,
    indices: Vec<usize>,
}

impl Hints {
    pub fn new(title: &str) -> Self {
        // Indices of chars that still need to be revealed
        let mut indices: Vec<_> = title
            .chars()
            .enumerate()
            .skip(1) // first char revealed immediatly
            .filter(|(_, c)| *c != ' ') // spaces revealed immediatly
            .map(|(i, _)| i)
            .collect();
        let mut rng = rand::thread_rng();
        indices.shuffle(&mut rng);
        let mut title_mask = Vec::with_capacity(title.len());
        title_mask.push(true);
        for c in title.chars().skip(1) {
            title_mask.push(c == ' ');
        }
        Self {
            hint_level: 0,
            title_mask,
            indices,
        }
    }

    pub fn get(&mut self, title: &str, artist: &str) -> String {
        self.hint_level = self.hint_level.saturating_add(1);
        match self.hint_level {
            1 => {
                let word_count = title.split(' ').count();
                format!(
                    "Let me give you a hint: The title has {amount} \
                    word{plural} and the starting letter is `{first}`",
                    amount = word_count,
                    plural = if word_count != 1 { "s" } else { "" },
                    first = title.chars().next().unwrap(),
                )
            }
            2 => {
                let mut artist_hint = String::with_capacity(artist.len());
                artist_hint.push(artist.chars().next().unwrap());
                for c in artist.chars().skip(1) {
                    artist_hint.push(if c == ' ' { c } else { '▢' });
                }
                format!(
                    "Here's my second hint: The artist looks like `{}`",
                    artist_hint
                )
            }
            _ => {
                if let Some(i) = self.indices.pop() {
                    self.title_mask[i] = true;
                    let title_hint: String = self
                        .title_mask
                        .iter()
                        .zip(title.chars())
                        .map(|(mask, c)| if *mask { c } else { '▢' })
                        .collect();
                    format!("Slowly constructing the title: `{}`", title_hint)
                } else {
                    format!("Bruh the title is literally `{}` xd", title)
                }
            }
        }
    }
}
