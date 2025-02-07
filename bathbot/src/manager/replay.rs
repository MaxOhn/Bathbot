use std::fmt::{Display, Formatter, Result as FmtResult};

use bathbot_cache::Cache as BathbotCache;
use bathbot_psql::{model::render::DbRenderOptions, Database};
use eyre::{Report, Result, WrapErr};
use rosu_render::model::{RenderOptions, RenderResolution, RenderSkinOption, Skin, SkinInfo};
use rosu_v2::{error::OsuError, Osu};
use twilight_model::id::{marker::UserMarker, Id};

#[derive(Copy, Clone)]
pub struct ReplayManager {
    psql: &'static Database,
    osu: &'static Osu,
    cache: &'static BathbotCache,
}

impl ReplayManager {
    pub fn new(psql: &'static Database, osu: &'static Osu, cache: &'static BathbotCache) -> Self {
        Self { psql, osu, cache }
    }

    pub async fn get_replay(self, score_id: u64) -> Result<Option<Box<[u8]>>, ReplayError> {
        match self.psql.select_osu_replay(score_id).await {
            Ok(Some(replay)) => return Ok(Some(replay)),
            Ok(None) => {}
            Err(err) => warn!(?err, "Failed to get replay from DB"),
        }

        // If the replay of a score id was not in the database, yet we requested it
        // already, that means the score has no available replay.
        let not_contained = self
            .cache
            .insert_into_set("__requested_replay_score_ids", score_id)
            .await
            .map_err(ReplayError::AlreadyRequestedCheck)?;

        if !not_contained {
            return Ok(None);
        }

        let replay = self
            .osu
            .replay_raw(score_id)
            .await
            .map_err(ReplayError::Osu)?;

        if let Err(err) = self.psql.insert_osu_replay(score_id, &replay).await {
            warn!(?err, "Failed to insert replay into DB");
        }

        Ok(Some(replay.into_boxed_slice()))
    }

    pub async fn get_settings(self, user: Id<UserMarker>) -> Result<ReplaySettings> {
        let options = self
            .psql
            .select_user_render_settings(user)
            .await
            .wrap_err("Failed to load settings")?;

        match options {
            Some(options) => Ok(ReplaySettings::from(options)),
            None => {
                let settings = ReplaySettings::default();

                if let Err(err) = self.set_settings(user, &settings).await {
                    warn!(?err);
                }

                Ok(settings)
            }
        }
    }

    pub async fn set_settings(self, user: Id<UserMarker>, settings: &ReplaySettings) -> Result<()> {
        let db_options = DbRenderOptions::from(settings);

        self.psql
            .upsert_user_render_settings(user, &db_options)
            .await
            .wrap_err("Failed to upsert settings")
    }

    pub async fn get_video_url(&self, score_id: u64) -> Result<Option<Box<str>>> {
        self.psql
            .select_replay_video_url(score_id)
            .await
            .wrap_err("Failed to get replay video url")
    }

    pub async fn store_video_url(&self, score_id: u64, video_url: &str) -> Result<()> {
        self.psql
            .upsert_replay_video_url(score_id, video_url)
            .await
            .wrap_err("Failed to store replay video url")
    }
}

pub enum ReplayError {
    Osu(OsuError),
    AlreadyRequestedCheck(Report),
}

impl ReplayError {
    pub const ALREADY_REQUESTED_TEXT: &str = "Failed to check whether replay was already requested";
}

#[derive(Default)]
pub struct ReplaySettings {
    options: RenderOptions,
    official_skin: ReplaySkin,
    custom_skin: Option<ReplaySkin>,
}

pub struct ReplaySkin {
    pub skin: RenderSkinOption<'static>,
    pub display_name: Box<str>,
}

impl Default for ReplaySkin {
    fn default() -> Self {
        Self {
            skin: RenderSkinOption::Official {
                name: "default".into(),
            },
            display_name: "Danser default skin (Redd glass)".into(),
        }
    }
}

impl ReplaySettings {
    pub fn new_with_official_skin(options: RenderOptions, skin: Skin) -> Self {
        Self {
            options,
            official_skin: ReplaySkin {
                skin: RenderSkinOption::from(skin.skin.into_string()),
                display_name: skin.presentation_name,
            },
            custom_skin: None,
        }
    }

