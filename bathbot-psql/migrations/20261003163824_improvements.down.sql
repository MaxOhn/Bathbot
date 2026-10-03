DROP INDEX osu_maps_mapset_index;

CREATE INDEX map_bookmarks_user_index ON user_map_bookmarks (user_id);
