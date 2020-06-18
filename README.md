# Bathbot
Fully fledged discord bot with functionalities all around [osu!](https://osu.ppy.sh/home), aswell as some general utility and stream tracking.

With the bot's osu! commands you can
- check recent plays (`<r`)
- a background guessing game (`<bg`)
- display the personal topscores (with specified min acc, combo, or grade) (`<top`)
- show you best score with each mod combination on a map (`<scores`)
- compare top scores between players (`<common`)
- check the global or even belgian leaderboards of maps (`<glb`, `<lb`)
- calculate a performance rating for players of a multiplayer match (`<mc`) credits to [dain98](https://github.com/dain98/Minccino)
- simulate scores with arbitrary acc, combo, amount 300s, ... (`<s`)
- display a bunch of statistics all around a users osu profile (`<osu`, `<taiko`, ...)
- recalculate the personal top 100 if all scores were unchoked (`<nochokes`, `<nc`)
- and a bunch more

Moreover, the majority of commands is accessible for **all** gamemodes.
### To invite the bot to your server, use [this link](https://discordapp.com/api/oauth2/authorize?client_id=297073686916366336&permissions=268823616&scope=bot)
A spreadsheet with all current commands can be found [here](http://bit.ly/badecoms) although I can't guarantee the sheet will stay up-to-date

## Setup
In case you want to get the bot running yourself to either modify a custom instance for you, or just to contribute to the project, here's what you need to do:
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
  - Add [diesel](https://diesel.rs/)'s CLI tool via `cargo install diesel_cli --no-default-features --features mysql` (in the directory of this repo)
  - Create all required tables for the database via `diesel migration run` (in the directory of this repo)
- Handling osu-tools
  - If you can't use the command `dotnet` in the CLI, [download it](https://dotnet.microsoft.com/download) so you can compile C# code
  - Mania and CtB PP calculation is done via [osu-tools](https://github.com/MaxOhn/osu-tools) so clone it anywhere via `git clone https://github.com/MaxOhn/osu-tools.git`
  - Build osu-tools via `dotnet build -c Release`
  - Assign the variable `PERF_CALC` in the `.env` file to `PerformanceCalculator.dll` e.g. `/path/to/osu-tools/PerformanceCalculator/bin/Release/netcoreapp3.1/PerformanceCalculator.dll`
- Assign all other variables of the `.env.example` file into your `.env` file

## Todos
- Mixer stream tracking
- Move from diesel to sqlx
- Allow username provision via discord user mention
- Update spreadsheet (`<mostplayed`, `pagination`, ...)
- Automize the bot setup via docker
- Check on previous id buffer of bg game
- Fix "Error while sending HTTP request." for `<c`
  ### Commands
  - None as of now