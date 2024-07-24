ALTER TABLE guild_configs ALTER COLUMN score_data TYPE BOOLEAN USING CASE WHEN score_data = 0 THEN true WHEN score_data = NULL THEN NULL ELSE false END;
ALTER TABLE guild_configs RENAME COLUMN score_data TO legacy_scores;

ALTER TABLE user_configs ALTER COLUMN score_data TYPE BOOLEAN USING CASE WHEN score_data = 0 THEN true WHEN score_data = NULL THEN NULL ELSE false END;
ALTER TABLE user_configs RENAME COLUMN score_data TO legacy_scores;