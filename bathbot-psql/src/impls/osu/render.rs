use eyre::{Result, WrapErr};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{model::render::DbRenderOptions, Database};

impl Database {
    pub async fn select_user_render_settings(
        &self,
        user_id: Id<UserMarker>,
    ) -> Result<Option<DbRenderOptions>> {
        let query = sqlx::query_as!(
            DbRenderOptions,
            r#"
SELECT 
  official_skin_name, 
  official_skin_display_name, 
  custom_skin_id, 
  custom_skin_display_name, 
  global_volume, 
  music_volume, 
  hitsound_volume, 
  show_hit_error_meter, 
  show_unstable_rate, 
  show_score, 
  show_hp_bar, 
  show_combo_counter, 
  show_pp_counter, 
  show_key_overlay, 
  show_scoreboard, 
  show_borders, 
  show_mods, 
  show_result_screen, 
  use_skin_cursor, 
  use_skin_hitsounds, 
  use_beatmap_colors, 
  cursor_scale_to_cs, 
  cursor_rainbow, 
  cursor_trail_glow, 
  draw_follow_points, 
  draw_combo_numbers, 
  cursor_size, 
  cursor_trail, 
  beat_scaling, 
  slider_merge, 
  objects_rainbow, 
  flash_objects, 
  use_slider_hitcircle_color, 
  seizure_warning, 
  load_storyboard, 
  load_video, 
  intro_bg_dim, 
  ingame_bg_dim, 
  break_bg_dim, 
  bg_parallax, 
  show_danser_logo, 
  skip_intro, 
  cursor_ripples, 
  slider_snaking_in, 
  slider_snaking_out, 
  show_hit_counter, 
  show_avatars_on_scoreboard, 
  show_aim_error_meter, 
  play_nightcore_samples 
FROM 
  user_render_settings 
WHERE 
  discord_id = $1"#,
            user_id.get() as i64
        );

        query
            .fetch_optional(self)
            .await
            .wrap_err("Failed to fetch optional")
    }

