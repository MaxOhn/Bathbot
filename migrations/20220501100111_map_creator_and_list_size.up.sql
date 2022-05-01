DELETE FROM maps; -- oboi that's a big one
ALTER TABLE maps ADD COLUMN user_id INT4 NOT NULL;