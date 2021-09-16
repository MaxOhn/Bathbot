CREATE TABLE IF NOT EXISTS user_configs (
    user_id INT8 NOT NULL,
    config   JSON NOT NULL,

    PRIMARY KEY (user_id)
);