    pub async fn upsert_user_render_settings(
        &self,
        user_id: Id<UserMarker>,
        settings: &DbRenderOptions,
    ) -> Result<()> {
        let DbRenderOptions {
            official_skin_name,
            official_skin_display_name,
            custom_skin_id,
            custom_skin_display_name,
            global_volume,
            music_volume,
            hitsound_volume,
            show_hit_error_meter,
            show_unstable_rate,
            show_score,
            show_hp_bar,
            show_combo_counter,
            show_pp_counter,
            show_key_overlay,
            show_scoreboard,
            show_borders,
            show_mods,
            show_result_screen,
            use_skin_cursor,
            use_skin_hitsounds,
            use_beatmap_colors,
            cursor_scale_to_cs,
            cursor_rainbow,
            cursor_trail_glow,
            draw_follow_points,
            draw_combo_numbers,
            cursor_size,
            cursor_trail,
            beat_scaling,
            slider_merge,
            objects_rainbow,
            flash_objects,
            use_slider_hitcircle_color,
            seizure_warning,
            load_storyboard,
            load_video,
            intro_bg_dim,
            ingame_bg_dim,
            break_bg_dim,
            bg_parallax,
            show_danser_logo,
            skip_intro,
            cursor_ripples,
            slider_snaking_in,
            slider_snaking_out,
            show_hit_counter,
            show_avatars_on_scoreboard,
            show_aim_error_meter,
            play_nightcore_samples,
        } = settings;

        let query = sqlx::query!(
            r#"
INSERT INTO user_render_settings (
  discord_id, official_skin_name, official_skin_display_name, 
  custom_skin_id, custom_skin_display_name, 
  global_volume, music_volume, hitsound_volume, 
  show_hit_error_meter, show_unstable_rate, 
  show_score, show_hp_bar, show_combo_counter, 
  show_pp_counter, show_key_overlay, 
  show_scoreboard, show_borders, show_mods, 
  show_result_screen, use_skin_cursor, 
  use_skin_hitsounds, use_beatmap_colors, 
  cursor_scale_to_cs, cursor_rainbow, 
  cursor_trail_glow, draw_follow_points, 
  draw_combo_numbers, cursor_size, 
  cursor_trail, beat_scaling, slider_merge, 
  objects_rainbow, flash_objects, 
  use_slider_hitcircle_color, seizure_warning, 
  load_storyboard, load_video, intro_bg_dim, 
  ingame_bg_dim, break_bg_dim, bg_parallax, 
  show_danser_logo, skip_intro, cursor_ripples, 
  slider_snaking_in, slider_snaking_out, 
  show_hit_counter, show_avatars_on_scoreboard, 
  show_aim_error_meter, play_nightcore_samples
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11, $12, $13, $14, $15, $16, $17, $18, 
    $19, $20, $21, $22, $23, $24, $25, $26, 
    $27, $28, $29, $30, $31, $32, $33, $34, 
    $35, $36, $37, $38, $39, $40, $41, $42, 
    $43, $44, $45, $46, $47, $48, $49, $50
  ) ON CONFLICT (discord_id) DO 
UPDATE 
SET 
  official_skin_name = $2, 
  official_skin_display_name = $3, 
  custom_skin_id = $4, 
  custom_skin_display_name = $5, 
  global_volume = $6, 
  music_volume = $7, 
  hitsound_volume = $8, 
  show_hit_error_meter = $9, 
  show_unstable_rate = $10, 
  show_score = $11, 
  show_hp_bar = $12, 
  show_combo_counter = $13, 
  show_pp_counter = $14, 
  show_key_overlay = $15, 
  show_scoreboard = $16, 
  show_borders = $17, 
  show_mods = $18, 
  show_result_screen = $19, 
  use_skin_cursor = $20, 
  use_skin_hitsounds = $21, 
  use_beatmap_colors = $22, 
  cursor_scale_to_cs = $23, 
  cursor_rainbow = $24, 
  cursor_trail_glow = $25, 
  draw_follow_points = $26, 
  draw_combo_numbers = $27, 
  cursor_size = $28, 
  cursor_trail = $29, 
  beat_scaling = $30, 
  slider_merge = $31, 
  objects_rainbow = $32, 
  flash_objects = $33, 
  use_slider_hitcircle_color = $34, 
  seizure_warning = $35, 
  load_storyboard = $36, 
  load_video = $37, 
  intro_bg_dim = $38, 
  ingame_bg_dim = $39, 
  break_bg_dim = $40, 
  bg_parallax = $41, 
  show_danser_logo = $42, 
  skip_intro = $43, 
  cursor_ripples = $44, 
  slider_snaking_in = $45, 
  slider_snaking_out = $46, 
  show_hit_counter = $47, 
  show_avatars_on_scoreboard = $48, 
  show_aim_error_meter = $49, 
  play_nightcore_samples = $50"#,
            user_id.get() as i64,
            official_skin_name,
            official_skin_display_name,
            *custom_skin_id,
            custom_skin_display_name.as_deref(),
            global_volume,
            music_volume,
            hitsound_volume,
            show_hit_error_meter,
            show_unstable_rate,
            show_score,
            show_hp_bar,
            show_combo_counter,
            show_pp_counter,
            show_key_overlay,
            show_scoreboard,
            show_borders,
            show_mods,
            show_result_screen,
            use_skin_cursor,
            use_skin_hitsounds,
            use_beatmap_colors,
            cursor_scale_to_cs,
            cursor_rainbow,
            cursor_trail_glow,
            draw_follow_points,
            draw_combo_numbers,
            cursor_size,
            cursor_trail,
            beat_scaling,
            slider_merge,
            objects_rainbow,
            flash_objects,
            use_slider_hitcircle_color,
            seizure_warning,
            load_storyboard,
            load_video,
            intro_bg_dim,
            ingame_bg_dim,
            break_bg_dim,
            bg_parallax,
            show_danser_logo,
            skip_intro,
            cursor_ripples,
            slider_snaking_in,
            slider_snaking_out,
            show_hit_counter,
            show_avatars_on_scoreboard,
            show_aim_error_meter,
            play_nightcore_samples,
        );

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }

    pub async fn select_osu_replay(&self, score_id: u64) -> Result<Option<Box<[u8]>>> {
        struct DbReplay {
            replay: Vec<u8>,
        }

        let query = sqlx::query_as!(
            DbReplay,
            r#"
SELECT 
  replay 
FROM 
  osu_replays 
WHERE 
  score_id = $1"#,
            score_id as i64
        );

        query
            .fetch_optional(self)
            .await
            .map(|opt| opt.map(|row| row.replay.into_boxed_slice()))
            .wrap_err("Failed to fetch optional")
    }

    pub async fn insert_osu_replay(&self, score_id: u64, replay: &[u8]) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO osu_replays (score_id, replay) 
VALUES 
  ($1, $2) ON CONFLICT (score_id) DO NOTHING"#,
            score_id as i64,
            replay
        );

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }

    pub async fn select_replay_video_url(&self, score_id: u64) -> Result<Option<Box<str>>> {
        let query = sqlx::query!(
            r#"
SELECT 
  video_url 
FROM 
  render_video_urls 
WHERE 
  score_id = $1"#,
            score_id as i64
        );

        query
            .fetch_optional(self)
            .await
            .map(|opt| opt.map(|row| row.video_url.into_boxed_str()))
            .wrap_err("Failed to fetch optional")
    }

    pub async fn upsert_replay_video_url(&self, score_id: u64, video_url: &str) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO render_video_urls (score_id, video_url) 
VALUES 
  ($1, $2) ON CONFLICT (score_id) DO 
UPDATE 
SET 
  video_url = $2"#,
            score_id as i64,
            video_url
        );

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }
}
