# Backend

> Keep this page up to date: any change to the backend's stack, structure, commands or conventions updates it in the same change.

The backend is Rust, built into the app. It's being ported from slunk's Node backend one context at a time ([design §8](../design/1_introduction.md#8-the-transition)). So far:
- **Phase 0:** the database connection, the migration runner with its backups, slunk's eight migrations, and their parity tests; the database commands, in `crates/slunk-cli`; the app: commands typed by tauri-specta, their error, the settings, opening, creating, switching and closing libraries, the file protocol, and opening files; and the sidecar, slunk's backend, which still serves every route not ported yet, in debug builds.
- **Phase 1:** the media context: registered files, the library on disk, file links, and reading audio files; lists' paging; and the write parity script.
- **Phase 2:** the catalog context's artists and labels, with their alternate names, pictures and logos, and an artist's file links; release formats and release media; and language tags.

The rest of slunk's catalog context (releases, tracks, credits, the import's suggestions), its tagging context and its library check aren't ported yet, nor the media context's part of the library check (`checkFiles`, `relinkFiles`, `rehashFiles`). Their rules are in slunk's backend doc, `legacy/docs/development/backend.md`, and a rule moves here only once Rust does it, with a test.

## Stack

| Piece | Choice |
|---|---|
| Shell | Tauri 2 |
| Commands | Tauri commands, typed by tauri-specta |
| SQLite | rusqlite, with the `bundled` feature: SQLite is compiled in |
| Queries | Plain SQL, and sea-query for lists with dynamic filters and sorts |
| Errors | thiserror |
| Plugins | tauri-plugin-dialog (choosing a library), tauri-plugin-opener (Finder, opening files), tauri-plugin-single-instance |
| Content types | mime_guess |
| Audio | lofty |
| Hashing, walking folders | sha2, walkdir |
| Text | unicode-normalization (NFC) |
| Sorting file names | ICU4X's collator (`icu_collator`), with numeric ordering |
| Language tags | ICU4X's locale canonicalizer (`icu_locale`), and its Unicode properties (`icu_properties`) for scripts |
| CLI | clap |
| Screenshots | tauri-plugin-wdio-webdriver, behind the `webdriver` feature |

tauri-specta, specta and specta-typescript are release candidates, so their versions are pinned exactly in `src-tauri/Cargo.toml`. The Rust version is pinned in `mise.toml`.

rusqlite's bundled SQLite turns foreign keys on for every new connection, unlike better-sqlite3. Code that needs them off, like the migration runner, sets the pragma itself.

sea-query builds the SQL and its values, and `repo/helpers.rs` binds them for rusqlite (`bind()`). sea-query-rusqlite would do that, but it's built for an older rusqlite, and two bundled SQLites can't link into one binary.

## Structure

```
Cargo.toml                   the Cargo workspace
crates/slunk/                the domain and its database, with no Tauri dependency
  src/error.rs               the domain's error
  src/text.rs                text as it's saved: NFC, trimmed as JavaScript trims
  src/paging.rs              a list's page, sort and search
  src/languages.rs           language tags as they're saved, as Node's Intl gave them
  src/media/                 the media context
    mod.rs                   Media: registering, browsing, updating, deleting
    library.rs               the library on disk, read-only
    paths.rs                 Node's path functions, ported
    file_types.rs            a file's type, from its extension
    repository.rs            the records the media context stores
    audio/                   an audio file's duration and tags
  src/catalog/               the catalog context, so far: artists, labels, lists
    mod.rs                   Catalog: lists, artists, labels, pictures, deletes
    names.rs                 alternate names, and the links credited with them
    repository.rs            the records the catalog stores
  src/repo/
    connection.rs            opening SQLite with the connection settings
    store.rs                 the one connection, and transactions
    migrator.rs              the migration runner
    backups.rs               backups, and removing old ones
    rebuild_table.rs         rebuild_table(), for migrations that change a table
    files.rs                 registered files
    file_links.rs            files linked to releases, artists and tracks
    named.rs                 artists and labels: their rows and alternate names
    artists.rs               names credited on links, and what blocks a delete
    labels.rs                the releases a label is on
    lists.rs                 release formats and release media
    stats.rs                 the languages in use
    helpers.rs               chunked in (…) lists, grouping, sea-query's values
    migrations/              one module per migration, listed in mod.rs
  tests/parity.rs            the migrations, checked against slunk's
  tests/media.rs             the media context's rules
  tests/audio.rs             durations and tags, checked against slunk's
  tests/catalog.rs           the catalog context's rules
  tests/languages.rs         language tags, checked against Node's
  tests/fixtures/parity/     what slunk's runner made, and the script that made it
  tests/fixtures/audio/      audio files, what slunk read from them, and the
                             script that made both
  tests/fixtures/languages/  what Node's Intl gave for language tags, and the
                             script that asked it
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
  src/commands/              the commands and their DTOs, one module per context;
                             catalog/ has one per entity
  examples/run_commands.rs   runs commands without the app, for write parity
scripts/write-parity/        the write parity script, with each phase's steps
legacy/slunk-be/             slunk's backend, run as the sidecar
```

`crates/slunk` doesn't depend on Tauri, so its tests run with a plain `cargo test`, and the CLI uses it directly. Its repositories are plain functions over a `&Connection`, and a `Transaction` is one.

**Contexts** are structs, like slunk's context factories, wired once when a library opens (`Library::new()` in `src-tauri/src/library.rs`), as slunk's `server.ts` wired them: `Media::new(store, library)` takes the store and the library on disk, and `Catalog::new(store, media)` the store and the media context, which it registers files with. The library holds the media context in an `Arc`, which both share. Commands call contexts, and contexts call repositories.

**The store** (`repo/store.rs`) holds the library's one connection, behind a mutex. `store.conn()` takes it. `store.transaction(|tx| …)` runs a closure in a transaction, started with `BEGIN IMMEDIATE`, since slunk's backend writes the same database until phase 6. A context that writes inside another's transaction takes the `tx`, as `Media::insert_prepared(tx, prepared)` does, so there's no nesting. A context reads files before taking the connection, never while holding it: `prepare_files()` looks up what's registered, lets go of the connection, then reads and hashes the rest.

**Errors** meant for the user are `slunk::Error::Invalid`, with slunk's words ("There's no file at music/a.flac"), and `NotFound` for something that isn't there ("There's no file 12"). Commands turn them into `invalid` and `notFound`.

**Text** is saved in Unicode NFC, then trimmed as JavaScript's `trim()` trims (`text.rs`): that's Unicode's White_Space without U+0085, plus U+FEFF, which Rust's `trim()` doesn't trim. Searches are cleaned the same way. Paths aren't text: they're kept as they are on disk.

**Lists** (`paging.rs`) take a page from 1, 50 rows a page by default and 200 at most, a sort column with `-` in front for descending (`-id`, newest first, by default), and a search, cleaned like text. A sort the list doesn't allow is refused: "Files can't be sorted by size". `like_pattern()` escapes a search for `like … escape '\'`.

## The media context

`crates/slunk/src/media/` registers files and reads the library. It never writes to the user's files: `media/library.rs` only reads them. Each rule here has a test in `tests/media.rs`, or next to its code, and the write parity script checks them against slunk's backend.

- **Paths** are relative to the database's folder, and resolved: `""` is the folder itself, and paths above it start with `../`. They're worked out as Node's `path` works them out (`media/paths.rs`), lexically, without touching the disk, with forward slashes, and with `..` stopping at `/`. A path given absolute is made relative. They aren't text: they're kept as they are on disk, in whatever Unicode form the disk has.
- **Browsing** (`folder`) lists a folder's subfolders and the files directly inside, following symlinks: a symlinked folder is listed as a folder, a symlinked file as a file, and a broken symlink is left out. Each subfolder says whether it, or a folder inside it, holds registered files. That's a range on the index of `files.path`, and it works for the folders above the database's folder too: `..` holds everything but the paths that start with `../../`. The parent is `None` at the top of the disk.
- **Scanning** (`folderFiles`) lists every file in a folder and its subfolders, by path. It doesn't follow symlinked folders, so a link loop can't make it run forever, but it lists symlinked files.
- **Left out of both:** hidden files and folders (names starting with `.`, like `.DS_Store`), and liquidslunk's own files: the database, its `-wal`/`-shm`/`-journal` files and `slunk-backups/`. A folder that can't be read is an error: "Can't read the folder music/a".
- **Sorting:** folders and files sort as a file browser sorts them, "2 Song" before "10 Song". That's JavaScript's `localeCompare(…, { numeric: true })`, which slunk used, with ICU's root collation: ICU4X's collator with numeric ordering.
- **Types** come from the extension, ignoring case, in `media/file_types.rs`:
  - `music`: aac, aif, aiff, alac, ape, dff, dsf, flac, m4a, mp3, mpc, oga, ogg, opus, wav, wma, wv;
  - `image`: avif, bmp, gif, heic, jpeg, jpg, png, svg, tif, tiff, webp;
  - `other`: everything else, and a name that starts with its only dot, like `.flac`.
- **Registering** reads each file's size, mtime and SHA-256, streaming the file through the hash:
  - The hash is lowercase hex, which a CHECK enforces.
  - The mtime is whole seconds as slunk read them: Node's `mtimeMs` is `seconds * 1000 + nanoseconds / 1e6` in floating point, then `Math.floor(mtimeMs / 1000)`. So a time a few hundred nanoseconds before a second rounds up to it.
  - Paths are normalized, each registered once, and returned in the order given. Paths already registered are returned as they are. New files get their ids in that order.
  - A folder (`""`) is refused: "That's a folder, not a file". So is a path with no file: "There's no file at music/a.flac". Either way, nothing is registered.
  - The files are read before the connection is taken, then inserted in one transaction. `prepare_files()` and `insert_prepared(tx, …)` are the two halves, for a context that registers files inside its own transaction.
- **Updating** changes the type, or the path, to relink a file that moved. A new path is read again; the same path, once normalized, isn't. A path another file is registered at is refused: "File 12 is already registered at music/a.flac". An unknown id is `NotFound`: "There's no file 12". The database's triggers keep a track's audio `music`, and a cover or picture an `image`.
- **Deleting** removes only the record, and fails while anything uses the file: a track's audio, a release's cover, an artist's picture, a label's logo, or a file link. The error says what uses it: `The file is still used: the picture of "Alika"`. An unknown id deletes nothing, as in slunk.
- **Uses** (`file_uses`) list everything that uses a file, with the id and name of what uses it, and a file link's note. **Counts** (`file_counts`) are the files, and those nothing uses.
- **Listing** (`files`) pages, sorts by `id` or `path`, searches the path ignoring ASCII case (`like`, with `%` and `_` escaped), and filters by type, files nothing uses (`unused`), and files that aren't a track's audio (`withoutTrack`). Each row has the track it's the audio of.
- **File links** (`repo/file_links.rs`) connect images and other files to releases, artists and tracks, each with an optional note, in the order they were added. Saving an owner's links replaces them all. The catalog uses them from phase 2.
- **Many ids or paths** are read in chunks of 500 (`repo/helpers.rs`), well under SQLite's limit on bound parameters.

### Audio

`media/audio/` reads an audio file's duration and embedded tags, as suggestions for the import (phase 5): title, artists, album, album artists, track and disc numbers, the disc's title, and a date (`2006`, `2006-01` or `2006-01-10`). A file it can't read has no duration and no tags.

slunk read them with music-metadata. liquidslunk reads each format's own tags with lofty, with its implicit conversions off, so ID3v2.3's frames stay as they are, and `Collector` picks the values from them as music-metadata did:
- **Which keys:** Vorbis comments (FLAC, Ogg, Opus), ID3v2.2, 2.3 and 2.4, ID3v1, MP4 atoms, APEv2, RIFF INFO and AIFF's text chunks each have music-metadata's table for the fields above: `ALBUMARTIST` or `ALBUM ARTIST`, `TPE2` or `aART` for album artists; `DISCSUBTITLE`, `TSST` or `----:com.apple.iTunes:DISCSUBTITLE` for the disc's title; `DATE`, `TDRC` or `©day` for dates, and `TYER` or `YEAR` for a year.
- **Which tag wins:** when a file has several (ID3v2 and ID3v1 in an MP3, ID3v2 and RIFF INFO in a WAV), the higher priority wins a single value: APEv2, Vorbis, ID3v2.4, 2.3, 2.2, RIFF INFO, MP4, AIFF, ID3v1. The track and disc numbers are the exception: the last read wins, whatever its tag.
- **Lists:** a second artist tag adds to the artists, and `ARTISTS` tags replace them. ID3v2.4 separates values with a null; ID3v2.3's artists are split at `/` too, and its album artists aren't. Values are trimmed, and blank ones left out.
- **Numbers and dates:** a track number is the number before a `/`, read as JavaScript's `parseInt` reads it, and 0 is none. A date is its first `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, or else the year.
- **Durations** are in whole milliseconds, rounded as music-metadata rounded them. lofty works out FLAC's and AIFF's from the sample count but cuts them to the millisecond, so `audio/durations.rs` reads the sample count itself. An MP3 with an Info or Xing header has its frame count. One without: music-metadata read four frames, and at a constant bitrate estimated the duration from the file's size and the fourth frame's, or else counted every frame. It counted AAC's ADTS frames (`.aac`) with the same parser. `durations.rs` does it all as music-metadata did, down to leaving out the frames that start in a file's last 163 bytes.

Known differences, which no file in `dev-library/music/` has:
- lofty keeps one of two ID3v2 frames with the same ID, which the standard doesn't allow, and music-metadata read both.
- lofty doesn't read AAC in ADTS (`.aac`) at 8 kHz, which music-metadata did, so it has no duration and no tags. At 44.1 kHz it does.
- Ogg Vorbis, Opus, APE, WavPack and Musepack use the same tables, but their durations are lofty's, and none of it has been checked against music-metadata ([design Appendix D](../design/1_introduction.md#appendix-d-no-dsf-reader-and-audio-checks-on-the-formats-this-mac-makes)). DSF, DFF and WMA have no duration and no tags: lofty doesn't read them.

`tests/audio.rs` checks every fixture in `tests/fixtures/audio/` against `expected.json`, what slunk's own `readAudio()` read from it: FLAC, M4A (AAC and Apple Lossless), MP3 (ID3v2.3, ID3v2.4, ID3v1, with and without an Info header), AAC in ADTS, WAV and AIFF, a second or so of silence each, with tags written byte by byte, so they don't come from lofty. `tests/fixtures/audio/generate.ts` makes them, on macOS, with afconvert and slunk's backend:

```sh
npm install --prefix legacy/slunk-be
node crates/slunk/tests/fixtures/audio/generate.ts
```

## The catalog context

`crates/slunk/src/catalog/` holds what the user catalogs. So far it has artists and labels, and the two lists the user manages: release formats and release media. Each rule here has a test in `tests/catalog.rs`, or next to its code, and the write parity script checks them against slunk's backend.

- **Lists** share one repository (`repo/lists.rs`) and one set of functions, with a `ListKind`. Names are cleaned like other text, and unique in their list ignoring case, which the database enforces: "There's already one with that name, ignoring case". Renaming an unknown one is `NotFound`: "There's no format 12".
- **Listing** sorts by `name`, ignoring case, or by `id`, and searches the name ignoring ASCII case. Artists and labels then sort by id, and their search matches an alternate name too, so "福居" finds Ryo Fukui. Lists of each have their own sort error: "Artists can't be sorted by picture".
- **Alternate names** (slunk's 3_alternate_names.md §4.2) are other names an artist or label goes by: other spellings ("ALIKA"), initials ("R. Fukui"), names in other scripts ("福居良"). An artist's can be real names.
  - They're saved with their artist or label, as an ordered list: a name with an id keeps its row, one without is added, and names left out are deleted. Left out entirely, they stay as they are.
  - Names are cleaned like other text, and a blank one is left out. They compare exactly, case included, so "ALIKA" isn't "Alika".
  - A save is refused when a name is given twice (`"ALIKA" is listed twice`), when one is the main name (`"Alika" is already the artist's main name`), unless it's an artist's real name, and when an id isn't one of the owner's names (`Name 3 isn't one of its alternate names`). Labels have no real names.
  - A name given without an id, which the artist has already, keeps its row, and so the links credited with it.
  - Names can trade places between rows in one save: renamed rows first move clear of the unique (owner, name) index, to U+FFFF and their id, as tracks are renumbered.
  - A label's are replaced. Left out, they stay as they are, except one the label is renamed to, which goes.
- **An artist's names and its links** are saved together (`catalog/names.rs`), so every link keeps showing what it showed. A link is a release artist, a track artist or feature, or an artist on a credit line, credited with one of the artist's alternate names, or with none, which shows the main name.
  - Removing a name that links are credited with credits them with the main name, then deletes it.
  - Renaming an alternate name renames it on its links: that's how a misprint is fixed.
  - A rename shows on the links with no credited name. With `keepOldName`, they're credited with the old name instead, which is added after the others, with the old name's language, if the artist doesn't have it. That happens first, so the links of a name that goes in the same save don't get it. With no link showing the main name, nothing is kept.
  - With the list left out, renaming the artist to one of its alternate names promotes it: its links lose their credited name, and the name goes, unless it's a real name, which stays as one. Its links then show the main name, the same text.
  - So "Make main", which sends the old main name as a new row, drops the promoted name and sets `keepOldName`, swaps the two names, and every link shows what it did.
  - `artist_name_uses()` counts the links credited with each name, and `main_name_uses()` those that show the main name.
- **Pictures and logos** can be set on their own (`set_artist_picture`, `set_label_picture`), from a file id or a path. A path that isn't registered yet is registered in the same transaction, so a file the database refuses ("an artist picture must be an image") isn't left registered, and neither is one for an artist or label that isn't there. A ref with neither: "A file needs an id or a path".
- **An artist's file links** are saved with it, replacing the old ones, in their order, with paths registered in the same transaction. Notes are cleaned, and a blank one is none.
- **Saving** an artist or label checks its language first, then reads the files given by path, then writes everything in one transaction, so a failure saves nothing and registers nothing. An unknown id is `NotFound`: "There's no artist 12". A blank name is the database's CHECK: "The name can't be empty".
- **Deletes** check what blocks them first, and the error says what: "The artist is still used: release artist on 1 release, credited 2 times", "The label is on 1 release", "The format is used by 3 releases". `artist_blockers()`, `label_blockers()` and `list_item_blockers()` give the same reasons ahead of time, for the delete dialog. Deleting an artist deletes its alternate names and file links, and its files stay registered. An unknown id deletes nothing, as in slunk.
- **`languages()`** lists the language tags in use on names and titles, the most used first, then by tag, for the language picker.

### Languages

`languages.rs` saves the language of a name as slunk saved it: a BCP 47 tag in the canonical form Node 26's `Intl.getCanonicalLocales` gives, with ICU 78 and CLDR 48. "JA-latn" is `ja-Latn`, "iw" is `he` and "jpn" is `ja`. A blank one is `None`, which means unknown. Refused:
- a malformed tag: `"ja_JP" isn't a language tag, like en, ja or ja-Latn`;
- `und`: "Leave the language blank when it isn't known";
- a well-formed tag whose language isn't 2 or 3 letters: `"japanese" isn't a language tag: it starts with a 2- or 3-letter code, like en or jpn`.

ICU4X's canonicalizer does most of the work. `canonical_locale()` adds what V8 and ICU4C do that it doesn't:
- V8 returns a lowercase two-letter code as it is, without asking ICU, unless it's one of eight deprecated or legacy codes. So `bh` stays `bh`, where ICU makes it `bho`, but `BH` is `bho`.
- ICU4C has an alias ICU4X's data doesn't: `jaw` is `jv`.
- ICU's languages can have 5 to 8 letters, and ICU4X's can't. The rest of such a tag is canonicalized with a placeholder language, which has no aliases.
- A `posix` variant on its own is `-u-va-posix`.
- ICU replaces deprecated extension values: `yes` for any `-u-` key, which is `true` and so left out, and the values in `KEYWORD_ALIASES` and `FIELD_ALIASES`, like `-u-ca-islamicc` and `-u-tz-japan`. Those tables are what Node replaced among every value of 3 and 4 letters, every time zone of 5, and the longer legacy names in ICU's data.

`tests/languages.rs` checks it against `tests/fixtures/languages/expected.json`, what Node gave: every 2- and 3-letter code, and 548 tags around them, with scripts, regions, variants and extensions, and the ones it refuses. While porting, it agreed with Node on all of about 650,000 tags. Node's answer depends on its ICU, so the fixture records the versions. To make it again, from the repository's root:

```sh
node crates/slunk/tests/fixtures/languages/generate.ts
```

`script_language()` gives the language a name's script gives away, for a name added as it was credited: kana are `ja`, and Hangul `ko`. Han characters could be Chinese or Japanese, so they give none. Nothing calls it yet.

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
- `media.rs`: `files`, `file`, `file_counts`, `folder`, `folder_files`, `register_files`, `update_file` and `delete_file` ([The media context](#the-media-context)), and `open_file` and `reveal_file`;
- `catalog/` ([The catalog context](#the-catalog-context)), a module per entity:
  - `lists.rs`: `list_items`, `list_item`, `list_item_options`, `create_list_item`, `rename_list_item` and `delete_list_item`, each with a `kind`;
  - `artists.rs`: `artists`, `artist`, `artist_options`, `artist_names`, `create_artist`, `update_artist`, `set_artist_picture` and `delete_artist`;
  - `labels.rs`: `labels`, `label`, `label_options`, `create_label`, `update_label`, `set_label_picture` and `delete_label`;
  - `mod.rs`: `languages`, and what they share.

Their conventions:
- **Async.** Every command is `async`, and does its database and disk work on Tokio's blocking pool, with `blocking()`, or `on_library()` for one that needs the open library, which is `noLibrary` without one. A command that isn't `async` runs on the main thread, and freezes the window while it works.
- **Thin.** A command that uses the library is a wrapper over a plain function that takes it, like `media::register(library, args)` or `catalog::artists::create(library, args)`, which maps the context's records to DTOs. The write parity script calls those functions too.
- **Results.** Every command returns `Result<_, CommandError>`.
- **Arguments.** A command takes its arguments as one struct, `args`, like `open_file`'s `FileArgs`, so the frontend passes them as an object, as `useCommand()` expects: `call("openFile", { id })`. A command can also take a `Channel` to report progress, as `open_library` does with a path.
- **DTOs** derive `Serialize` (or `Deserialize`) and `specta::Type`, with `#[serde(rename_all = "camelCase")]`. Enums are tagged: `{ type: "created", … }`. A DTO has the fields its GraphQL document asked for, with the same names, so components change little: a file's `type`, `sizeBytes` and `uses[].entityId`. A shared one is flattened in with `#[serde(flatten)]`, as `FileDetails` holds a `File`, and an update's arguments hold its input: `UpdateArtistArgs` is `{ id, ...ArtistInput }`.
- **Pickers** have their own small commands, like `artist_options`, which return the first 20 matches by name (slunk's design §7.3).
- **Optional arguments** are `Option`s with `#[serde(default)]` and `#[specta(optional)]`, so the frontend leaves them out: `useCommand("files", { sort: "path" })`.
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

## Write parity

`npm run parity:writes` checks that commands write what slunk's GraphQL wrote (design §10). It runs the same steps on two copies of a seeded library, one through slunk's backend and one through the commands, then compares what each step returned, its error included, and the two databases, table by table, leaving out `inserted_at`, `updated_at` and `applied_at`:
1. It makes, in a temporary folder, two libraries with the same files: audio fixtures, text files and images, a hidden folder, symlinks, a name in NFD, and a folder next to the library for `../` paths. Their mtimes are a few nanoseconds before a second.
2. It migrates one with the CLI, seeds it, and prepares it through GraphQL with each phase's `prepare` steps, then copies the database to the other.
3. It runs the steps through slunk's backend, on a free port, and through `src-tauri/examples/run_commands.rs`, which opens the other library without the app and calls the commands' functions with the same JSON arguments.
4. It compares results with ids as numbers, since GraphQL's are strings, and each library's folder replaced. Where a command returns something else than its GraphQL query, the step's `fromGraphql` maps one to the other: a picker's command returns a list, and its query a page.

Each phase adds its steps in a module of `scripts/write-parity/` (`media.ts` is phase 1's, `catalog.ts` phase 2's), lists it in `PHASES`, and adds its commands to `run_commands.rs`. The steps run in order, on the same library, so a phase's steps start where the earlier phases' left it. A step that fails says so, with its message, even when both sides fail alike. `KEEP=1` keeps the temporary folder when they differ. It needs `npm install --prefix legacy/slunk-be`, and runs until phase 6, which deletes slunk's backend.

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
- **Tests:** `cargo test --workspace`. Tests make their own temporary databases and files, and never read `dev-library/`. Audio and language tag tests use the committed fixtures in `crates/slunk/tests/fixtures/`.
- **Comments** cite the base design as "design §5.6", and slunk's docs by name, like "slunk's design §7.4".
