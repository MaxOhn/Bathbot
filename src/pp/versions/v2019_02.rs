use super::MyOsuPP;

use rosu_pp::{
    osu::{no_leniency::stars, OsuAttributeProvider},
    Mods, PpResult, StarResult,
};

impl<'m> MyOsuPP<'m> {
    pub fn calculate_v2019(mut self) -> PpResult {
        if self.attributes.is_none() {
            let attributes = stars(self.map, self.mods, self.passed_objects)
                .attributes()
                .unwrap();
            self.attributes.replace(attributes);
        }

        // Make sure the hitresults and accuracy are set
        self.assert_hitresults();

        let total_hits = self.total_hits();
        let mut multiplier = 1.12;

        // NF penalty
        if self.mods.nf() {
            multiplier *= 0.9;
        }

        // SO penalty
        if self.mods.so() {
            multiplier *= 0.95;
        }

        let aim_value = self.compute_aim_value(total_hits as f32);
        let speed_value = self.compute_speed_value(total_hits as f32);
        let acc_value = self.compute_accuracy_value(total_hits);

        let pp = (aim_value.powf(1.1) + speed_value.powf(1.1) + acc_value.powf(1.1))
            .powf(1.0 / 1.1)
            * multiplier;

        let attributes = StarResult::Osu(self.attributes.unwrap());

        PpResult { pp, attributes }
    }

    fn compute_aim_value_v2019(&self, total_hits: f32) -> f32 {
        let attributes = self.attributes.as_ref().unwrap();

        // TD penalty
        let raw_aim = if self.mods.td() {
            attributes.aim_strain.powf(0.8)
        } else {
            attributes.aim_strain
        };

        let mut aim_value = (5.0 * (raw_aim / 0.0675).max(1.0) - 4.0).powi(3) / 100_000.0;

        // Longer maps are worth more
        let len_bonus = 0.95
            + 0.4 * (total_hits / 2000.0).min(1.0)
            + (total_hits > 2000.0) as u8 as f32 * 0.5 * (total_hits / 2000.0).log10();
        aim_value *= len_bonus;

        // Penalize misses
        aim_value *= 0.97_f32.powi(self.n_misses as i32);

        // Combo scaling
        if let Some(combo) = self.combo.filter(|_| attributes.max_combo > 0) {
            aim_value *= ((combo as f32 / attributes.max_combo as f32).powf(0.8)).min(1.0);
        }

        // AR bonus
        let ar_factor = if attributes.ar > 10.33 {
            0.3 * (attributes.ar - 10.33)
        } else if attributes.ar < 8.0 {
            0.01 * (8.0 - attributes.ar)
        } else {
            0.0
        };
        aim_value *= 1.0 + ar_factor;

        // HD bonus
        if self.mods.hd() {
            aim_value *= 1.0 + 0.04 * (12.0 - attributes.ar);
        }

        // FL bonus
        if self.mods.fl() {
            aim_value *= 1.0
                + 0.35 * (total_hits / 200.0).min(1.0)
                + (total_hits > 200.0) as u8 as f32 * 0.3 * ((total_hits - 200.0) / 300.0).min(1.0)
                + (total_hits > 500.0) as u8 as f32 * (total_hits - 500.0) / 1200.0;
        }

        // Scale with accuracy
        aim_value *= 0.5 + self.acc.unwrap() / 2.0;
        aim_value *= 0.98 + attributes.od * attributes.od / 2500.0;

        aim_value
    }

    fn compute_speed_value_v2019(&self, total_hits: f32) -> f32 {
        let attributes = self.attributes.as_ref().unwrap();

        let mut speed_value =
            (5.0 * (attributes.speed_strain / 0.0675).max(1.0) - 4.0).powi(3) / 100_000.0;

        // Longer maps are worth more
        let len_bonus = 0.95
            + 0.4 * (total_hits / 2000.0).min(1.0)
            + (total_hits > 2000.0) as u8 as f32 * 0.5 * (total_hits / 2000.0).log10();
        speed_value *= len_bonus;

        // Penalize misses
        speed_value *= 0.97_f32.powi(self.n_misses as i32);

        // Combo scaling
        if let Some(combo) = self.combo.filter(|_| attributes.max_combo > 0) {
            speed_value *= ((combo as f32 / attributes.max_combo as f32).powf(0.8)).min(1.0);
        }

        // AR bonus
        if attributes.ar > 10.33 {
            let ar_factor = 0.3 * (attributes.ar - 10.33);
            speed_value *= 1.0 + ar_factor;
        }

        // HD bonus
        if self.mods.hd() {
            speed_value *= 1.0 + 0.04 * (12.0 - attributes.ar);
        }

        // Scaling the speed value with accuracy and OD
        speed_value *= 0.02 + self.acc.unwrap();
        speed_value *= 0.96 + (attributes.od).powi(2) / 1600.0;

        speed_value
    }

    fn compute_accuracy_value_v2019(&self, total_hits: usize) -> f32 {
        let attributes = self.attributes.as_ref().unwrap();
        let n_circles = attributes.n_circles;

        let n300 = self.n300.unwrap_or(0);
        let n100 = self.n100.unwrap_or(0);
        let n50 = self.n50.unwrap_or(0);

        let better_acc_percentage = (n_circles > 0) as u8 as f32
            * (((n300 - (total_hits - n_circles)) * 6 + n100 * 2 + n50) as f32
                / (n_circles * 6) as f32)
                .max(0.0);

        let mut acc_value = 1.52163_f32.powf(attributes.od) * better_acc_percentage.powi(24) * 2.83;

        // Bonus for many hitcircles
        acc_value *= ((n_circles as f32 / 1000.0).powf(0.3)).min(1.15);

        // HD bonus
        if self.mods.hd() {
            acc_value *= 1.08;
        }

        // FL bonus
        if self.mods.fl() {
            acc_value *= 1.02;
        }

        acc_value
    }
}
