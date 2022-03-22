ALTER TABLE mapsets ADD COLUMN genre INT2 NOT NULL DEFAULT 0;
ALTER TABLE mapsets ADD COLUMN language INT2 NOT NULL DEFAULT 0;

CREATE TABLE osekai_medals (
    medal_id    INT4 NOT NULL,
    name        TEXT NOT NULL,
    icon_url    TEXT NOT NULL,
    description TEXT NOT NULL,
    restriction INT2,
    grouping    TEXT NOT NULL,
    solution    TEXT,
    mods        INT4,
    mode_order  INT8 NOT NULL,
    ordering    INT8 NOT NULL,

    PRIMARY KEY (medal_id)
);