# Bathbot
Work in progress discord bot port for the game [osu!](https://osu.ppy.sh/home) from [PP-Generator](https://github.com/MaxOhn/PP-Generator) to rust

## Setup
- Clone this repo __with submodules__ via `git clone --recurse-submodules https://github.com/MaxOhn/Bathbot.git`
    - if the repo is already cloned, update the submodules via `git submodule update --init --recursive`
- [roppai](https://github.com/MaxOhn/roppai) must be cloned into the same directory as this repo (be sure roppai can be built by following its Readme)
- Be sure there is a MySQL server runnning which the application can access
  - In case MySQL is missing, [download](https://dev.mysql.com/downloads/installer/) it
  - Install as full package and start the server, specifically make sure MySQL-Connector-C-6.1 or similar is included
  - Add the system environment variable `MYSQLCLIENT_LIB_DIR` with the path `/path/to/mysql/connector/lib/vs14`
- Create a new database on the MySQL server and add its path to the `.env` file e.g. `mysql://username:password@localhost/db_name`
- Add [diesel](https://diesel.rs/)'s CLI tool via `cargo install diesel_cli --no-default-features --features mysql` (in the directory of this repo)
- Create all required tables for the database via `diesel migration run` (in the directory of this repo)
- Done

## Todos
- Distribute `Top` role automatically
- (Mixer notifications)
- (Numbered commands i.e. `<recent7`)
- (Mania star calculation)
  ### Commands
   - `<bg`
   - Lyrics
   - (`<reach`)
   - (`<trackedstreams`)
   - (`<allstreams`)