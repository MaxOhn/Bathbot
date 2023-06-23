CREATE TABLE IF NOT EXISTS osu_replays (
    score_id    INT8 NOT NULL,
    replay      BYTEA NOT NULL,
    PRIMARY KEY (score_id)
);

CREATE TABLE IF NOT EXISTS user_render_settings (
    discord_id                 INT8 NOT NULL,
    skin_id                    INT4,
    skin_name                  VARCHAR(128),
    skin_presentation_name     VARCHAR(128) NOT NULL,
    global_volume              INT2 NOT NULL,
    music_volume               INT2 NOT NULL,
    hitsound_volume            INT2 NOT NULL,
    show_hit_error_meter       BOOLEAN NOT NULL,
    show_unstable_rate         BOOLEAN NOT NULL,
    show_score                 BOOLEAN NOT NULL,
    show_hp_bar                BOOLEAN NOT NULL,
    show_combo_counter         BOOLEAN NOT NULL,
    show_pp_counter            BOOLEAN NOT NULL,
    show_key_overlay           BOOLEAN NOT NULL,
    show_scoreboard            BOOLEAN NOT NULL,
    show_borders               BOOLEAN NOT NULL,
    show_mods                  BOOLEAN NOT NULL,
    show_result_screen         BOOLEAN NOT NULL,
    use_skin_cursor            BOOLEAN NOT NULL,
    use_skin_hitsounds         BOOLEAN NOT NULL,
    use_beatmap_colors         BOOLEAN NOT NULL,
    cursor_scale_to_cs         BOOLEAN NOT NULL,
    cursor_rainbow             BOOLEAN NOT NULL,
    cursor_trail_glow          BOOLEAN NOT NULL,
    draw_follow_points         BOOLEAN NOT NULL,
    draw_combo_numbers         BOOLEAN NOT NULL,
    cursor_size                FLOAT4 NOT NULL,
    cursor_trail               BOOLEAN NOT NULL,
    beat_scaling               BOOLEAN NOT NULL,
    slider_merge               BOOLEAN NOT NULL,
    objects_rainbow            BOOLEAN NOT NULL,
    flash_objects              BOOLEAN NOT NULL,
    use_slider_hitcircle_color BOOLEAN NOT NULL,
    seizure_warning            BOOLEAN NOT NULL,
    load_storyboard            BOOLEAN NOT NULL,
    load_video                 BOOLEAN NOT NULL,
    intro_bg_dim               INT2 NOT NULL,
    ingame_bg_dim              INT2 NOT NULL,
    break_bg_dim               INT2 NOT NULL,
    bg_parallax                BOOLEAN NOT NULL,
    show_danser_logo           BOOLEAN NOT NULL,
    skip_intro                 BOOLEAN NOT NULL,
    cursor_ripples             BOOLEAN NOT NULL,
    slider_snaking_in          BOOLEAN NOT NULL,
    slider_snaking_out         BOOLEAN NOT NULL,
    show_hit_counter           BOOLEAN NOT NULL,
    show_avatars_on_scoreboard BOOLEAN NOT NULL,
    show_aim_error_meter       BOOLEAN NOT NULL,
    play_nightcore_samples     BOOLEAN NOT NULL,
    PRIMARY KEY (discord_id)
);

ALTER TABLE user_configs ADD COLUMN render_button BOOLEAN;
ALTER TABLE guild_configs ADD COLUMN render_button BOOLEAN;

CREATE TABLE IF NOT EXISTS render_video_urls (
    score_id  INT8 NOT NULL,
    video_url VARCHAR(128) NOT NULL,
    PRIMARY KEY (score_id)
);