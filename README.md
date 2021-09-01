## [2020.08.01] This is the previous Bathbot version. The current version is not (yet?) public so this repo will serve as public interface.

# Bathbot
Fully fledged discord bot with functionalities all around [osu!](https://osu.ppy.sh/home) including top score tracking and a background guessing game, aswell as twitch stream tracking and some general utility.
All osu! gamemodes are supported and there are slash commands for almost all regular commands.

With the bot's osu! commands you can
- check recent plays (`<r`)
- track top scores (`<track`, `<trackmania`, ...)
- a background guessing game (`<bg`)
- display the personal top scores with various filters and orderings (`<top`)
- show your best score on a map (`<c`)
- compare top scores among players (`<common`)
- check a map's global leaderboards (`<lb`)
- calculate a performance rating for players of a multiplayer match (`<mc`) credits to [dain98](https://github.com/dain98/Minccino)
- live track an ongoing multiplayer match (`<matchlive`)
- simulate scores with arbitrary acc, combo, amount 300s, ... (`<s`)
- display a bunch of statistics all around a users osu profile (`<osu`, `<taiko`, ...)
- recalculate the personal top 100 if all scores were unchoked (`<nc`)
- show all scores of a user that are in the top of a map's global leaderboard (`<osg`)
- and a ton more

With the `<help` command the bot will DM you a list of all available commands.
With `<help [command name]` (e.g. `<help osg`) the bot will explain the command, show how to use it and give examples.

### To invite the bot to your server, use [this link](https://discord.com/api/oauth2/authorize?client_id=297073686916366336&permissions=36776045632&scope=bot%20applications.commands)
You can also join its [discord server](https://discord.gg/n9fFstG) to keep up with updates, suggest things or report bugs

## Credits
- [5joshi](https://osu.ppy.sh/users/4279650) for these CRAZY GOOD reaction emotes :)
- Main icon from [syedhassan](https://pngtree.com/syedhassan_564486)
- Remaining reaction emotes from [Smashicons](https://www.flaticon.com/authors/smashicons)

## Setup
Trust me, you don't want to set this up, it will be a pain. Feel free to use the code in this repo as inspiration if you're working on something similar but I wouldn't even recommend that.
If you're interested in the code you can join the [discord server](https://discord.gg/n9fFstG) and ask me directly about it. I'm generally happy to provide any help and current code pieces you need :)

In case I didn't convince you and you still want to get the bot running yourself...

Note that there were some changes prior to moving to the new bathbot version but these instructions were not modified accordingly so they're not completely right but should get you on the right track:
- Clone this repo via `git clone --recurse-submodules https://github.com/MaxOhn/Bathbot.git`
- Handling oppai:
  - PP calculation for osu! and taiko is done via C-binding of [oppai](https://github.com/Francesco149/oppai-ng) so you will need the [LLVM](http://releases.llvm.org/download.html) C compiler
  - After installing LLVM, add the environment variable `LIBCLANG_PATH` which leads to the `bin` folder of the LLVM installation e.g. `C:\Program Files\LLVM\bin` (letting the installation put LLVM onto the PATH variable is not sufficient!)
- Handling the database:
  - Be sure there is a MySQL server runnning which the application can access
    - In case MySQL is missing, [download it](https://dev.mysql.com/downloads/installer/)
    - Install as full package and start the server, specifically make sure MySQL-Connector-C-6.1 or similar is included
    - Add the system environment variable `MYSQLCLIENT_LIB_DIR` with the path `/path/to/mysql/connector/lib/vs14`
  - Create a new database on the MySQL server and add its path to the `.env` file e.g. `mysql://username:password@localhost/db_name`
  - ~~Add [diesel](https://diesel.rs/)'s CLI tool via `cargo install diesel_cli --no-default-features --features mysql` (in the directory of this repo)~~
  - ~~Create all required tables for the database via `diesel migration run` (in the directory of this repo)~~
  - I moved from diesel to sqlx and table migration is not yet setup so you'll have to check in the `/migrations` directory and add tables manually for now :(
- Handling osu-tools
  - If you can't use the command `dotnet` in the CLI, [download it](https://dotnet.microsoft.com/download) so you can compile C# code
  - Mania and CtB PP calculation is done via [osu-tools](https://github.com/MaxOhn/osu-tools) so clone it anywhere via `git clone https://github.com/MaxOhn/osu-tools.git`
  - Build osu-tools via `dotnet build -c Release`
  - Assign the variable `PERF_CALC` in the `.env` file to `PerformanceCalculator.dll` e.g. `/path/to/osu-tools/PerformanceCalculator/bin/Release/netcoreapp3.1/PerformanceCalculator.dll`
- Assign all other variables of the `.env.example` file into your `.env` file
