pub struct TourneyBadges;

impl TourneyBadges {
    pub fn count<I, S>(badges: I) -> usize
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        fn filter_badge(badge: &str, buf: &mut String) -> bool {
            buf.clear();

            let iter = badge.bytes().map(|byte| byte.to_ascii_lowercase());

            // SAFETY: from `str::make_ascii_lowercase`:
            // "changing ASCII letters only does not invalidate UTF-8."
            unsafe { buf.as_mut_vec() }.extend(iter);

            TourneyBadges::is_tourney_badge(&*buf)
        }

        let mut lowercase = String::new();

        badges
            .into_iter()
            .filter(|badge| filter_badge(badge.as_ref(), &mut lowercase))
            .count()
    }

    fn is_tourney_badge(badge: &str) -> bool {
        !(badge.contains("fanart contest")
            || badge.contains(" art contest")
            || badge.starts_with("art contest")
            || badge.starts_with("aspire")
            || badge.starts_with("assessment")
            || badge.starts_with("beatmap")
            || badge.starts_with("community choice")
            || badge.starts_with("contrib")
            || badge.starts_with("centurion mapper")
            || badge.starts_with("elite")
            || badge.starts_with("exemplary")
            || badge.starts_with("global")
            || (badge.starts_with("idol") && !badge.starts_with("idol@"))
            || badge.starts_with("global")
            || badge.starts_with("longstanding")
            || (badge.starts_with("map") && !badge.starts_with("maple"))
            || badge.starts_with("moderation")
            || badge.starts_with("monthly")
            || badge.starts_with("nominat")
            || (badge.starts_with("osu!") && badge.contains("completionist"))
            || badge.starts_with("outstanding")
            || badge.starts_with("pending")
            || badge.starts_with("spotlight")
            || badge.contains("playlist")
            || badge.contains("pickem"))
    }
}

#[cfg(test)]
mod tests {
    use super::TourneyBadges;

    #[test]
    fn false_negatives() {
        let badges = [
            "Maple Cup 2015 Winner",            // /u/2155578
            "Belgian osu! Cup 2020",            // /u/7078544
            "osu! World Cup #3 Winning Team",   // /u/124493
            "iDOL@NSTER 2019 osu!mania Winner", // /u/8798383
        ];

        assert_eq!(TourneyBadges::count(badges), badges.len());
    }

    #[test]
    fn false_positives() {
        let badges = [
            "Elite Mapper 2011",                                               // /u/106
            "Pending Cup #3 Mapping Contest Winner",                           // /u/3076909
            "Mappers' Guild first level contributor",                          // /u/3181083
            "Centurion Mapper (100+ Beatmaps Ranked)",                         // /u/896613
            "osu! completionist (awarded 2023-03-12)",                         // /u/2927048
            "osu!taiko completionist (awarded 2019-11-03)",                    // /u/4841352
            "Nominated 200+ beatmaps as a Beatmap Nominator",                  // /u/3181083
            "Outstanding contribution to the Mentorship Project",              // /u/4945926
            "New Beginnings Art Contest Finalist (#1, 2203 votes)",            // /u/13103233
            "Beatmap Spotlights: Spring 2023 - osu!mania (Diamond 1)",         // temporary badge
            "Halloween 2022 Fanart Contest Finalist, (#1, 3749 votes)",        // /u/13103233
            "Longstanding commitment to World Cup Organisation (3 years)",     // /u/2155578
            "Exemplary performance as a Beatmap Nominator during 2021 (osu!)", // /u/3181083
            "Mapper's Choice Awards 2021: Top 3 in \
            the user/beatmap category Hitsounding", // /u/2155578
            "Aspire V Community Pick Grand Award: \
            Innovative Storyboarding (osu!) Runner Up and Song Title Runner Up", // /u/2330619
            "Featured Artist Playlist Leader: osu! (June 2022)",               // /u/12736534
            "OWC 2022 Pickem Winner",                                          // /u/3149577
        ];

        assert_eq!(TourneyBadges::count(badges), 0);
    }
}
