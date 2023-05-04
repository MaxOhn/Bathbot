CREATE TABLE IF NOT EXISTS huismetbenen_countries (
    country_name VARCHAR(32) NOT NULL,
    country_code VARCHAR(2) NOT NULL,
    CHECK (country_code = UPPER(country_code)),
    PRIMARY KEY (country_code)
);