    pub fn new_with_custom_skin(options: RenderOptions, skin: SkinInfo, id: u32) -> Self {
        Self {
            options,
            official_skin: ReplaySkin::default(),
            custom_skin: Some(ReplaySkin {
                skin: RenderSkinOption::Custom { id },
                display_name: skin.name,
            }),
        }
    }

    pub fn options(&self) -> &RenderOptions {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut RenderOptions {
        &mut self.options
    }

    pub fn skin(&self, allow_custom_skin: bool) -> &ReplaySkin {
        if allow_custom_skin {
            self.custom_skin.as_ref().unwrap_or(&self.official_skin)
        } else {
            &self.official_skin
        }
    }

    pub fn official_skin(&mut self, skin: Skin) {
        self.official_skin = ReplaySkin {
            skin: RenderSkinOption::Official {
                name: skin.skin.into_string().into(),
            },
            display_name: skin.presentation_name,
        };
    }

    pub fn custom_skin(&mut self, id: u32, skin: SkinInfo) {
        self.custom_skin = Some(ReplaySkin {
            skin: RenderSkinOption::Custom { id },
            display_name: skin.name,
        });
    }

    pub fn remove_custom_skin(&mut self) {
        self.custom_skin.take();
    }

    pub fn skin_name(&self) -> (&str, Option<CustomSkinName<'_>>) {
        let custom = self.custom_skin.as_ref().map(|skin| {
            let RenderSkinOption::Custom { id } = skin.skin else {
                unreachable!()
            };

            CustomSkinName {
                name: skin.display_name.as_ref(),
                id,
            }
        });

        (self.official_skin.display_name.as_ref(), custom)
    }
}

pub struct CustomSkinName<'n> {
    name: &'n str,
    id: u32,
}

impl Display for CustomSkinName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{} (ID {})", self.name, self.id)
    }
}

impl From<DbRenderOptions> for ReplaySettings {
    fn from(options: DbRenderOptions) -> Self {
        let settings = RenderOptions {
            resolution: RenderResolution::HD720,
            global_volume: options.global_volume as u8,
            music_volume: options.music_volume as u8,
            hitsound_volume: options.hitsound_volume as u8,
            show_hit_error_meter: options.show_hit_error_meter,
            show_unstable_rate: options.show_unstable_rate,
            show_score: options.show_score,
            show_hp_bar: options.show_hp_bar,
            show_combo_counter: options.show_combo_counter,
            show_pp_counter: options.show_pp_counter,
            show_key_overlay: options.show_key_overlay,
            show_scoreboard: options.show_scoreboard,
            show_borders: options.show_borders,
            show_mods: options.show_mods,
            show_result_screen: options.show_result_screen,
            use_skin_cursor: options.use_skin_cursor,
            use_skin_colors: options.use_skin_colors,
            use_skin_hitsounds: options.use_skin_hitsounds,
            use_beatmap_colors: options.use_beatmap_colors,
            cursor_scale_to_cs: options.cursor_scale_to_cs,
            cursor_rainbow: options.cursor_rainbow,
            cursor_trail_glow: options.cursor_trail_glow,
            draw_follow_points: options.draw_follow_points,
            draw_combo_numbers: options.draw_combo_numbers,
            cursor_size: options.cursor_size,
            cursor_trail: options.cursor_trail,
            beat_scaling: options.beat_scaling,
            slider_merge: options.slider_merge,
            objects_rainbow: options.objects_rainbow,
            flash_objects: options.flash_objects,
            use_slider_hitcircle_color: options.use_slider_hitcircle_color,
            seizure_warning: options.seizure_warning,
            load_storyboard: options.load_storyboard,
            load_video: options.load_video,
            intro_bg_dim: options.intro_bg_dim as u8,
            ingame_bg_dim: options.ingame_bg_dim as u8,
            break_bg_dim: options.break_bg_dim as u8,
            bg_parallax: options.bg_parallax,
            show_danser_logo: options.show_danser_logo,
            skip_intro: options.skip_intro,
            cursor_ripples: options.cursor_ripples,
            slider_snaking_in: options.slider_snaking_in,
            slider_snaking_out: options.slider_snaking_out,
            show_hit_counter: options.show_hit_counter,
            show_avatars_on_scoreboard: options.show_avatars_on_scoreboard,
            show_aim_error_meter: options.show_aim_error_meter,
            play_nightcore_samples: options.play_nightcore_samples,
            show_strain_graph: options.show_strain_graph,
            show_slider_breaks: options.show_slider_breaks,
            ignore_fail: options.ignore_fail,
        };

        let official_skin = ReplaySkin {
            skin: RenderSkinOption::Official {
                name: options.official_skin_name.into(),
            },
            display_name: options.official_skin_display_name.into(),
        };

        let custom_skin = options
            .custom_skin_id
            .zip(options.custom_skin_display_name)
            .map(|(id, name)| ReplaySkin {
                skin: RenderSkinOption::Custom { id: id as u32 },
                display_name: name.into(),
            });

        Self {
            options: settings,
            official_skin,
            custom_skin,
        }
    }
}

