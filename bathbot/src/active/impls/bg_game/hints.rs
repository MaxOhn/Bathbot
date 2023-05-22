use std::iter;

use rand::seq::SliceRandom;

pub struct Hints {
    pub artist_guessed: bool,
    hint_level: u8,
    title_mask: Box<[bool]>,
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

        let title_mask = iter::once(true)
            .chain(title.chars().skip(1).map(|c| c == ' '))
            .collect();

        Self {
            artist_guessed: false,
            hint_level: 0,
            title_mask,
            indices,
        }
    }

    pub fn get(&mut self, title: &str, artist: &str) -> String {
        self.hint_level = self.hint_level.saturating_add(1);

        if self.hint_level == 1 {
            let word_count = title.split(' ').count();

            format!(
                "Let me give you a hint: The title has {amount} \
                word{plural} and the starting letter is `{first}`",
                amount = word_count,
                plural = if word_count != 1 { "s" } else { "" },
                first = title.chars().next().unwrap(),
            )
        } else if self.hint_level == 2 && !self.artist_guessed {
            let mut artist_hint = "Here's my second hint: The artist looks like `".to_owned();
            artist_hint.reserve(3 * artist.len() - 1);

            let mut artist_iter = artist.chars();

            if let Some(c) = artist_iter.next() {
                artist_hint.push(c);
                artist_hint.extend(artist_iter.map(|c| if c == ' ' { c } else { '▢' }));
            }

            artist_hint.push('`');

            artist_hint
        } else if let Some(i) = self.indices.pop() {
            self.title_mask[i] = true;

            let mut title_hint = "Slowly constructing the title: `".to_owned();

            let title_iter =
                self.title_mask
                    .iter()
                    .zip(title.chars())
                    .map(|(mask, c)| if *mask { c } else { '▢' });

            title_hint.extend(title_iter);
            title_hint.push('`');

            title_hint
        } else {
            format!("Bruh the title is literally `{title}` xd")
        }
    }
}
