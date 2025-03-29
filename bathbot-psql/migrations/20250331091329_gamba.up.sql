CREATE TABLE IF NOT EXISTS bathcoins (
    osu_id INT4 NOT NULL,
    amount INT8 NOT NULL,
    PRIMARY KEY (osu_id),
    constraint amount_positive CHECK (amount >= 0)
);