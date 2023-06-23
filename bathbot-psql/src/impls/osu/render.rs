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
  skin_id, 
  skin_name, 
  skin_presentation_name, 
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
            skin_id,
            skin_name,
            skin_presentation_name,
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
            r#"INSERT INTO user_render_settings (
  discord_id, skin_id, skin_name, skin_presentation_name, 
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
    $43, $44, $45, $46, $47, $48, $49
  ) ON CONFLICT (discord_id) DO 
UPDATE 
SET 
  skin_id = $2, 
  skin_name = $3, 
  skin_presentation_name = $4,
  global_volume = $5, 
  music_volume = $6, 
  hitsound_volume = $7, 
  show_hit_error_meter = $8, 
  show_unstable_rate = $9, 
  show_score = $10, 
  show_hp_bar = $11, 
  show_combo_counter = $12, 
  show_pp_counter = $13, 
  show_key_overlay = $14, 
  show_scoreboard = $15, 
  show_borders = $16, 
  show_mods = $17, 
  show_result_screen = $18, 
  use_skin_cursor = $19, 
  use_skin_hitsounds = $20, 
  use_beatmap_colors = $21, 
  cursor_scale_to_cs = $22, 
  cursor_rainbow = $23, 
  cursor_trail_glow = $24, 
  draw_follow_points = $25, 
  draw_combo_numbers = $26, 
  cursor_size = $27, 
  cursor_trail = $28, 
  beat_scaling = $29, 
  slider_merge = $30, 
  objects_rainbow = $31, 
  flash_objects = $32, 
  use_slider_hitcircle_color = $33, 
  seizure_warning = $34, 
  load_storyboard = $35, 
  load_video = $36, 
  intro_bg_dim = $37, 
  ingame_bg_dim = $38, 
  break_bg_dim = $39, 
  bg_parallax = $40, 
  show_danser_logo = $41, 
  skip_intro = $42, 
  cursor_ripples = $43, 
  slider_snaking_in = $44, 
  slider_snaking_out = $45, 
  show_hit_counter = $46, 
  show_avatars_on_scoreboard = $47, 
  show_aim_error_meter = $48, 
  play_nightcore_samples = $49"#,
            user_id.get() as i64,
            *skin_id,
            skin_name.as_deref(),
            skin_presentation_name,
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
