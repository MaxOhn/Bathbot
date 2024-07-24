ALTER TABLE guild_configs RENAME COLUMN legacy_scores TO score_data;
ALTER TABLE guild_configs ALTER COLUMN score_data TYPE INT2 USING CASE WHEN score_data = true THEN 0 WHEN score_data = false THEN 1 ELSE NULL END;

ALTER TABLE user_configs RENAME COLUMN legacy_scores TO score_data;
ALTER TABLE user_configs ALTER COLUMN score_data TYPE INT2 USING CASE WHEN score_data = true THEN 0 WHEN score_data = false THEN 1 ELSE NULL END;