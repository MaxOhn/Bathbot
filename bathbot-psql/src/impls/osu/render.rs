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
  use_skin_colors, 
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
  show_strain_graph, 
  show_slider_breaks, 
  ignore_fail 
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
            use_skin_colors,
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
            show_strain_graph,
            show_slider_breaks,
            ignore_fail,
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
  use_skin_colors, use_skin_hitsounds, 
  use_beatmap_colors, cursor_scale_to_cs, 
  cursor_rainbow, cursor_trail_glow, 
  draw_follow_points, draw_combo_numbers, 
  cursor_size, cursor_trail, beat_scaling, 
  slider_merge, objects_rainbow, flash_objects, 
  use_slider_hitcircle_color, seizure_warning, 
  load_storyboard, load_video, intro_bg_dim, 
  ingame_bg_dim, break_bg_dim, bg_parallax, 
  show_danser_logo, skip_intro, cursor_ripples, 
  slider_snaking_in, slider_snaking_out, 
  show_hit_counter, show_avatars_on_scoreboard, 
  show_aim_error_meter, play_nightcore_samples, 
  show_strain_graph, show_slider_breaks, ignore_fail
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11, $12, $13, $14, $15, $16, $17, $18, 
    $19, $20, $21, $22, $23, $24, $25, $26, 
    $27, $28, $29, $30, $31, $32, $33, $34, 
    $35, $36, $37, $38, $39, $40, $41, $42, 
    $43, $44, $45, $46, $47, $48, $49, $50, 
    $51, $52, $53, $54
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
  use_skin_colors = $21, 
  use_skin_hitsounds = $22, 
  use_beatmap_colors = $23, 
  cursor_scale_to_cs = $24, 
  cursor_rainbow = $25, 
  cursor_trail_glow = $26, 
  draw_follow_points = $27, 
  draw_combo_numbers = $28, 
  cursor_size = $29, 
  cursor_trail = $30, 
  beat_scaling = $31, 
  slider_merge = $32, 
  objects_rainbow = $33, 
  flash_objects = $34, 
  use_slider_hitcircle_color = $35, 
  seizure_warning = $36, 
  load_storyboard = $37, 
  load_video = $38, 
  intro_bg_dim = $39, 
  ingame_bg_dim = $40, 
  break_bg_dim = $41, 
  bg_parallax = $42, 
  show_danser_logo = $43, 
  skip_intro = $44, 
  cursor_ripples = $45, 
  slider_snaking_in = $46, 
  slider_snaking_out = $47, 
  show_hit_counter = $48, 
  show_avatars_on_scoreboard = $49, 
  show_aim_error_meter = $50, 
  play_nightcore_samples = $51, 
  show_strain_graph = $52, 
  show_slider_breaks = $53, 
  ignore_fail = $54"#,
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
            use_skin_colors,
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
            show_strain_graph,
            show_slider_breaks,
            ignore_fail,
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
