<p align="center">
    <img src="https://raw.githubusercontent.com/MaxOhn/Bathbot/main/media/bb-text-coloured-hori.svg" alt="Bathbot" width=50% height=50%>
</p>
<p align="center">
    A feature-rich discord bot with functionality all around <a href="https://osu.ppy.sh">osu!</a>
</p>
<p align="center">
    <a href="https://discord.gg/n9fFstG">
        <img src="https://img.shields.io/discord/741040473476694159?color=%237289DA&label=Bathbots%20workshop&logo=discord&style=for-the-badge" alt="Discord">
    </a>
    <a href="https://ko-fi.com/T6T0BTB5T">
        <img src="https://ko-fi.com/img/githubbutton_sm.svg" alt="Ko-fi">
    </a>
</p>
<p align="center">
    <img src="https://img.shields.io/badge/dynamic/json?color=blueviolet&label=server%20count&query=guild_count&url=https%3A%2F%2Fbathbot.ddns.net%2Fguild_count&style=flat&cacheSeconds=3600" alt="Server count">
    <img src="https://tokei.rs/b1/github/MaxOhn/Bathbot?category=code" alt="Lines of Code">
</p>

# Features

- check recent plays (`<r` / `/rs`)
- track top scores (`<track`, `<trackmania`, ... / `/track`)
- a background guessing game (`/bg`)
- display the personal top scores with various filters and orderings (`<top` / `/top`)
- show your best scores on a map (`<c` / `/cs`)
- compare top scores among players (`<common` / `/compare top`)
- check a map's global leaderboards (`<lb` / `/leaderboard`)
- calculate a performance rating for players of a multiplayer match (`<mc` / `/matchcost`)
- live track an ongoing multiplayer match (`/matchlive`)
- simulate scores with arbitrary acc, combo, amount 300s, ... (`/simulate`)
- display a bunch of statistics all around a users osu profile (`<osu`, `<taiko`, ... / `/profile`)
- recalculate the personal top 100 if all scores were unchoked (`<nc` / `/nochoke`)
- show all scores of a user that are in the top of a map's global leaderboard (`<osg` / `/osustats scores`)
- list server members in order of some attribute in their osu! profile like pp, medal count, ... (`/serverleaderboard`)
- notify a channel when a twitch streams comes online (`/trackstream` / `<addstream`)
- ... and a ton more

All osu! gamemodes are supported and commands exist as slash commands as well as prefix commands.

With the `<help` command the bot will DM you a list of all available prefix commands. With `<help [command name]` (e.g. `<help osg`) the bot will explain the command, show how to use it and give examples.

For help on slash commands, use the `/help` command.

### To invite the bot to your server, use [this link](https://discord.com/api/oauth2/authorize?client_id=297073686916366336&permissions=36776045632&scope=bot%20applications.commands).
You can also join its [discord server](https://discord.gg/n9fFstG) to keep up with updates, suggest features or report bugs.

## Credits
- [Hubz](https://osu.ppy.sh/users/10379965) for the main icon and slick webpages
- [5joshi](https://osu.ppy.sh/users/4279650) for these CRAZY GOOD reaction emotes :)
- [Mr Helix](https://osu.ppy.sh/users/2330619) and his website [huismetbenen](https://snipe.huismetbenen.nl/) for providing snipe data
- [Piotrekol](https://osu.ppy.sh/users/304520) and [Ezoda](https://osu.ppy.sh/users/1231180) and their website [osustats](https://osustats.ppy.sh/) for providing leaderboard data
- [mulraf](https://osu.ppy.sh/users/1309242), [Hubz](https://osu.ppy.sh/users/10379965), and the rest of the [osekai](https://osekai.net/) team for providing medal data
- [nzbasic](https://osu.ppy.sh/users/9008211) and his website [osutracker](https://osutracker.com/) for providing global data and country top scores
- [OMKelderman](https://osu.ppy.sh/users/2756335) and his [flag conversion](https://osuflags.omkserver.nl/) service
- [respektive](https://osu.ppy.sh/users/1023489) for his [score rank api](https://github.com/respektive/osu-profile#score-rank-api) and [osustats api](https://github.com/respektive/osustats)

## Internals

- Difficulty & performance calculation: [rosu-pp](https://github.com/MaxOhn/rosu-pp)
- osu!api: [rosu-v2](https://github.com/MaxOhn/rosu-v2)
- Discord: [twilight](https://github.com/twilight-rs/twilight)
- Database: [sqlx](https://github.com/launchbadge/sqlx) (postgres)
- Redis: [bb8-redis](https://github.com/djc/bb8)
- Server: [routerify](https://github.com/routerify/routerify)

## Setup

I wouldn't necessarily recommend to try and get the bot running yourself but feel free to give it a shot.

[Rust](https://www.rust-lang.org/) must be installed and additionally either [docker](https://www.docker.com/) must be installed to setup the databases automatically (recommended) or [postgres](https://www.postgresql.org/) and [redis](https://redis.io/) must be installed manually.

- Copy the content of `.env.example` into a new file `.env` and provide all of its variables. The most important ones are
  - `DISCORD_TOKEN`
  - `OSU_CLIENT_ID`
  - `OSU_CLIENT_SECRET`
  - `MAP_PATH`
- If you don't run through docker, be sure these env variables are also set
  - `DATABASE_URL`
  - `REDIS_HOST`
  - `REDIS_PORT`
- If you do run through docker, you can
  - boot up the databases with `docker-compose up -d` (must be done)
  - use `docker ps` to make sure `bathbot-db` and `bathbot-redis` have the status `Up` 
  - inspect the postgres container with `docker exec -it bathbot-db psql -U bathbot -d bathbot`
  - inspect the redis container with `docker exec -it bathbot-redis redis-cli`
  - shut the databases down with `docker-compose down`
- Next, install `sqlx-cli` if you haven't already. You can do so with `cargo install sqlx-cli --no-default-features --features postgres,rustls`.
- Then migrate the database with `sqlx migrate run`. This command will complain if the `DATABASE_URL` variable in `.env` is not correct.
- And finally you can compile and run the bot with `cargo run`. Be sure you have a hobby or some other activity to do while you get to enjoy the rust compilation times™️.

If the `--release` flag is set when compiling, the bot will be faster and have a few additional features such as
- host a server on `INTERNAL_IP:INTERNAL_PORT` (`.env` variables) with endpoints related to linking osu! accounts or `/metrics` to expose metric data which you can make use of through something like [prometheus](https://prometheus.io/) and visualize with [grafana](https://grafana.com/)
- osu! top score tracking
- twitch stream tracking
- matchlive tracking
