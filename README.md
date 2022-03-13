![Banner](https://i.imgur.com/LNX6Db3.png)

# Bathbot

Fully fledged discord bot with functionalities all around [osu!](https://osu.ppy.sh/home) including top score tracking and a background guessing game, aswell as twitch stream tracking and some general utility.

All osu! gamemodes are supported and commands exist as slash commands aswell as prefix commands.

Some popular features of the bot:
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
- list all members of the server with respect to some attribute of their osu! profile like global rank, pp, medal count, ... (`/serverleaderboard`)
- and a ton more

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
- `TWITCH_CLIENT_ID`
- `TWITCH_TOKEN`
- `MAP_PATH`
- `REDIS_HOST`
- `REDIS_PORT`

Next, you need to migrate the database schema. For that, either make sure you have `sqlx-cli` installed (e.g. via `cargo install sqlx-cli --no-default-features --features rustls,postgres`) so you can run `sqlx migrate run`, or check out all `*.up.sql` files in `/migrations` and execute them manually in the correct order.

That should be all. When running the bot, try to work through all remaining error messages it throws at you :)

The bot also exposes metric data to `INTERNAL_IP:INTERNAL_PORT/metrics` (`.env` variables) which you can make use of through something like [prometheus](https://prometheus.io/) and visualize through [grafana](https://grafana.com/).