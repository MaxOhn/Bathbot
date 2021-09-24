CREATE TABLE osekai_medals (
    medal_id INT4 NOT NULL,
    name TEXT NOT NULL,
    icon_url TEXT NOT NULL,
    description TEXT NOT NULL,
    restriction INT2,
    grouping TEXT NOT NULL,
    solution TEXT,
    mods INT4,
    mode_order INT8 NOT NULL,
    ordering INT8 NOT NULL,

    PRIMARY KEY (medal_id)
);

CREATE INDEX osekai_medal_name ON osekai_medals (name);
CREATE INDEX osekai_medal_grouping ON osekai_medals (grouping);
