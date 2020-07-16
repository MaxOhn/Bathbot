mod error;
mod oppai;

pub use error::OppaiErr;
pub use oppai::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage() {
        let path = "E:/Games/osu!/beatmaps/1969122.osu";
        let mut oppai = Oppai::new();
        oppai.calculate(path).unwrap();
        oppai
            .set_accuracy(98.73)
            .set_mods(24)
            .set_miss_count(1)
            .calculate(path)
            .unwrap();
        oppai
            .set_combo(150)
            .set_hits(42, 13)
            .calculate(path)
            .unwrap();
    }
}
