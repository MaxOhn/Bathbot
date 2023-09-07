ALTER TABLE user_render_settings ADD COLUMN show_strain_graph BOOLEAN NOT NULL DEFAULT false, ADD COLUMN show_slider_breaks BOOLEAN NOT NULL DEFAULT false, ADD COLUMN ignore_fail BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE user_render_settings ALTER COLUMN show_strain_graph DROP DEFAULT, ALTER COLUMN show_slider_breaks DROP DEFAULT, ALTER COLUMN ignore_fail DROP DEFAULT;
