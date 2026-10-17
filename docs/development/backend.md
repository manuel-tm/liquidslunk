# Backend

> Keep this page up to date: any change to the backend's stack, structure, commands or conventions updates it in the same change.

The backend is Rust, built into the app. It's being ported from slunk's Node backend one context at a time ([design §8](../design/1_introduction.md#8-the-transition)). Phase 0 built:
- the database connection, the migration runner with its backups, slunk's eight migrations, and their parity tests;
- the database commands, in `crates/slunk-cli`;
- the app: commands typed by tauri-specta, their error, the settings, opening, creating, switching and closing libraries, the file protocol, and opening files;
- the sidecar: slunk's backend, which still serves every route not ported yet, in debug builds.

slunk's contexts (media, catalog, tagging, library check) aren't ported yet. Their rules are in slunk's backend doc, `legacy/docs/development/backend.md`, and a rule moves here only once Rust does it.

## Stack

| Piece | Choice |
|---|---|
| Shell | Tauri 2 |
| Commands | Tauri commands, typed by tauri-specta |
| SQLite | rusqlite, with the `bundled` feature: SQLite is compiled in |
| Errors | thiserror |
| Plugins | tauri-plugin-dialog (choosing a library), tauri-plugin-opener (Finder, opening files), tauri-plugin-single-instance |
| Content types | mime_guess |
| CLI | clap |
| Screenshots | tauri-plugin-wdio-webdriver, behind the `webdriver` feature |

tauri-specta, specta and specta-typescript are release candidates, so their versions are pinned exactly in `src-tauri/Cargo.toml`. The Rust version is pinned in `mise.toml`.

rusqlite's bundled SQLite turns foreign keys on for every new connection, unlike better-sqlite3. Code that needs them off, like the migration runner, sets the pragma itself.

## Structure

```
Cargo.toml                   the Cargo workspace
crates/slunk/                the domain and its database, with no Tauri dependency
  src/error.rs               the domain's error
  src/media.rs               where a registered file is on disk
  src/repo/
    connection.rs            opening SQLite with the connection settings
    migrator.rs              the migration runner
    backups.rs               backups, and removing old ones
    rebuild_table.rs         rebuild_table(), for migrations that change a table
    files.rs                 registered files
    migrations/              one module per migration, listed in mod.rs
  tests/parity.rs            the migrations, checked against slunk's
  tests/fixtures/parity/     what slunk's runner made, and the script that made it
crates/slunk-cli/            the database commands (npm run db:*)
src-tauri/
  tauri.conf.json            the app's name, identifier, window, CSP and bundle
  capabilities/default.json  what the window may call
  icons/                     Tauri's placeholder icons
  sidecar-watchdog.cjs       stops the sidecar if the app is gone
  src/lib.rs                 the builder: plugins, state, protocol, commands
  src/error.rs               the commands' error, and SQLite's errors in words
  src/settings.rs            the settings file
  src/library.rs             opening, creating, switching and closing libraries
  src/protocol.rs            slunk://localhost/files/:id
  src/sidecar.rs             slunk's backend, in debug builds
  src/commands/              the commands and their DTOs, one module per context
legacy/slunk-be/             slunk's backend, run as the sidecar
```

`crates/slunk` doesn't depend on Tauri, so its tests run with a plain `cargo test`, and the CLI uses it directly. Its repositories are plain functions over a `&Connection`, and a `Transaction` is one.

## The database

### Connection

`connection::open()` opens the app's connection, with `foreign_keys = ON`, `journal_mode = WAL` and `busy_timeout = 5000`. The database must exist: `migrate` creates it. There's one connection, behind a mutex. Dropping it, when it's the last connection to the database, checkpoints the WAL and removes the `-wal` and `-shm` files. Files are read before taking the connection, never while holding it.

liquidslunk writes only next to the database: the database file itself, its `-wal`/`-shm` files while it's open, and `slunk-backups/`. Apart from those, it writes its settings in the app's config folder.

The schema is described in [reference/database.md](../reference/database.md).

### Running migrations

The runner is `repo/migrator.rs`, ported from slunk's. `migrate` does the following:

1. Opens its own connection with foreign keys off, so a table rebuild can't cascade deletes.
2. Compares the migrations with the `migrations` table. A library with an applied migration this version doesn't know is refused: "This library was upgraded by a newer version of liquidslunk." So is a pending migration that sorts before the last applied one.
3. If anything is pending, backs the database up with `VACUUM INTO` to `slunk-backups/slunk-<timestamp>.db`, next to the database. A database that didn't exist yet isn't backed up. Two backups in the same second get a `-2`, `-3`, … suffix. Then it removes the oldest backups beyond the number to keep (`keepBackups` in the settings, or `--keep-backups` in the CLI; 10 by default). Only files named like its backups are removed; anything else in `slunk-backups/` stays.
4. Starts `BEGIN IMMEDIATE`, which takes the write lock, so the app and the CLI can't interleave. It reads what's pending again, then runs each `up` and records it in `migrations`, which it creates the first time.
5. Runs `PRAGMA foreign_key_check`, and rolls back if any row breaks a foreign key.
6. Commits.

So all pending migrations apply together or not at all. After a failure, the database is as it was, and the backup is still there. `rollback` works the same way, running `down` for the last *n* migrations, newest first.

The app runs `migrate` whenever it opens a library, so users never migrate by hand. The `migrations` table, its rows and the backups' names are slunk's, so slunk and liquidslunk read each other's.

### The migrations

Each migration is a module in `repo/migrations/`, `m<timestamp>_<name>.rs`, with an `up` and a `down` that take a `&Transaction`. `migrations![…]` in `mod.rs` declares them and lists them in order, and a test checks that the list is sorted and lists every module in the folder.

slunk's eight migrations are its Kysely migrations' statements, copied from what Kysely actually ran, so they build slunk's schema to the character. Each module runs its statements in order with `run()`, each prepared on its own, as Kysely ran them. Two have logic: `add_tag_value_rules` checks the rows already there before adding its triggers, and `add_credited_names` rebuilds four tables.

The rules:
- Never edit a migration once it has been committed. Write a new one.
- No new migrations until the cut-over, phase 6: slunk's backend, the sidecar, wouldn't know the new schema.
- Every `down` undoes its `up`, or fails with an explicit "irreversible" error.
- A migration runs inside the runner's transaction, so it never starts its own.
- Migrations don't use app code, which changes over time while a migration must not. The exception is `rebuild_table`, which is part of the migration system.
- Starter rows, like the first release formats, are inserted by migrations, and nothing may depend on them. Sample data goes in the seeds.

Writing one, once migrations are allowed again:
1. `npm run db:gen -- <name>` creates `m<timestamp>_<name>.rs` from a template, and adds it at the end of `migrations![…]`.
2. Write `up` as SQL statements: end every table with `strict`, and name indexes `<table>_<columns>_index`.
3. Write `down` so it undoes `up`.
4. Migrate, roll back and migrate a scratch database with the CLI, and run `cargo test`.
5. Update [reference/database.md](../reference/database.md).

### Changing an existing table

SQLite's `ALTER TABLE` only adds, renames and drops columns. Changing a column's type, CHECK, default or foreign key means rebuilding the table, with `rebuild_table()` from `repo/rebuild_table.rs`:

```rust
rebuild_table(
    tx,
    "artists",
    Rebuild {
        // The complete new definition, after `create table "new_artists"`
        define: r#"("id" integer not null primary key autoincrement, "name" text not null, "sort_name" text not null) strict"#,
        // New or changed columns, as expressions over the old table
        copy: &[("sort_name", "name")],
    },
)?;
```

It follows SQLite's recipe, as slunk's `rebuildTable()` did:
1. It creates the new definition under a temporary name, `new_<table>`.
2. It copies the rows. Columns both definitions share are copied as they are, `copy` fills new or changed ones, and any other new column gets its default.
3. It drops the old table and renames the new one. It turns `legacy_alter_table` on for the rename, because other tables' triggers name the dropped table and would make the rename fail.
4. It recreates the table's indexes and triggers from their old SQL.
5. It puts back the `AUTOINCREMENT` counter, which the copy lowers to the highest id left. Otherwise ids of deleted rows would be reused.

Using it:
- **Generated columns** aren't copied: `pragma_table_info` doesn't list them, and the new definition creates them again.
- **Indexes and triggers** come back exactly as they were. To change one, or if it uses a column that's gone, drop it before `rebuild_table()` and create the new version after.
- **Other tables** keep working. Their foreign keys and triggers name the table, so they apply to the new one.
- **Foreign keys:** it only runs with foreign keys off, as the runner sets them, and fails otherwise.
- **Don't change its behavior.** Committed migrations call it, and they must keep doing what they did. Add options instead.

### Parity with slunk

`tests/parity.rs` checks the Rust migrations against slunk's, with fixtures in `tests/fixtures/parity/` that slunk's own runner made:
- `fresh.schema.json` and `fresh.sql`: a new database slunk migrated. Rust's has the same `sqlite_schema`, row for row, and the same rows.
- `state_<k>.sql`: a database after slunk's first *k* migrations, each followed by its seed in `seeds/`, rows in every table it made. `final_<k>.sql` is that database after slunk's runner applied the rest, and Rust's runner, from the same state, ends with the same database.
- `rolled_back_<k>.sql`: the database after *k* migrations, after slunk's runner rolled the last one back. Rust's rollback ends with the same database.

The tests compare everything but `migrations.applied_at`. One more test checks that every `down` undoes its `up`. `add_track_tags`' down recreates three triggers with the baseline's logic, laid out differently, as slunk's did, so that test compares SQL without whitespace and parentheses.

`tests/fixtures/parity/generate.ts` makes the fixtures, with slunk's migrations in `legacy/slunk-be/` and the `sqlite3` command-line tool. They don't need to change while the migrations don't. To make them again, from the repository's root:

```sh
npm install --prefix legacy/slunk-be
node crates/slunk/tests/fixtures/parity/generate.ts
```

### The database commands

`crates/slunk-cli` has slunk's Ecto-style commands, which `package.json` wraps as `npm run db:*`. They take the database as `--db <path>`, and `migrate`, `rollback` and `reset` take `--keep-backups <n>` (default 10):

| Command | What it does |
|---|---|
| `npm run db:migrate -- --db <path>` | Backs up the database, then applies pending migrations |
| `npm run db:rollback -- --db <path> [n]` | Backs up the database, then reverts the last *n* migrations (default 1) |
| `npm run db:status -- --db <path>` | Lists migrations as applied, pending, or `MISSING` (applied, but unknown here) |
| `npm run db:gen -- <name>` | Creates `m<timestamp>_<name>.rs` and lists it |
| `npm run db:seed -- --db <path>` | Loads sample data, unless the database already has releases |
| `npm run db:reset -- --db <path>` | Backs up and deletes the database, then migrates and seeds a new one |

`seed` runs slunk's seeds through `legacy/slunk-be`, since they use the catalog, which isn't ported yet: five releases, with their artists, labels, formats, media, alternate names, and the tags Genre and Score. There are no files or tracks.

## The app

### Commands

The frontend calls the backend through Tauri commands, in `src-tauri/src/commands/`, one module per context:
- `library.rs`: `library_status`, `open_library`, `create_library`, `close_library`, `recent_libraries` and `reveal_library`;
- `media.rs`: `open_file` and `reveal_file`.

Their conventions:
- **Async.** Every command is `async`, and does its database and disk work on Tokio's blocking pool, with `blocking()`. A command that isn't `async` runs on the main thread, and freezes the window while it works.
- **Results.** Every command returns `Result<_, CommandError>`.
- **Arguments.** A command takes its arguments as one struct, `args`, like `open_file`'s `FileArgs`, so the frontend passes them as an object, as `useCommand()` expects: `call("openFile", { id })`. A command can also take a `Channel` to report progress, as `open_library` does with a path.
- **DTOs** derive `Serialize` (or `Deserialize`) and `specta::Type`, with `#[serde(rename_all = "camelCase")]`. Enums are tagged: `{ type: "created", … }`.
- **Ids** are `i64`, exported to TypeScript as `number` (`dangerously_cast_bigints_to_number()`): SQLite's ids stay far below 2^53.

**The bindings.** tauri-specta writes `src/bindings.ts` from the commands and their DTOs whenever a debug build starts (`lib.rs`). It's committed, so start a debug build after changing a command, and commit what it writes. It wraps each result as `{ status: "ok", data } | { status: "error", error }`, which the frontend's `useCommand()` unwraps.

**Errors.** `CommandError` is `{ kind, message }`, where `kind` is:
- `noLibrary`: no library is open, and the frontend goes to Welcome;
- `notFound`;
- `invalid`: a message for the user, which a form or Welcome shows as it is;
- `internal`.

SQLite's constraint errors are `invalid`, rewritten by the same table slunk's backend used: `UNIQUE constraint failed: tracks.file_id` becomes "That file is already a track's audio". The triggers' own messages pass through. Other errors are `internal`.

### Choosing the library

**The settings** are `settings.json` in the app's config folder, `~/Library/Application Support/com.liquidslunk.desktop/` on macOS. Debug builds use `settings.dev.json` there instead, so `npm run tauri dev` never opens the library the installed app uses.

```json
{
  "version": 1,
  "library": "/Users/…/Music/slunk.db",
  "recent": ["/Users/…/Music/slunk.db", "/Volumes/Archive/slunk.db"],
  "keepBackups": 10
}
```

- `library` is the database the next launch opens. `recent` is the last five opened, newest first.
- `keepBackups` is how many backups `slunk-backups/` keeps, a whole number from 1.
- Only Rust reads and writes the file. It's written atomically, a temporary file then a rename, and only once a library has opened, so a library that fails to open never becomes the saved one. A file that's missing or can't be read gives the defaults.

**Opening a library**, `Libraries::open()` in `library.rs`, on the blocking pool:
1. **Existing library:** the file is an SQLite database, and has the `migrations` table or no tables at all. Anything else is refused: "That isn't a Slunk library." **New library** (`create`): the folder exists and can be written. If it has a `slunk.db` already, the command says so, and the frontend offers to open it.
2. In debug builds, checks that the sidecar can start.
3. **Migrates**, with its backup. If anything is pending, `progress` hears `{ type: "migrating", pending }` first, so Welcome can say it's upgrading.
4. **Closes** the open library, if there is one, then **opens** the app's connection, and keeps the library in Tauri's state.
5. **Saves** the settings.
6. In debug builds, **starts the sidecar**.

Each failure is an `invalid` error, in words. An I/O or locking error adds that SQLite can't keep a library on a network drive. The result carries the migration's backup and applied migrations, for a toast.

**Switching** opens the new library: the old one stays open until the new one is checked and migrated. **Closing** (`close_library`, and the app's exit) stops the sidecar, then drops the connection, which checkpoints the WAL and removes the `-wal` and `-shm` files. It's in that order because SQLite only removes them when the app's connection is the last one to close. The settings still open the library at the next launch.

**One instance:** tauri-plugin-single-instance focuses the open window instead of starting a second app.

**`--library <path>`**, in debug builds only, opens a library at startup without touching the settings, for scripts: `npm run tauri dev -- -- -- --library /absolute/path/slunk.db`. The first `--` is npm's, the second ends the Tauri CLI's options, and the third the runner's..

### Serving files

`protocol.rs` serves `slunk://localhost/files/:id`, or `http://slunk.localhost/files/:id` on Windows: a registered file's bytes, by id, read-only, for `<img>` and `<audio>`. It answers on the blocking pool.
- An unknown id, no open library, or a file missing on disk gets a 404.
- The content type comes from the extension, by mime_guess, except `audio/mp4` for `m4a`, as slunk's backend sent it.
- Range requests get a 206 with `Content-Range`, so audio can seek. A response is a whole body in memory, so a range with no end (`bytes=0-`) gets 1 MB, and the media element asks for the rest as it plays. A request with no range gets the whole file.

The app's CSP allows `slunk:` and `http://slunk.localhost` in `img-src` and `media-src`. A page's `fetch()` can't read them: they're for media elements.

### Opening files

`open_file` opens a registered file in its default app, and `reveal_file` shows it in Finder, with the opener plugin, from its absolute path. `reveal_library` shows the open library's database in Finder. The webview has no opener permission: only these commands use it.

## The sidecar

Until phase 6, slunk's backend serves every route that isn't ported yet. When a library opens in a debug build, `sidecar.rs` starts it:
- `node src/server.ts`, in `legacy/slunk-be/`, on port 4000, with `SLUNK_DB` set to the library and `SLUNK_SKIP_MIGRATE=1`, since Rust has migrated it already. Its output goes to the terminal running the app.
- Opening waits until it listens. If its dependencies aren't installed, or something else listens on port 4000, opening fails before anything changes, and says so.
- Vite proxies `/graphql` to it, so Apollo works as it did in slunk.
- It stops when the library closes or the app exits, before the app's connection closes: it gets SIGTERM, which closes its database, and is killed if it hasn't stopped after 5 seconds.
- `sidecar-watchdog.cjs`, preloaded with `node --require`, stops it if the app is gone without stopping it, as when `tauri dev` restarts the app after a Rust change.

Both write the same database. WAL and `busy_timeout` handle it. Release builds never start the sidecar, so pages that use GraphQL can't load in them yet.

## Conventions

- **Formatting and linting:** `cargo fmt`, and `cargo clippy --workspace --all-targets` with no warnings.
- **Tests:** `cargo test --workspace`. Tests make their own temporary databases and never read `dev-library/`.
- **Comments** cite the base design as "design §5.6", and slunk's docs by name, like "slunk's design §7.4".
