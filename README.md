<p align="center">
    <img src="https://raw.githubusercontent.com/MaxOhn/Bathbot/main/media/bb-text-coloured-hori.svg" alt="Bathbot" width=50% height=50%>
</p>
<p align="center">
    A feature-rich discord bot with functionality all around [osu!](https://osu.ppy.sh/)
</p>
<p align="center">
    <a href="https://discord.gg/n9fFstG">
        <img src="https://img.shields.io/discord/741040473476694159?color=%237289DA&label=Bathbots%20workshop&logo=discord&style=for-the-badge" alt="Discord">
    </a>
    <a href="https://ko-fi.com/T6T0BTB5T">
        <img src="https://ko-fi.com/img/githubbutton_sm.svg" alt="Ko-fi">
    </a>
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
- [Adri](https://osu.ppy.sh/users/4579132) and his website [osudaily](https://osudaily.net/) for providing rank data
- [mulraf](https://osu.ppy.sh/users/1309242), [Hubz](https://osu.ppy.sh/users/10379965), and the rest of the [osekai](https://osekai.net/) team for providing medal data
- [nzbasic](https://osu.ppy.sh/users/9008211) and his website [osutracker](https://osutracker.com/) for providing global data and country top scores
- [OMKelderman](https://osu.ppy.sh/users/2756335) and his [flag conversion](https://osuflags.omkserver.nl/) service

## Internals

- Difficulty & performance calculation: [rosu-pp](https://github.com/MaxOhn/rosu-pp)
- osu!api: [rosu-v2](https://github.com/MaxOhn/rosu-v2)
- Discord: [twilight](https://github.com/twilight-rs/twilight)
- Database: [sqlx](https://github.com/launchbadge/sqlx) (postgres)
- Redis: [bb8-redis](https://github.com/djc/bb8)
- Server: [routerify](https://github.com/routerify/routerify)

## Setup

Copy the content of `.env.example` into a new file `.env` and provide all of its variables. The most important ones are
- `DATABASE_URL`
- `DISCORD_TOKEN`
- `OSU_CLIENT_ID`
- `OSU_CLIENT_SECRET`
- `MAP_PATH`
- `REDIS_HOST`
- `REDIS_PORT`

Next, you need to migrate the database schema. For that, either make sure you have `sqlx-cli` installed (e.g. via `cargo install sqlx-cli --no-default-features --features rustls,postgres`) so you can run `sqlx migrate run`, or check out all `*.up.sql` files in `/migrations` and execute them manually in the correct order.

That should be all. When running the bot, try to work through all remaining error messages it throws at you :)

The bot also exposes metric data to `INTERNAL_IP:INTERNAL_PORT/metrics` (`.env` variables) which you can make use of through something like [prometheus](https://prometheus.io/) and visualize through [grafana](https://grafana.com/).