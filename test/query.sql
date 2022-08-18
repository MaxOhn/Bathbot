WITH stats AS (
    SELECT
        global_rank,
        pp,
        last_update
    FROM
        osu_user_stats_mode
    WHERE
        mode = 0
        AND now() - last_update < interval '1 days'
)
SELECT
    *
FROM ((
        SELECT
            global_rank,
            pp
        FROM (
            SELECT
                *
            FROM
                stats
            WHERE
                pp > 8000
            ORDER BY
                pp ASC
            LIMIT 2) AS innerTable
    ORDER BY
        last_update DESC
    LIMIT 1)
UNION ALL (
    SELECT
        global_rank,
        pp
    FROM (
        SELECT
            *
        FROM
            stats
        WHERE
            pp < 8000
        ORDER BY
            pp DESC
        LIMIT 2) AS innerTable
ORDER BY
    last_update DESC
LIMIT 1)) AS neighbors;

