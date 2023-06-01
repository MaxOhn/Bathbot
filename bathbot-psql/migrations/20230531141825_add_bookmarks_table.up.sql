CREATE TABLE IF NOT EXISTS user_map_bookmarks (
    user_id     INT8 NOT NULL,
    map_id      INT4 NOT NULL,
    insert_date TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, map_id)
);

CREATE INDEX map_bookmarks_user_index ON user_map_bookmarks (user_id);