impl From<&ReplaySettings> for DbRenderOptions {
    fn from(settings: &ReplaySettings) -> Self {
        let ReplaySettings {
            options,
            official_skin,
            custom_skin,
        } = settings;

        let RenderSkinOption::Official { ref name } = official_skin.skin else {
            unreachable!()
        };

        let (custom_skin_id, custom_skin_display_name) = match custom_skin {
            Some(skin) => {
                let RenderSkinOption::Custom { id } = skin.skin else {
                    unreachable!()
                };
                let name = skin.display_name.as_ref().to_string();

                (Some(id as i32), Some(name))
            }
            None => (None, None),
        };

        Self {
            official_skin_name: name.as_ref().to_string(),
            official_skin_display_name: official_skin.display_name.as_ref().to_string(),
            custom_skin_id,
            custom_skin_display_name,
            global_volume: options.global_volume as i16,
            music_volume: options.music_volume as i16,
            hitsound_volume: options.hitsound_volume as i16,
            show_hit_error_meter: options.show_hit_error_meter,
            show_unstable_rate: options.show_unstable_rate,
            show_score: options.show_score,
            show_hp_bar: options.show_hp_bar,
            show_combo_counter: options.show_combo_counter,
            show_pp_counter: options.show_pp_counter,
            show_key_overlay: options.show_key_overlay,
            show_scoreboard: options.show_scoreboard,
            show_borders: options.show_borders,
            show_mods: options.show_mods,
            show_result_screen: options.show_result_screen,
            use_skin_cursor: options.use_skin_cursor,
            use_skin_colors: options.use_skin_colors,
            use_skin_hitsounds: options.use_skin_hitsounds,
            use_beatmap_colors: options.use_beatmap_colors,
            cursor_scale_to_cs: options.cursor_scale_to_cs,
            cursor_rainbow: options.cursor_rainbow,
            cursor_trail_glow: options.cursor_trail_glow,
            draw_follow_points: options.draw_follow_points,
            draw_combo_numbers: options.draw_combo_numbers,
            cursor_size: options.cursor_size,
            cursor_trail: options.cursor_trail,
            beat_scaling: options.beat_scaling,
            slider_merge: options.slider_merge,
            objects_rainbow: options.objects_rainbow,
            flash_objects: options.flash_objects,
            use_slider_hitcircle_color: options.use_slider_hitcircle_color,
            seizure_warning: options.seizure_warning,
            load_storyboard: options.load_storyboard,
            load_video: options.load_video,
            intro_bg_dim: options.intro_bg_dim as i16,
            ingame_bg_dim: options.ingame_bg_dim as i16,
            break_bg_dim: options.break_bg_dim as i16,
            bg_parallax: options.bg_parallax,
            show_danser_logo: options.show_danser_logo,
            skip_intro: options.skip_intro,
            cursor_ripples: options.cursor_ripples,
            slider_snaking_in: options.slider_snaking_in,
            slider_snaking_out: options.slider_snaking_out,
            show_hit_counter: options.show_hit_counter,
            show_avatars_on_scoreboard: options.show_avatars_on_scoreboard,
            show_aim_error_meter: options.show_aim_error_meter,
            play_nightcore_samples: options.play_nightcore_samples,
            show_strain_graph: options.show_strain_graph,
            show_slider_breaks: options.show_slider_breaks,
            ignore_fail: options.ignore_fail,
        }
    }
}
