DELETE FROM maps; -- oboi that's a big one
ALTER TABLE maps ADD COLUMN user_id INT4 NOT NULL;

ALTER TABLE user_configs ADD COLUMN list_size INT2;
ALTER TABLE guild_configs ADD COLUMN list_size INT2;