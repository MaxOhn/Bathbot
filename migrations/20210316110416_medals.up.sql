CREATE TABLE medals (
    medal_id     INT4 NOT NULL,
    name         VARCHAR(40) NOT NULL,
    description  TEXT NOT NULL,
    grouping     VARCHAR(30) NOT NULL,
    icon_url     TEXT NOT NULL,
    instructions TEXT,
    mode         INT2,

    PRIMARY KEY (medal_id)
);

CREATE INDEX medals_grouping ON medals (grouping);