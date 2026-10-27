# Backend

> Keep this page up to date: any change to the backend's stack, structure, commands or conventions updates it in the same change.

The backend is Rust, built into the app. It was ported from slunk's Node backend one context at a time: first the database, its migrations and the database commands, and the app around them, then the media context, the catalog, the tagging context and the library check. While the port ran, slunk's backend served the routes not ported yet, beside the app in debug builds, and a script checked that the commands wrote what slunk's backend wrote, step by step. Once every page ran on Rust commands, slunk's backend was removed, with what ran it.

Since the port, the backend has added the music player's commands, `player_tracks` and `release_track_ids` ([Commands](#commands)), and the menu bar's Controls menu ([The menu bar](#the-menu-bar)); the external libraries context, with liquidslunk's first migration of its own ([The external libraries context](#the-external-libraries-context)); and artist aliases, with a migration of their own ([The catalog context](#the-catalog-context)).

Each rule on this page has a test.

## Stack

| Piece | Choice |
|---|---|
| Shell | Tauri 2 |
| Commands | Tauri commands and events, typed by tauri-specta |
| SQLite | rusqlite, with the `bundled` feature: SQLite is compiled in |
| Queries | Plain SQL, and sea-query for lists with dynamic filters and sorts |
| Errors | thiserror |
| Menu bar | Tauri's menus |
| Plugins | tauri-plugin-dialog (choosing a library), tauri-plugin-opener (Finder, opening files), tauri-plugin-single-instance |
| Content types | mime_guess |
| Audio | lofty; converting with macOS's `afconvert` |
| Images | `image` (covers: JPEG, PNG, GIF, BMP, TIFF, WebP) |
| JSON in the domain | serde_json (an external library's manifest) |
| Volumes | libc's `statvfs` and `statfs` (free space, mount points), and `diskutil eject` |
| iPod | liquidslunk's own reader and writer of its databases ([reference/ipod.md](../reference/ipod.md)) |
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
  src/dates.rs               dates as they're typed, stored and shown
  src/library_check.rs       the library check, over the media context and
                             the catalog
  src/external/              the external libraries context
    mod.rs                   External: external libraries, their rules, what
                             they come to, memberships; a track's tags, and
                             its path; plans, syncs, eject and Locate
    repository.rs            the records the context stores
    tags.rs                  a track's tags, from the catalog
    template.rs              path templates, and names on disk
    estimate.rs              what a track's file will take on a destination
    convert.rs               what each source becomes, and afconvert
    covers.rs                covers, checked, resized, as JPEG
    write_tags.rs            tags into files, with lofty
    manifest.rs              the manifest, and its journal
    plan.rs                  the plan, from what should be there and what is
    sync.rs                  running a plan
    folder.rs                a destination's files and folders: a folder's,
                             or an iPod's
    scratch.rs               a sync's temporary folder
    volume.rs                free space, mount points, and ejecting
    ipod/                    the iPod: finding it, and its databases
      mod.rs                 mounted iPods, paths, new file names, Mac times
      sysinfo.rs             which iPod it is, from SysInfo
      itunesdb.rs            the iTunesDB, read and written
      artwork.rs             the ArtworkDB and its thumbnails
      audio.rs               what the database needs of a file: gapless
                             data, Sound Check
      play_counts.rs         Play Counts
      bytes.rs               little-endian chunks
      sync.rs                running a plan on an iPod
  src/media/                 the media context
    mod.rs                   Media: registering, browsing, updating, deleting
    check.rs                 the library check's file part, and its fixes
    library.rs               the library on disk, read-only
    paths.rs                 Node's path functions, ported
    file_types.rs            a file's type, from its extension
    repository.rs            the records the media context stores
    audio/                   an audio file's duration and tags
  src/catalog/               the catalog context: artists, labels, lists,
                             releases, tracks
    mod.rs                   Catalog: lists, artists, labels, pictures, deletes
    names.rs                 alternate names, and the links credited with them
    aliases.rs               an artist's aliases, saved as one group with it
    releases.rs              releases: details, links, cover, files, credits
    tracks.rs                tracks, and a release's track list and disc titles
    suggestions.rs           what the import makes of a folder's tags and names
    credited.rs              artists as credited, and credit lines as saved
    titles.rs                alternate titles as they're saved
    repository.rs            the records the catalog stores
  src/tagging/               the tagging context
    mod.rs                   Tagging: definitions, values, filters, bulk edits
    values.rs                values as they're typed and shown
    repository.rs            the records the tagging context stores
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
    aliases.rs               artist aliases: groups, inserted and deleted
    labels.rs                the releases a label is on
    lists.rs                 release formats and release media
    releases.rs              releases, and their artists, labels, formats, media
    titles.rs                alternate titles of releases, tracks and discs
    tracks.rs                tracks, and who performs them
    credits.rs               credit lines, and the roles in use
    stats.rs                 the languages in use, and the catalog's totals
    tags.rs                  tag definitions, their values, and list filters
    external_libraries.rs    external libraries and their rules, and the
                             tracks the rules come to
    helpers.rs               chunked in (…) lists, grouping, sea-query's values
    seeds.rs                 the sample data (npm run db:seed)
    migrations/              one module per migration, listed in mod.rs
  tests/parity.rs            the migrations, checked against slunk's
  tests/media.rs             the media context's rules
  tests/audio.rs             durations and tags, checked against slunk's
  tests/catalog.rs           the catalog context's rules: artists, their
                             aliases, labels, lists
  tests/releases.rs          the catalog context's rules for releases
  tests/tracks.rs            the rules for tracks and the import's suggestions
  tests/languages.rs         language tags, checked against Node's
  tests/tagging.rs           the tagging context's rules
  tests/library_check.rs     the library check's rules
  tests/external.rs          the external libraries context's mapping: tags,
                             templates, names on disk
  tests/external_libraries.rs  external libraries, their rules, what they come
                             to, memberships, and their tables' constraints
  tests/external_sync.rs     syncing a folder library and an iPod from start
                             to end, with afconvert (macOS only)
  tests/seeds.rs             the sample data
  tests/fixtures/parity/     what slunk's migration runner made
  tests/fixtures/audio/      audio files, what slunk read from them, and the
                             script that makes the files
  tests/fixtures/languages/  what Node's Intl gave for language tags, and the
                             script that asked it
crates/slunk-cli/            the database commands (npm run db:*)
src-tauri/
  tauri.conf.json            the app's name, identifier, window, CSP and bundle
  capabilities/default.json  what the window may call
  icons/                     the app's icons, and icon.svg, their source
  src/lib.rs                 the builder: plugins, state, protocol, commands,
                             menu bar
  src/error.rs               the commands' error, and SQLite's errors in words
  src/settings.rs            the settings file
  src/library.rs             opening, creating, switching and closing libraries
  src/protocol.rs            slunk://localhost/files/:id
  src/menu.rs                the menu bar, with the Controls menu
  src/commands/              the commands and their DTOs, one module per context;
                             catalog/ has one per entity
```

`crates/slunk` doesn't depend on Tauri, so its tests run with a plain `cargo test`, and the CLI uses it directly. Its repositories are plain functions over a `&Connection`, and a `Transaction` is one.

**Contexts** are structs, like slunk's context factories, wired once when a library opens (`Library::new()` in `src-tauri/src/library.rs`), as slunk's `server.ts` wired them: `Media::new(store, library)` takes the store and the library on disk, `Catalog::new(store, media)` the store and the media context, which it registers files with, `Tagging::new(store)` the store, `LibraryCheck::new(media, catalog)` the media context and the catalog, and `External::new(store, media)` the store, which it reads the catalog through, and the media context, for the library's folder. The library holds the media context and the catalog in `Arc`s, which they share. Commands call contexts, and contexts call repositories.

**The store** (`repo/store.rs`) holds the library's one connection, behind a mutex. `store.conn()` takes it. `store.transaction(|tx| …)` runs a closure in a transaction, started with `BEGIN IMMEDIATE`, which takes the write lock at once: another process writing the library, like the database commands, makes it wait for the lock (`busy_timeout`) rather than fail midway. `store.read(|tx| …)` runs one in a read transaction, a plain `BEGIN`, so what it reads is one consistent picture of the library, as a sync's plan reads it. A context that writes inside another's transaction takes the `tx`, as `Media::insert_prepared(tx, prepared)` does, so there's no nesting. A context reads files before taking the connection, never while holding it: `prepare_files()` looks up what's registered, lets go of the connection, then reads and hashes the rest.

**Errors** meant for the user are `slunk::Error::Invalid`, with slunk's words ("There's no file at music/a.flac"), and `NotFound` for something that isn't there ("There's no file 12"). Commands turn them into `invalid` and `notFound`.

**Text** is saved in Unicode NFC, then trimmed as JavaScript's `trim()` trims (`text.rs`): that's Unicode's White_Space without U+0085, plus U+FEFF, which Rust's `trim()` doesn't trim. Searches are cleaned the same way. Paths aren't text: they're kept as they are on disk.

**Lists** (`paging.rs`) take a page from 1, 50 rows a page by default and 200 at most, a sort column with `-` in front for descending (`-id`, newest first, by default), and a search, cleaned like text. A sort the list doesn't allow is refused: "Files can't be sorted by size". `like_pattern()` escapes a search for `like … escape '\'`.

## The media context

`crates/slunk/src/media/` registers files and reads the library. It never writes to the user's files: `media/library.rs` only reads them. Each rule here has a test in `tests/media.rs`, or next to its code. While the port ran, a script also checked them against slunk's backend.

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
- **File links** (`repo/file_links.rs`) connect images and other files to releases, artists and tracks, each with an optional note, in the order they were added. Saving an owner's links replaces them all.
- **Many ids or paths** are read in chunks of 500 (`repo/helpers.rs`), well under SQLite's limit on bound parameters.

### Audio

`media/audio/` reads an audio file's duration and embedded tags, as suggestions for the import ([Import suggestions](#import-suggestions)): title, artists, album, album artists, track and disc numbers, the disc's title, and a date (`2006`, `2006-01` or `2006-01-10`). A file it can't read has no duration and no tags.

slunk read them with music-metadata. liquidslunk reads each format's own tags with lofty, with its implicit conversions off, so ID3v2.3's frames stay as they are, and `Collector` picks the values from them as music-metadata did:
- **Which keys:** Vorbis comments (FLAC, Ogg, Opus), ID3v2.2, 2.3 and 2.4, ID3v1, MP4 atoms, APEv2, RIFF INFO and AIFF's text chunks each have music-metadata's table for the fields above: `ALBUMARTIST` or `ALBUM ARTIST`, `TPE2` or `aART` for album artists; `DISCSUBTITLE`, `TSST` or `----:com.apple.iTunes:DISCSUBTITLE` for the disc's title; `DATE`, `TDRC` or `©day` for dates, and `TYER` or `YEAR` for a year.
- **Which tag wins:** when a file has several (ID3v2 and ID3v1 in an MP3, ID3v2 and RIFF INFO in a WAV), the higher priority wins a single value: APEv2, Vorbis, ID3v2.4, 2.3, 2.2, RIFF INFO, MP4, AIFF, ID3v1. The track and disc numbers are the exception: the last read wins, whatever its tag.
- **Lists:** a second artist tag adds to the artists, and `ARTISTS` tags replace them. ID3v2.4 separates values with a null; ID3v2.3's artists are split at `/` too, and its album artists aren't. Values are trimmed, and blank ones left out.
- **Numbers and dates:** a track number is the number before a `/`, read as JavaScript's `parseInt` reads it, and 0 is none. A date is its first `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, or else the year.
- **Durations** are in whole milliseconds, rounded as music-metadata rounded them. lofty works out FLAC's and AIFF's from the sample count but cuts them to the millisecond, so `audio/durations.rs` reads the sample count itself. An MP3 with an Info or Xing header has its frame count. One without: music-metadata read four frames, and at a constant bitrate estimated the duration from the file's size and the fourth frame's, or else counted every frame. It counted AAC's ADTS frames (`.aac`) with the same parser. `durations.rs` does it all as music-metadata did, down to leaving out the frames that start in a file's last 163 bytes.

Known differences, which no file in `dev-library/music/` has:
- lofty keeps one of two ID3v2 frames with the same ID, which the standard doesn't allow, and music-metadata read both.
- lofty doesn't read AAC in ADTS (`.aac`) at 8 kHz, which music-metadata did, so it has no duration and no tags. At 44.1 kHz it does.
- Ogg Vorbis, Opus, APE, WavPack and Musepack use the same tables, but their durations are lofty's, and none of it has been checked against music-metadata: this Mac can't make files in those formats without ffmpeg, so there are no fixtures for them. DSF, DFF and WMA have no duration and no tags: lofty doesn't read them.

`tests/audio.rs` checks every fixture in `tests/fixtures/audio/` against `expected.json`, what slunk's reader, with music-metadata, read from it: FLAC, M4A (AAC and Apple Lossless), MP3 (ID3v2.3, ID3v2.4, ID3v1, with and without an Info header), AAC in ADTS, WAV and AIFF, a second or so of silence each, with tags written byte by byte, so they don't come from lofty. `tests/fixtures/audio/generate.ts` makes the files, on macOS, with afconvert:

```sh
node crates/slunk/tests/fixtures/audio/generate.ts
```

`expected.json` was written while slunk's backend was in the repository, and isn't written again: the files hold the same audio and tags each time they're made. A new fixture needs its expected values added by hand.

## The catalog context

`crates/slunk/src/catalog/` holds what the user catalogs: artists and labels, the two lists the user manages, release formats and release media, releases ([Releases](#releases)) and tracks ([Tracks](#tracks)), and what a folder suggests for importing it ([Import suggestions](#import-suggestions)). Each rule here has a test in `tests/catalog.rs`, `tests/releases.rs` or `tests/tracks.rs`, or next to its code. While the port ran, a script also checked them against slunk's backend.

- **Lists** share one repository (`repo/lists.rs`) and one set of functions, with a `ListKind`. Names are cleaned like other text, and unique in their list ignoring case, which the database enforces: "There's already one with that name, ignoring case". Renaming an unknown one is `NotFound`: "There's no format 12".
- **Listing** sorts by `name`, ignoring case, or by `id`, and searches the name ignoring ASCII case. Artists and labels then sort by id, and their search matches an alternate name too, so "福居" finds Ryo Fukui. Lists of each have their own sort error: "Artists can't be sorted by picture".
- **Alternate names** are other names an artist or label goes by: other spellings ("ALIKA"), initials ("R. Fukui"), names in other scripts ("福居良"). An artist's can be real names.
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
- **One more name** (`add_artist_name()`) is added after an artist's others, as the import adds a tag's other names: cleaned, in its language, canonical, and never a real name. It's refused when it's blank ("The name is blank"), when it's the main name (`"Ryo Fukui" is already the artist's main name`), and when the artist has it already (`Ryo Fukui already has the name "福居良"`).
- **Aliases** are other artists that are the same person or band under another name: deadmau5 and testpilot. Each keeps its own page, releases, credits, picture, files and external library rules. They're kept as groups ([reference/database.md](../reference/database.md#artist-aliases)), and every artist in a group is an alias of every other one. An alias means the same person, so a name two people each released under can't be an alias of both: it would make them one group.
  - They're saved with their artist (`catalog/aliases.rs`), as a list of artist ids, which become one group with it, under a new number: the rows of the artist and of the artists given are deleted, then inserted again. Left out, the artist's group stays as it is, and an empty list takes it out of it.
  - Everyone else keeps their group: the artist's old aliases that are left out stay aliases of each other, and so do the other aliases of an artist given without them. So the form sends a picked artist's aliases with it, which joins their groups.
  - A group left with one artist loses it, whether a save or a delete left it: the database's trigger does that.
  - An id given twice is kept once. The artist itself is refused ("An artist can't be its own alias"), and an unknown one is `NotFound` ("There's no artist 12").
  - `artist_aliases()` lists an artist's aliases by name, ignoring case, then by id. Deleting an artist takes it out of its group, and aliases never block a delete.
  - Nothing else follows aliases: credits, the import, search, the release list's artist filter and external library rules each name one artist.
- **Pictures and logos** can be set on their own (`set_artist_picture`, `set_label_picture`), from a file id or a path. A path that isn't registered yet is registered in the same transaction, so a file the database refuses ("an artist picture must be an image") isn't left registered, and neither is one for an artist or label that isn't there. A ref with neither: "A file needs an id or a path".
- **An artist's file links** are saved with it, replacing the old ones, in their order, with paths registered in the same transaction. Notes are cleaned, and a blank one is none.
- **Saving** an artist or label checks its language first, then reads the files given by path, then writes everything in one transaction, so a failure saves nothing and registers nothing. An unknown id is `NotFound`: "There's no artist 12". A blank name is the database's CHECK: "The name can't be empty".
- **Deletes** check what blocks them first, and the error says what: "The artist is still used: release artist on 1 release, credited 2 times", "The label is on 1 release", "The format is used by 3 releases". `artist_blockers()`, `label_blockers()` and `list_item_blockers()` give the same reasons ahead of time, for the delete dialog. Deleting an artist deletes its alternate names and file links, and its files stay registered. An unknown id deletes nothing, as in slunk.
- **`languages()`** lists the language tags in use on names and titles, the most used first, then by tag, for the language picker.

### Releases

`catalog/releases.rs` saves a release's details and what hangs off it, and reads what the release pages show. Its tracks are saved by `catalog/tracks.rs` ([Tracks](#tracks)).
- **Details** are saved together: the title, cleaned; its language, canonical ([Languages](#languages)), which an update leaves as it is when it's left out; the date as typed, `2006`, `2006-01` or `2006-01-10`, stored with its precision ([Dates](#dates)), and a blank one is none; the catalog number, cleaned, and a blank one is none; and whether it's various artists. They're checked before anything is written: the language first, then the date. An unknown id is `NotFound`: "There's no release 12".
- **Artists** are saved as credited, each once, the first time it's given (`catalog/credited.rs`): blank, or the artist's main name, is the main name; one of the artist's alternate names is that name; anything else is added to its alternate names, in the language its script gives away ([Languages](#languages)), and remembered, so two links in one save share it. Names compare exactly, case included. An unknown artist is `NotFound`: "There's no artist 12".
- **Labels** keep the order they're given in, each once. **Formats and media** each once too, and they're listed by their own id, not in the order given: slunk's query ordered them by `id`, which SQLite read as its alias for the format's or medium's id.
- **Alternate titles** are saved as an ordered list, replacing the old ones (`catalog/titles.rs`). They're cleaned, a blank one is left out, and each is printed unless it says otherwise. A title given twice is refused (`"A" is listed twice on the release`), and so is the main title (`"Scenery" is already the main title of the release`). Left out, they stay as they are, except one that's now the main title, which goes.
- **The list** (`list_releases()`) sorts by `title`, ignoring case, by `released`, with undated releases last either way, or by `id`, then by id. Its search matches part of the title or of an alternate title, and its filters all have to match: a format, a medium, a label, a release artist, and tag filters ([The tagging context](#the-tagging-context)). `release_ids()` gives every match's id, newest first, to select them all.
- **The cover and file links** are saved together (`set_release_files()`), or the cover alone (`set_release_cover()`), from a file id or a path. Paths are registered in the same transaction, so a file the database refuses ("a release cover must be an image") isn't left registered, and neither is one for a release that isn't there.
- **Credit lines** are saved with the release's tracks' (`set_release_credits()`), each replacing its owner's. A role is cleaned, and an artist given twice on a line is kept once, with the first name it's credited with. A save is refused for a blank role ("A credit needs a role"), a line with no artist ("The Piano credit needs an artist"), two roles that differ only in case ("Two credits have the role PIANO, ignoring case. Put their artists on one line."), and another release's track ("Track 12 isn't on this release"). `credit_roles()` suggests the roles in use, the first 20 with what's typed in them, the most used first, each spelling its own.
- **Deleting** a release fails while it has tracks: "The release still has 2 tracks. Delete them first." `release_blockers()` says it ahead of time: "Has 2 tracks". Its links, titles, credits, tags and file links go with it, and its files stay registered. An unknown id deletes nothing, as in slunk.
- **What the pages read:** a release's tracks, by disc then number, with their alternate titles, artists, features and credit lines; its disc titles; an artist's appearances, the releases where it's on tracks without being a release artist, its credit lines, and how many of its releases have each format; and `totals()`, the dashboard's numbers.

### Tracks

`catalog/tracks.rs` saves tracks, one at a time with everything that's saved with them, or a release's whole list at once, with its disc titles.
- **A release's tracks** are saved as a whole list (`set_release_tracks()`), in one transaction:
  - rows with an id update those tracks: their disc, number, title, and the title's language and alternate titles when they're given;
  - rows with a file id or a path create tracks, registering paths in the same transaction, with the durations read before it starts;
  - tracks left out are deleted, and their files stay registered.

  A row with another release's track is refused ("Track 12 isn't on this release"), and so is one with no file ("A file needs an id or a path"). Before saving, the release's track numbers move a million up, so swapping two numbers can't trip the unique (release, disc, number) index midway.
- **Disc titles** are saved after the tracks, since a title needs a track on its disc ([reference/database.md](../reference/database.md#disc-titles)). Given, they replace the release's, with their languages and alternate titles; left out, they stay as they are. Titles are cleaned, and a blank one is left out, since a disc without a title has no row. Two titles for one disc are refused ("Disc 1 has two titles"), and so are alternate titles on a disc with no title ("Disc 2 has alternate titles but no title"). A disc left without tracks loses its title, whichever save moves or deletes them: the database does that.
- **A track** is created (`create_track()`) from a registered music file, whose duration is read first: "There's no file 12", "music/a.txt isn't a music file". Saving one (`update_track()`) reads the duration again only when its file changes. Its title's language, left out, stays as it is. Its alternate titles, artists, credit lines and file links are saved with it, replacing the old ones, with paths registered in the same transaction, and its tags too when they're given ([The tagging context](#the-tagging-context)); left out, they stay as they are. An unknown id is `NotFound`: "There's no track 12". Deleting one (`delete_track()`) deletes its artists, credit lines, alternate titles, file links and tags, and its file stays registered.
- **A track's titles** are cleaned, and its alternate titles saved as a release's are ([Releases](#releases)), naming the track by its number in their errors: `"初夏" is already the main title of track 4`.
- **Artists** are a track's own performers, saved as credited, as a release's are, then its features, each once. An artist can't be both: "An artist can't both perform a track and be featured on it". A name added for one link is shared by the others in the same save, whole list included.
- **Who performs a track** (`performers()`): its own artists, without features; on a regular release with none, the release's artists, as credited there; on a various-artists release, only its own, or nobody.
- **The track list** (`list_tracks()`) sorts by `title`, ignoring case, or by `id`, then by id. Its search matches part of the title or of an alternate title, and its filters all have to match: a release, and tag filters.

### Import suggestions

`catalog/suggestions.rs` reads a folder for the import, without writing anything:
- **A release** (`suggest_release()`): the most common album, date and album artist tags of the folder's first 50 audio files, or their artist tags when they have no album artists. A tie goes to the value read first. Without an album tag, the title is the folder's name, and a date that isn't one, like `2013-02-30`, is left out. It also counts the folder's audio files, images and other files, and finds the releases with that title already, main or alternate, ignoring case, so a folder isn't imported twice.
- **Titles with " = "** are kept whole, and the import offers to split them. The check for releases with that title also tries each part, so "Scenery = シーナリィ" finds "Scenery". `equals_parts()` splits only at an "=" with white space on both sides, as JavaScript's `\s` matched it, since "E=MC²" is a title. Parts are trimmed, and empty and repeated ones left out.
- **A folder's tracks** (`suggest_tracks()`): a row for each audio file in the folder and its subfolders, in the folder's order, with its disc, number and title from its tags, or else from its path, relative to the folder: `CD2/03 Title.flac` is disc 2, track 3, "Title". A folder named `cd`, `disc` or `disk` and a number, in any case of ASCII letters, gives the disc. A name that starts with one to three digits, then white space, `.`, `_` or `-`, gives the number, and the rest the title. A file with no number in either gets its place on its disc. Each row also has its disc's title tag, its duration, its registration if the file is registered, and the track it's already the audio of, on any release.
- **Artist tags** are matched all at once, a folder's together, in three steps: the main name exactly; then an alternate name exactly, credited with it; then either one ignoring ASCII case, credited as tagged, so "alika", with only "Alika" and "ALIKA" known, is printed as "alika". The first step that finds anyone decides. Finding more than one artist picks none, and `matches` says how many there were.
- **A tag with " = "** ("Ryo Fukui = 福居良") is matched by its first part. Its other parts that the artist doesn't have yet are `other_names`, for the import to offer as alternate names (`add_artist_name()`).
- **These match slunk's regular expressions** exactly, written by hand: JavaScript's `\s` is `trim()`'s white space, its `\d` ASCII digits, and its `i` flag, without `u`, ASCII letters only, so the Kelvin sign isn't a "k".

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

`script_language()` gives the language a name's script gives away, for a name added as it was credited ([Releases](#releases)): kana are `ja`, and Hangul `ko`. Han characters could be Chinese or Japanese, so they give none.

## The tagging context

`crates/slunk/src/tagging/` holds tag definitions and their values on releases and tracks. Each type, `text`, `number` or `date`, has its own definition table, so the same id exists once per type, and every function takes the type with the id. Each type also has a value table per owner, release or track (`release_text_tags`, `track_text_tags`, …). Releases and tracks share the definitions, so one Genre tag works on both. `repo/tags.rs` serves all of them. Each rule here has a test in `tests/tagging.rs`, or next to its code. While the port ran, a script also checked them against slunk's backend.

- **Listing** shows one type's tags, or every type's together. It sorts by `name`, ignoring case, or by `id`, and ties go text, number, date. The search matches part of the name. Another sort is refused: "Tags can't be sorted by multiple".
- **Names** are cleaned like other text, and unique across types, ignoring case. Each type's table enforces its own names ("There's already one with that name, ignoring case"), and the context checks the other two types when a tag is created or renamed: "There's already a number tag named Score". A blank name is the database's CHECK.
- **Single or multiple.** Switching a tag to single-value fails while a release or track has several values, and says how many do: "1 release and 1 track have several values for this tag. Leave one value on each before making it single-value." `multi_value_owners()` lists them. The database refuses it too. An unknown id is `NotFound`: "There's no number tag 12".
- **Deleting** a tag fails while any release or track has a value: "The tag is set on 2 releases and 1 track". `tag_blockers()` says it ahead of time: "Set on 2 releases and 1 track". An unknown id deletes nothing, as in slunk.
- **Values** are typed as text and stored by type. One that can't be read names its tag: "Score: Not a number: abc".
  - Text is cleaned (NFC, trimmed).
  - A number is like `8`, `-3.25`, `.5` or `1e3`, as slunk's pattern and JavaScript's `Number()` read it, and must be finite.
  - A date is `2006`, `2006-01` or `2006-01-10`, stored at midnight UTC on the first unknown day ([Dates](#dates)). There's no precision, so it comes back as a full day: `2006-01-01`.

  They're shown as text (`format_value()`): a date as `2006-01-10`, and a number as JavaScript's `String()` writes it, with the fewest digits that read back as the same number: `8.5`, `1000`, `1e+21`, `1e-7`. Of two candidates as close, JavaScript takes the even one, which Rust's shortest form doesn't always do, so `format_number()` rounds the exact value to that many digits instead. It agreed with Node on 700,000 numbers while porting.
- **A release's or track's values** are saved all at once (`set_tags()`, or `Tagging::set_tags_with()` inside another context's transaction), replacing the old ones:
  - blank values are left out, so a tag with none is removed;
  - a value given twice, ignoring case for text, is kept once, as first spelled;
  - values keep the order they were given in, and a tag given twice gets one list;
  - a single-value tag with two values is refused ("Score takes one value"), after every value has been read;
  - an unknown tag is `NotFound` ("There's no text tag 12"), and `set_tags()` says so of a release or track that isn't there too ("There's no release 12").

  They're read back grouped by tag, by tag name ignoring case, then text, number, date (`tags_of()`).
- **Text values** are counted for a tag's page (`tag_values()`): each value with how many releases and tracks have it, the most used first, then by value. "Rock" and "rock" count as one, since values compare ignoring case. The search matches part of the value.
- **List filters** (`tag_filters()`), for releases and tracks, are checked and turned into ranges:
  - text: `equals` or `contains`, ignoring case: "A text tag filter takes a value, or part of one";
  - numbers and dates: `equals`, or `min` and `max`, both included: "A number tag filter takes a value, or a range";
  - a date covers its whole year, month or day, so `equals: "2024"` is all of 2024, and `max: "2024-03"` runs through March 31.

  `repo::tags::tag_matches()` makes each one a sea-query condition, `releases.id in (select release_id …)` (or `tracks.id`) over the value table, which uses its `(tag_id, value)` index. Several filters must all match, even on the same tag. The release list and the track list use them.
- **Bulk edits** change one tag on many releases at once, in one transaction, and return how many releases changed.
  - `set_tag_on_releases()`: a single-value tag gets the value instead of its old one, and a multi-value tag gets it added. Releases that have it already, ignoring case, are left alone. A blank value: "Type a value to set".
  - `remove_tag_from_releases()`: removes one value, or every value of the tag when none is given.
  - Ids of releases that don't exist are skipped.

### Dates

`dates.rs` reads a date as the user types it, `2006`, `2006-01` or `2006-01-10`: epoch seconds at midnight UTC on its first day, and how much of it is known. slunk worked dates out with JavaScript's `Date.UTC()`, and `dates.rs` does it with whole days, refusing what slunk refused:
- anything else: "Not a date: 06. Write it as 2006, 2006-01 or 2006-01-10.";
- a day that doesn't exist: "There's no such date: 2006-02-30";
- a year before 100, which `Date.UTC()` read as 1900 to 1999: "There's no such date: 0099".

`last_day()` is the last day of a date's year, month or day, for a filter's range, and `format_date()` writes a date's known parts as `toISOString()` writes them: `2006`, `2006-01` or `2006-01-10`.

## The library check

`crates/slunk/src/library_check.rs` checks the library against what was registered, and the catalog for gaps the database can't rule out. It joins the media context, which reads the disk (`media/check.rs`), and the catalog. Running it writes nothing; the fixes are separate calls. Each rule here has a test in `tests/library_check.rs`, or next to its code.
- **Missing and changed files:** it reads every registered file's size and mtime, without hashing. A file that isn't there is missing. One whose size or mtime isn't what was read has changed, with what's on disk now.
- **Where a missing file moved:** when files are missing, it scans the library, which is the database's folder and the top folders outside it that hold registered files (`../shared` for `../shared/a.m4a`), and hashes the unregistered files of a missing file's size. Those with the same hash are where it may have moved, the ones with its name first. The likely move is the only one, or else the only one with its name.
- **Unregistered files** are the files in folders that hold registered ones that aren't registered themselves, by folder. The files of a missing file's size that were hashed are left out, matched or not, as slunk left them out.
- **Order:** missing files, changed files and folders as a file browser sorts paths ([The media context](#the-media-context)), and a folder's files as browsing lists them.
- **The catalog's gaps:** releases with no artist, by title, and tracks on various-artists releases with no artist of their own but features, whom nobody performs, by release title, then release, disc and number.
- **Relinking** (`relink_files()`) points files that moved at their new paths, one move at a time, as editing a file's path does: each file is read again, and a path that's taken, or has no file, is refused. A refused move leaves the ones before it saved.
- **Rehashing** (`rehash_files()`) reads changed files again: every file is read first, so one that's gone fails the call before anything is saved ("There's no file at music/a.flac"). Then their size, mtime and hash are saved in one transaction, and the tracks whose audio they are get their durations read again, saved where they differ. Ids that aren't registered are left out.

## The external libraries context

`crates/slunk/src/external/` is the external libraries context: a selection of the library, for a folder or an iPod. It has the mapping, a track's tags as every external library writes them and its path in a folder library; external libraries, their rules, what they come to, and what that will take; syncing a folder library; and syncing an iPod. Each rule here has a test in `tests/external.rs`, `tests/external_libraries.rs` or `tests/external_sync.rs`, or next to its code.

It reads the catalog's and the tags' tables itself, through their repositories, with one connection, so a sync's plan reads everything it needs in one read transaction, which the catalog's methods, each taking the connection on its own, can't give.

### External libraries

An external library is a destination, a selection and a profile (`external/repository.rs`; [reference/database.md](../reference/database.md#external-libraries)).
- **Creating** one gives it a UUID, version 4, from `/dev/urandom`, which its destination's manifest will name it by. Names are cleaned like other text, and unique ignoring case, which the database enforces: "There's already one with that name, ignoring case".
- **Its kind** is a folder or an iPod, and can't change: "An external library can't change from a folder to an iPod, or back". An unknown id is `NotFound`: "There's no external library 12".
- **A folder library's folder** is an absolute path, resolved lexically, and never where the user's own files are, since a sync writes there ([What liquidslunk writes](../README.md#what-liquidslunk-writes)). Refused: none ("A folder library needs a folder"), a relative one, the library's own folder, one inside it, one that holds it, and one that holds a registered file, which is a range on the path index, as for browsing. A folder belongs to one external library, which the database enforces: "Another external library uses that folder".
- **Its profile:** the format lossless files become, `original`, `flac`, `alac` or `aac`; AAC's bitrate, 128 to 320 kbps; CD resolution; a path template, checked, and the default when it's blank; and covers: embedded or not, their size, 100 to 3000 pixels, and a `cover.jpg` or not.
- **An iPod's** profile is what it plays: Apple Lossless or AAC ("An iPod plays Apple Lossless or AAC"), at CD resolution, with no folder, template, embedded covers or `cover.jpg`, whatever's given. Its iPod is the one connected at its first sync ([Syncing an iPod](#syncing-an-ipod)).
- **Its destination** (`destination()`) is connected when a folder library's folder is there and its manifest names this external library, or it has none yet; missing when the folder isn't there; and elsewhere when its manifest names another external library, which a sync refuses: "This folder belongs to another external library, Car." An iPod library's is the mounted iPod whose manifest names it, or else the only one with no manifest that liquidslunk syncs; with several of those, it's `Several`, and a sync is refused until all but one are ejected. The manifest's identity is read again only when its size or mtime changes, so the sidebar's dots don't read it each time.
- **Locate** (`locate()`) points a folder library at its folder where it is now: one that moved, or a card that mounted under another name. The folder is checked as a new one is, and refused when its manifest names another external library.
- **Deleting** one deletes its rules, and touches nothing on its destination. With `remove_files`, its destination has to be connected, and what liquidslunk wrote there goes first: the files its manifest lists, the folders that leaves empty, and `.liquidslunk/`. On an iPod, its tracks' files go, the database is written again without them, and so is the manifest; it's refused while the iPod also has tracks from somewhere else, which a database of liquidslunk's own would leave out. Deleting one that's syncing is refused.

### Rules and what they come to

A track is in an external library when a rule includes it and no rule leaves it out (`repo::external_libraries::members()`, one query):
- **A release** brings in its tracks. **An artist** the tracks of every release it's a release artist of, now and later, but not its appearances on other artists' releases. **A track** brings in itself.
- **A tag value** brings in the releases with it, whole, and the tracks with it. It's a list's tag filter (`tag_filters()`, [The tagging context](#the-tagging-context)) on releases or on tracks: a text tag's value, ignoring case, or a number or date tag's value or range, both ends included. A text rule has no "contains".
- **Everything** brings in every track, now and later: a "select all" that a static list of releases couldn't be, since it would miss what's imported after. The other rules stay as they are beside it, so removing it leaves what they bring in.
- **Left out:** a release or a track, which wins over every rule, so "everything but The Wall" is an everything rule and The Wall left out.
- **Order:** by release title, ignoring case, then release, disc and number.

`set_rules()` replaces an external library's rules, in one transaction. A tag value rule's values are cleaned like text and have to read as the tag's type. Each thing is named once: a rule given twice is kept once, text values ignoring case. Refused: leaving out anything but a release or a track ("Only a release or a track can be left out"), including and leaving out the same thing ("A rule can't both include something and leave it out"), a text rule with a range ("A text tag value rule takes a value"), a number or date rule with neither a value nor a range, and something that isn't there ("There's no release 12", "There's no text tag 3"). Deleting a release, an artist, a track or a tag deletes its rules (CASCADE).

**From the catalog's pages,** `add()` and `remove()` change one thing's rules, in one transaction:
- Adding a release brings it in whole: its exclusion goes, and its tracks', and it gets a rule. An artist gets a rule. A track's exclusion goes and it gets a rule; when its release is left out, the release's other tracks are left out one by one instead, since the release's exclusion would still leave it out. Ids that aren't there are skipped, as in the bulk edits.
- Removing something takes out its own rule, and when a broader rule still brings it in, leaves it out. A release's tracks' own rules go with it, those that leave them out too. An artist's, a tag value's or an everything rule just goes.

**Memberships** (`memberships()`) say which external libraries a release, an artist or a track is in, by name, and why, with how many of its tracks are in:
- a track: its own rule, or else its release's, an artist's, a tag value's or an everything rule;
- a release: its own rule or an artist's, even with no tracks yet, unless it's left out; or else a tag value's that brings in all its tracks; or else an everything rule, even with no tracks yet, unless it's left out; or else only some of its tracks, through their own rules;
- an artist: the external libraries with a rule for it. An everything rule doesn't count, since only an artist's own rule can be removed from its page.

**What it will take** (`contents()`): its releases, its tracks, and an estimate of their size (`estimated_size()`, `external/estimate.rs`), by what each source becomes ([Syncing a folder library](#syncing-a-folder-library)): a lossless file that becomes AAC takes its duration at the bitrate; anything else, copied or lossless, its own size. An `.m4a` counts at its own size, since its extension doesn't say whether it's AAC or Apple Lossless, and covers aren't counted. Tracks a sync will leave out are counted apart: APE, WavPack and Musepack except in a folder library that keeps the originals, and DSF, DFF and WMA always. `sample_track()` gives a track to show the path template with: the first one in it, or else the catalog's newest.

### Syncing a folder library

A sync works out a plan, shows it, then carries it out. `plan_sync()` keeps the plan it works out, and `sync()` runs that plan, by its id: a plan that isn't the last one shown is refused ("The plan is out of date. Look at it again."), and so is one whose folder's manifest changed since.

**What each source becomes** (`convert.rs`), read from the file's headers with lofty, not from its extension, since an `.m4a` may be AAC or Apple Lossless:

| Source | Keep the original | FLAC | Apple Lossless | AAC |
|---|---|---|---|---|
| FLAC, ALAC, WAV, AIFF | Copied | FLAC | ALAC | AAC |
| MP3, AAC, Ogg Vorbis, Opus | Copied | Copied | Copied | Copied |
| APE, WavPack, Musepack | Copied | Left out | Left out | Left out |
| DSF, DFF, WMA | Left out | Left out | Left out | Left out |

- A file in the format it would become is copied. At CD resolution, a lossless file above 16 bits or 48 kHz is converted, even to its own format: to 16 bits, at 44.1 kHz from 88.2, 176.4 or 352.8 kHz, at 48 kHz from other rates above 48 kHz, and at its own rate otherwise. Without CD resolution, lossless keeps its bits, up to 24.
- A file that's left out is listed with its reason: "Core Audio can't convert APE, so it's only copied, to a folder library that keeps the originals", "liquidslunk can't tag DSD files".

**Converting** runs `/usr/bin/afconvert` directly, never through a shell, reading the source and writing into the sync's temporary folder:
- **AAC** is `-d aac -b <bitrate> -s 2 -q 127`: constrained VBR, at the encoder's best quality, in an M4A, at 44.1 or 48 kHz when the source is above 48 kHz. Apple's encoder takes at most some bitrate for a sample rate and channels (8 kHz mono, 24 kbps; 44.1 kHz mono, 256 kbps), so when it refuses the profile's, the next lower of 320, 256, 192, 160, 128, 96, 64, 48, 32, 24, 16 and 8 kbps is tried.
- **Apple Lossless and FLAC** always name their bit depth (`-d alac/1` is 16-bit, `/2` 20-bit, `/3` 24-bit): otherwise `afconvert` makes 32-bit Apple Lossless from a 16-bit FLAC. Lossless transcodes are bit-exact.
- **Dither:** `afconvert` only rounds when it cuts bits, unless it's asked, so a conversion that cuts bits or changes the sample rate of lossless audio passes `--dither 1`, TPDF. Without it, a 1 kHz tone at 0.4 of a 16-bit step, in a 24-bit file, came out as digital silence; with it, at 0.404 of a step, in ±1 of noise.
- A file `afconvert` can't convert is a failure of that file, with what it said, and the rest of the sync goes on.

**Covers** (`covers.rs`): a release's cover, at most the profile's size on its longest side, never enlarged, as a JPEG at quality 90, transparent pixels on white. A JPEG already within the size is written as it is. A plan checks a cover from its headers only, and lists one it can't read, or in another format than JPEG, PNG, GIF, BMP, TIFF or WebP, as left out, with its reason. The tracks are written without it.

**Tags** (`write_tags.rs`) replace whatever a file had, in each format's own fields, from `Tags` ([A track's tags](#a-tracks-tags)), with the cover as the front cover:
- **FLAC and Ogg:** Vorbis comments, one per value, and the cover as a `PICTURE` block, or `METADATA_BLOCK_PICTURE` in Ogg.
- **MP3, AAC in ADTS, WAV and AIFF:** ID3v2.4, several values in one frame separated by nulls, and `TXXX` for `ARTISTS`, `CATALOGNUMBER` and `RELEASETYPE`. ID3v1, APE and RIFF INFO tags are removed.
- **M4A:** MP4 atoms, one data atom per value, freeform ones as `----:com.apple.iTunes:<NAME>`. What `afconvert` writes among the tags is kept: the encoder's gapless data, `iTunSMPB`, without which an AAC file plays with gaps, and Sound Check, `iTunNORM`.
- **APE, WavPack and Musepack** kept as they are: APEv2.
- They're only ever written into a file liquidslunk made: a copy or a conversion in the temporary folder. lofty's `remove_from_path()` opens a file read-only and then fails to truncate it, so tags are removed through a file opened for writing.

**The manifest** (`manifest.rs`) is `.liquidslunk/manifest.json` at the folder's root: its format's version, the external library's UUID and its name then, and each file liquidslunk wrote, with its path, its size and mtime as read back once written, and what it holds:
- a track's file: its track, its audio key (the source's SHA-256, what it became, like "aac 256 kbps", and liquidslunk's way of converting, `CONVERSION_VERSION`; the encoder, macOS's version, is recorded but not compared, so an update doesn't convert everything again), its tags as written (`Tags::fields()`), and its cover's key (the cover's SHA-256 and its size);
- a `cover.jpg`: its release and its cover's key.

A sync appends each change to `.liquidslunk/journal.jsonl` before it renames a file into place, or after it removes one, and waits until the line is on the disk. The manifest is read with its journal replayed, a last line cut short left out, and rewritten atomically, a temporary file then a rename, when a sync ends, which removes the journal. So a sync interrupted by a card pulled out carries on from its last file. Rewriting the whole manifest after each file instead would cost too much on a slow card: it's a few hundred kilobytes for a few hundred tracks. A file is as written when its size and mtime are: FAT32 keeps local time, so a shift of whole quarter-hours, up to a day, from a change of time zone or of daylight saving time, doesn't count.

**The plan** (`plan.rs`, a plain function of what should be there and what is):
- **What should be there** is read in one read transaction (`Store::read()`): the tracks the rules come to, their tags, their files and their releases' covers. Then the connection is let go, and the sources' headers are read, several at once.
- **What is there** is the manifest, each file checked against the disk by its size and mtime, without hashing.
- **Paths** come from the template, told apart as `unique_paths()` tells them apart, except that a track keeps the path it has while it's one of its template's numbered forms, so names don't trade places between syncs.
- **Each track:** added when it has no file; converted again when its audio key changed; restored when its file is missing or not as written; moved when its path changed, and retagged too when its tags or cover changed; retagged when only those changed. A retag says what changed, field by field: "Title: Shoka → 初夏".
- **A `cover.jpg`**, when the profile asks for one, in each folder that holds one release's tracks: added, written again when its cover changed, or restored.
- **Removed:** what's there and no longer wanted.
- **Left out:** a file liquidslunk didn't write at a track's path, which is never replaced ("A file liquidslunk didn't write is at …"); a track whose source is missing from the library, whose copy stays as it is; a source that can't be converted; a cover that can't be read.
- **Space:** what it writes, estimated (AAC from durations and the bitrate, lossless in proportion to the bits and the rate it keeps, copies at their size), less what the files it removes or replaces take, against what's free on the folder's volume. A sync that doesn't fit is refused: "It needs 3.2 GB more than the folder has free."

**Running it** (`sync.rs`):
1. `.liquidslunk/tmp` is made and emptied of what an earlier sync left.
2. **Removals**, and the old files of tracks written again at a new path, go first, to make room.
3. **Moves:** a file is renamed to its new path. A move to a path another move has yet to leave waits for it, and when every one waits, one moves aside first.
4. **Additions, conversions and restores:** converted or copied, then tagged, in the sync's temporary folder (`scratch.rs`), on one thread per core but one; written to the folder one at a time, as they're ready. Each file is copied byte by byte, without its attributes, under a temporary name in `.liquidslunk/tmp`, waited on until it's on the disk, then renamed into place, which is atomic on one volume.
5. **Retags:** the file is copied into the temporary folder, tagged there, and written back the same way, over the old one.
6. **Covers,** the same way.
7. The manifest is written, the folders the sync left empty are removed, those with anything else in them stay, and `last_synced_at` is set, unless it was cancelled.

- **Progress** reports the stage, the steps done and all of them, the bytes written, and the track in hand. **Cancelling** (`cancel_sync()`) stops after the file in hand, and keeps what's done.
- **One sync at a time:** another is refused while one runs. The library can be browsed and edited meanwhile: the sync works from the plan.
- **Failures:** a file that can't be converted or tagged is listed, and the sync goes on. A folder that can't be written ends the sync with its error, keeping what's done. A folder that's gone, a card pulled out, says so: "/Volumes/CARD/Music isn't there any more: its disk may have been ejected or unplugged. Connect it again, and the next sync carries on from there."
- **Never outside the folder:** a sync makes folders only inside its destination's folder, never the folder itself or anything above it, so a card that's gone isn't made again on the Mac's own disk.
- **No AppleDouble files** (`folder.rs`). macOS gives every file and folder a process creates an extended attribute, `com.apple.provenance`, which can't be removed, and FAT32 and exFAT keep extended attributes in a `._` file beside the file, which some players list as a broken track. Opening an existing file for writing makes one too, while a rename doesn't. So every file is written new, never rewritten in place, and the `._` file of each file or folder liquidslunk makes, and of the journal at each append, is deleted as soon as it's made. After a sync of 283 tracks to an exFAT card image, it had no `._` files.
- **Temporary files:** each sync has its own folder in the system's temporary folder, `liquidslunk-sync-<pid>-<n>`, with a lock held while it runs, and removed when it ends. At launch, those whose lock no sync holds are removed (`scratch::remove_leftovers()`).

**Ejecting** (`eject()`): a folder on a volume other than the Mac's own disk, `/` and the volumes in `/System/Volumes`, or an iPod, is ejected with `diskutil eject`, so it can be unplugged. Not while a sync runs.

### Syncing an iPod

An iPod library syncs the iPod with video, 5th and 5.5th generations. What liquidslunk reads and writes on it, byte by byte, is in [reference/ipod.md](../reference/ipod.md).

**Finding it** (`ipod/mod.rs`): an iPod is a volume in `/Volumes` with an `iPod_Control/iTunes` folder, which it has when its disk use is on. Its model is read from `iPod_Control/Device/SysInfo`: another model is refused ("liquidslunk syncs only the iPod with video (5th and 5.5th generations). This one is model MB029."), and an iPod without one is synced with a warning. `mounted_ipods()` lists them, each with the external library its manifest names. An iPod library's iPod is found as [External libraries](#external-libraries) says; it gets its manifest, `iPod_Control/iTunes/liquidslunk.json`, at its first sync, and is recognized by it wherever it mounts. Tests look in another folder than `/Volumes` (`External::with_volumes()`).

**Its plan** (`plan::plan_ipod()`) is a folder library's, with these differences:
- **Paths:** the iPod names its own files. A new file goes in one of the `F00` to `F49` music folders already there, at random, as `LSK` and four random letters and digits, with its extension in lowercase. A file written again, converted or restored, gets a new path, and the old file goes once the database no longer lists it. There are no moves, no `cover.jpg`s, and no paths of someone else's to leave alone.
- **What is there** is the manifest checked against the disk, and against the iTunesDB: a file it lists but the database doesn't, as after an iPod unplugged midway, is retagged with "Listed on the iPod: No → Yes". A sync cut short can leave two files for a track: the one that's best kept stays, and the other is removed.
- **A retag** changes only the database. **Covers** are the iPod's artwork, made at 200 pixels, so a cover's key is its SHA-256 and 200.
- **Tracks from somewhere else,** those the iTunesDB lists at paths the manifest doesn't, are the plan's `other`. A sync is refused unless the user agrees to erase them ("This iPod has 1,234 tracks from somewhere else. Agree to erase them to sync it."), with `sync`'s `erase`.
- **What each source becomes** is as in a folder library with the iPod's format, and Ogg Vorbis and Opus become AAC. Every conversion passes `--soundcheck-generate`.

**Running it** (`ipod/sync.rs`). The iPod reads only its database, which is written last, so an iPod unplugged midway still plays what it played before:
1. With `erase`, the files of tracks from somewhere else are deleted, only in `iPod_Control/Music`, and never one the manifest lists.
2. When what's added wouldn't fit before what's removed is gone, the database is first written without the removed tracks and the files they replace, and those go.
3. **New files** are converted or copied, and tagged, on one thread per core but one, then their facts are read (below), and each is written to the iPod as a folder library's are, journaled before it's renamed into place. A track written again keeps its `dbid`, and when it was added.
4. **The database:** the artwork, then the iTunesDB, each under a temporary name in `iPod_Control/iTunes/liquidslunk-tmp`. The old iTunesDB is copied to `iTunesDB.liquidslunk-previous`, the first time in a sync, `Play Counts` is removed, and the new files are renamed over the old ones. Retags are journaled once the database has them.
5. **Removals:** the files of removed tracks, and those new files replaced.
6. The manifest is written, and `last_synced_at` is set.

- **Cancelling** stops after the file in hand, and the database is still written, with the retags and removals, for what's done.
- **The database** is written from the manifest alone, whose files on an iPod keep what it needs: each track's `dbid`, its kind, length, bitrate, sample rate, Sound Check, gapless data, and when it was added. Its tags are the manifest's as written: title, artist, album, album artist, the sort fields, the year from the date, compilation, and track and disc numbers with their totals. It lists the tracks by album artist, album, disc and number, each by its sort form when it has one. The iPod browses by the master playlist's sorted indexes, which sort by the sort fields, so a Japanese title sorts by its transliteration.
- **Counts:** each track's play count, skips, their last times and its rating are the iPod's: they're read from the database it replaces, with `Play Counts`' added in by position, and carried over by `dbid`. They're never brought into the library.
- **Facts** (`ipod/audio.rs`), read from the file written: an AAC file's encoder delay, padding and samples from its `iTunSMPB` atom; an Apple Lossless file's samples, with no delay or padding; a copied MP3's from its LAME header and its frames; Sound Check from `iTunNORM`, which `afconvert --soundcheck-generate` adds. The length is the samples played, since lofty's counts the encoder's delay and padding in. Apple Lossless records no bitrate, so it's the file's average.
- **Artwork** (`ipod/artwork.rs`): each track with a cover gets an image in the ArtworkDB, with a 100 × 100 and a 200 × 200 thumbnail, RGB565, the cover scaled to fit and centred on white. A release's tracks share their thumbnails' pixels. The `.ithmb` files are made again at each write, in the sync's temporary folder: a cover that's the same as before is copied from the old files, and a new one is made from the library's, several at once.
- **The database's ids** are kept from the one it replaces: its own, its library's and its master playlist's. An iPod without a database gets new ones.

### A track's tags

`tags_of()` works out tracks' tags from the catalog (`external/tags.rs`). It reads everything they need with one connection, with a query per kind of row. It writes only what the catalog's own structure says, the same for every external library:

| Field | From the catalog |
|---|---|
| Title | The track's main title, with its features: "Bridges (feat. Mari, Sander)" |
| Artist | Who performs the track (`performers()`, [Tracks](#tracks)), joined with ", " |
| Artists | Each performer, then each feature |
| Album | The release's main title |
| Album artist | The release's artists, joined with ", ". A compilation's is its release artist, like an artist named "Various Artists" |
| Compilation | A various-artists release |
| Date, and year | The release date, as precise as it's known: `2006`, `2006-01` or `2006-01-10` |
| Track | Its number, and the last track number on its disc |
| Disc | Its disc, and the release's last disc |
| Disc title | Its disc's main title |
| Label | The release's labels, by their main names, in order |
| Catalog number | The release's |
| Media, and release type | The release's media and formats, as they're named, in the release's order: by their own ids |
| Sort fields | Below |
| Cover | The release's cover. Tracks don't have their own |

- **Artists are named as credited:** the name printed on each link. An "&" is only ever part of a name.
- **A field with no value isn't written:** a list is empty, and anything else `None`.
- **Tag values and credit lines aren't read.** Which tags exist and what roles say is up to the user, so no rule turns them into fields: a rule from a tag's name or a role's spelling to a field would guess. So the genre, grouping, comment, rating and composer fields stay empty, and so do the iPod's Genres and Composers menus and its stars. Formats and media are part of the schema, so they're written as they're named.

**Sort fields**, for the artist, the album artist, the album and the title, are what players sort by instead of a name or title that isn't in Latin script, so a Japanese release sorts by its transliteration, not after Z:
- **Latin script:** a text is in Latin script when none of its characters belongs to another script, by ICU's script property. Digits, punctuation and spaces belong to none (`is_latin()`).
- **A title** sorts as its first alternate title that's a Latin-script form of it: in Latin script, with a language tagged with the Latin script, in the title's own language, like `ja-Latn` for `ja`, or in any language when the title's own isn't known. A track's title without a language has its release's. So "初夏" sorts as "Shoka", in `ja-Latn`, and not as its translation, "Early Summer", in `en`.
- **A name an artist is credited with** sorts as the artist's main name when that's in Latin script, whatever its language, or else as the first of the artist's alternate names that's a form of it. A real name isn't one.
- **Names joined with ", "** sort as each one's sort name, or else the name itself: "福居良, Mari" sorts as "Ryo Fukui, Mari". When none of them has one, there's no sort field.

### Path templates

`Template::parse()` reads a folder library's path template (`external/template.rs`), and `path()` gives a track's path from its tags and its file's extension. The default, `DEFAULT_TEMPLATE`:

```
${album_artist}/[${year} - ]${album}/[${disc}-]${track_number:2} ${track_title}.${format}
```

gives `Ryo Fukui/1976 - Scenery/01 It Could Happen to You.flac`, and `…/2-01 ….flac` on a release with two discs.

| Variable | Its value |
|---|---|
| `${album_artist}`, `${artist}` | The album artist and the artist, as tagged |
| `${album}` | The release's title |
| `${year}`, `${date}` | The release date's year, and the date as precise as it's known |
| `${cd_number}`, `${cd_count}` | The track's disc, and the release's last disc |
| `${disc}` | The track's disc on a release with more than one, and empty on a release with one |
| `${disc_title}` | Its disc's title |
| `${track_number}`, `${track_count}` | The track's number, and the last one on its disc |
| `${track_title}` | The track's title, as tagged, with its features |
| `${label}`, `${catalog_number}` | The release's first label, and its catalog number |
| `${format}` | The file's extension: `flac`, `m4a`, `mp3` |

- **`${name:N}`** pads a number to N digits, from 1 to 9: `${track_number:2}` is `03`, and 123 stays 123.
- **`[…]`** is left out when a variable in it is empty. It can hold a `/`, so a folder can be optional: `[${disc_title}/]`. Brackets can't go inside brackets.
- **`/`** in the template's own text separates folders. A value's own `/` doesn't: it's replaced, so a title can't make a folder.
- **Refused:** a template is cleaned like other text, then refused, with the reason, when it has a variable it doesn't know (`${genre} isn't a template variable`), pads a variable that isn't a number, leaves a `${` or a `[` open, has a `]` with no `[` or brackets inside brackets, doesn't end in `.${format}`, or has neither `${track_number}` nor `${track_title}`, which would give every track of a release the same name.

**Names on disk** are made safe for FAT32 and exFAT, each folder's and the file's (`safe_name()`):
- in NFC, with `/ \ : * ? " < > |` and control characters as `_`;
- without leading dots, which hide a file on macOS and Linux ("...And Justice for All"), or trailing dots and spaces, which Windows drops;
- at most 255 UTF-16 code units, cut at a character, and a file's name before its extension;
- `_` for a name left empty.

**Two tracks with the same path** are told apart by `unique_paths()`. Paths are compared ignoring case and Unicode normalization, as FAT32, exFAT and APFS compare them. The track with the lowest id keeps the path, and the others get " (2)", " (3)", … before the extension, skipping any path another track has.

**The track page** shows a track's tags with its path by a template, in a folder library that keeps the original files, so the file keeps its extension, in lowercase (`exported()`).

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

The first eight are slunk's: its Kysely migrations' statements, copied from what Kysely actually ran, so they build slunk's schema to the character. Each module runs its statements in order with `run()`, each prepared on its own, as Kysely ran them. Two have logic: `add_tag_value_rules` checks the rows already there before adding its triggers, and `add_credited_names` rebuilds four tables. The ninth, `add_external_libraries`, is liquidslunk's first of its own, the tenth, `add_everything_rules`, rebuilds its rules table to add everything rules, and the eleventh, `add_artist_aliases`, adds artist aliases.

The rules:
- Never edit a migration once it has been committed. Write a new one.
- A new migration is a one-way door: slunk refuses a library with a migration it doesn't have, so it can't open that library again, apart from its backups.
- Every `down` undoes its `up`, or fails with an explicit "irreversible" error.
- A migration runs inside the runner's transaction, so it never starts its own.
- Migrations don't use app code, which changes over time while a migration must not. The exception is `rebuild_table`, which is part of the migration system.
- Starter rows, like the first release formats, are inserted by migrations, and nothing may depend on them. Sample data goes in the seeds.

Writing one:
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

`tests/parity.rs` checks slunk's eight migrations, the first eight in `MIGRATIONS`, against slunk's, with fixtures in `tests/fixtures/parity/` that slunk's own runner made:
- `fresh.schema.json` and `fresh.sql`: a new database slunk migrated. Rust's has the same `sqlite_schema`, row for row, and the same rows.
- `state_<k>.sql`: a database after slunk's first *k* migrations, each followed by its seed in `seeds/`, rows in every table it made. `final_<k>.sql` is that database after slunk's runner applied the rest, and Rust's runner, from the same state, ends with the same database.
- `rolled_back_<k>.sql`: the database after *k* migrations, after slunk's runner rolled the last one back. Rust's rollback ends with the same database.

The tests compare everything but `migrations.applied_at`. One more test checks that every `down` undoes its `up`, liquidslunk's own migrations too. `add_track_tags`' down recreates three triggers with the baseline's logic, laid out differently, as slunk's did, so that test compares SQL without whitespace and parentheses.

The fixtures were made by slunk's runner while its backend was in the repository, and aren't made again: they record what slunk's eight migrations did, and those never change. A new migration has no slunk version to check against, so its tests are its own.

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

`seed` loads the sample data in `repo/seeds.rs`, a port of slunk's seeds that makes the same rows: five releases, with their artists, labels, formats, media, alternate names and titles, a credited name, and the tags Genre and Score. There are no files or tracks, which need real files on disk. It writes the rows directly, in one transaction, and skips a database that already has releases.

## The app

### Commands

The frontend calls the backend through Tauri commands, in `src-tauri/src/commands/`, one module per context:
- `library.rs`: `library_status`, `open_library`, `create_library`, `close_library`, `recent_libraries` and `reveal_library`;
- `media.rs`: `files`, `file`, `file_counts`, `folder`, `folder_files`, `register_files`, `update_file` and `delete_file` ([The media context](#the-media-context)), and `open_file` and `reveal_file`;
- `catalog/` ([The catalog context](#the-catalog-context)), a module per entity:
  - `lists.rs`: `list_items`, `list_item`, `list_item_options`, `create_list_item`, `rename_list_item` and `delete_list_item`, each with a `kind`;
  - `artists.rs`: `artists`, `artist`, `artist_options`, `artist_names`, `artist_aliases`, `create_artist`, `update_artist`, `set_artist_picture`, `add_artist_name` and `delete_artist`, and the artist page's: `artist_header`, which every tab shows, with the artist's aliases, `artist_releases`, `artist_appearances` and `artist_credits`;
  - `labels.rs`: `labels`, `label`, `label_options`, `create_label`, `update_label`, `set_label_picture` and `delete_label`, and the label's page, `label_page`;
  - `releases.rs` ([Releases](#releases)): the list, `releases` and `release_ids`; the release page, `release_page`; the edit tabs' header, `release_header`, and their reads, `release`, `release_files` and `release_credits`; `create_release`, `update_release`, `delete_release`, `set_release_cover`, `set_release_files` and `set_release_credits`; the picker's `release_options`, `credit_roles`, and the dashboard's `catalog_totals`;
  - `tracks.rs` ([Tracks](#tracks)): the list, `tracks`; the track page, `track_page`; the track form's read, `track_form`, and `create_track`, `update_track` and `delete_track`; and the Tracks tab's, `release_tracks`, which with a folder has the import's rows too, and `set_release_tracks`; and the player's: `player_tracks`, which returns what the player shows of each track it's given (its title, who performs it, its release and the release's cover, and its file), once each, in the order given, where an id that isn't a track is left out, not an error, since a queue can name a track deleted since; and `release_track_ids`, a release's tracks' ids by disc, then by number, none for a release that isn't there; and the track picker's `track_options`, the first 20 tracks by title, with their release. Their tests are next to them;
  - `import.rs` ([Import suggestions](#import-suggestions)): the import's reads, `release_suggestion` and `import_files`, the release's cover and links with the folder's files;
  - `mod.rs`: `languages`, and what they share: the DTOs of what a release refers to, and `Related`, which loads it;
- `library_check.rs` ([The library check](#the-library-check)): `library_check`, and its fixes, `relink_files` and `rehash_files`;
- `external.rs` ([The external libraries context](#the-external-libraries-context)): `external_libraries`, each with whether it's connected; `external_library`, its page, with what's in it, what that takes, and a sample track for its template; `create_external_library`, `update_external_library` and `delete_external_library`; its rules, `external_library_rules` and `set_external_library_rules`, each with what it names; the catalog pages', `add_to_external_library`, `remove_from_external_library` and `external_memberships`; `external_library_tracks`, what's in one, a page at a time, as the track list shows tracks; `exported_tags`, a track's tags as an external library writes them, with its cover, and its path by a template, the default one unless one is given, where a template that can't be used is `invalid`, with the reason, and a track that isn't there is null; and syncing ([Syncing a folder library](#syncing-a-folder-library), [Syncing an iPod](#syncing-an-ipod)): `sync_plan`, what a sync would do, with its labels, what's left out, whether it fits, and on an iPod, the iPod and its tracks from somewhere else; `sync`, which runs it by its id, with its progress through a `Channel`, and `erase` on an iPod with tracks from somewhere else, and returns how it went; `cancel_sync`; `eject_external_library`; `locate_external_library`; and `mounted_ipods`, the iPods mounted now, with their model, free space and the external library each belongs to. `external_library` says whether its destination is connected, missing, another external library's or one of several iPods, what's free there, whether it ejects, whether it's syncing, and an iPod library's iPod, and `delete_external_library` takes `removeFiles`;
- `menu.rs` ([The menu bar](#the-menu-bar)): `set_controls_menu`;
- `tagging.rs` ([The tagging context](#the-tagging-context)): `tags`, `tag`, `tag_options`, `tags_by_ref`, `tag_values`, `tag_value_options`, `create_tag`, `update_tag` and `delete_tag`, which name a tag by its `type` and id; a release's tags, `release_tags` and `set_release_tags`; and the bulk edits, `set_tag_on_releases` and `remove_tag_from_releases`.

Their conventions:
- **Async.** Every command is `async`, and does its database and disk work on Tokio's blocking pool, with `blocking()`, or `on_library()` for one that needs the open library, which is `noLibrary` without one. A command that isn't `async` runs on the main thread, and freezes the window while it works. The one exception is `set_controls_menu`, whose work belongs on the main thread: the menu is there.
- **Thin.** A command that uses the library is a wrapper over a plain function that takes it, like `media::register(library, args)` or `catalog::artists::create(library, args)`, which maps the context's records to DTOs.
- **Results.** Every command returns `Result<_, CommandError>`.
- **Arguments.** A command takes its arguments as one struct, `args`, like `open_file`'s `FileArgs`, so the frontend passes them as an object, as `useCommand()` expects: `call("openFile", { id })`. A command can also take a `Channel` to report progress, as `open_library` does with a path.
- **DTOs** derive `Serialize` (or `Deserialize`) and `specta::Type`, with `#[serde(rename_all = "camelCase")]`. Enums are tagged: `{ type: "created", … }`. A DTO has the fields and names the GraphQL query it replaced asked for, so components changed little in the port: a file's `type`, `sizeBytes` and `uses[].entityId`. A shared one is flattened in with `#[serde(flatten)]`, as `FileDetails` holds a `File`, and an update's arguments hold its input: `UpdateArtistArgs` is `{ id, ...ArtistInput }`.
  - No serde attribute that makes a type's two directions differ, like `skip_serializing_if` or `deserialize_with`: specta then splits it into `…_Serialize` and `…_Deserialize` types. Where a field is there on one page and not another, there are two DTOs, like `CreditedArtist` and `NamedCreditedArtist`, with the artist's alternate names.
  - A field an update can leave out, rather than set to null, is a `Given<T>`, which has its own `Deserialize`: left out, `Given(None)`, and given, even as null, `Given(Some(…))`. A release's `titleLanguage` is one.
- **Related rows** are loaded the way slunk's DataLoaders loaded them: a page's rows first, then one query per kind of row they refer to. `Related` (`commands/catalog/mod.rs`) loads artists, the names they're credited with, labels, formats and media for a page, and builds their DTOs.
- **Pickers** have their own small commands, like `artist_options`, which return the first 20 matches by name.
- **Optional arguments** are `Option`s with `#[serde(default)]` and `#[specta(optional)]`, so the frontend leaves them out: `useCommand("files", { sort: "path" })`.
- **Ids** are `i64`, exported to TypeScript as `number` (`dangerously_cast_bigints_to_number()`): SQLite's ids stay far below 2^53.

**The bindings.** tauri-specta writes `src/bindings.ts` from the commands, the events and their DTOs whenever a debug build starts (`lib.rs`). It's committed, so start a debug build after changing a command, and commit what it writes. It wraps each result as `{ status: "ok", data } | { status: "error", error }`, which the frontend's `useCommand()` unwraps.

**Errors.** `CommandError` is `{ kind, message }`, where `kind` is:
- `noLibrary`: no library is open, and the frontend goes to Welcome;
- `notFound`;
- `invalid`: a message for the user, which a form or Welcome shows as it is;
- `internal`.

SQLite's constraint errors are `invalid`, rewritten by the table slunk's backend used, with the external libraries' own added: `UNIQUE constraint failed: tracks.file_id` becomes "That file is already a track's audio", and `UNIQUE constraint failed: external_libraries.folder` "Another external library uses that folder". The triggers' own messages pass through. Other errors are `internal`.

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
2. **Migrates**, with its backup. If anything is pending, `progress` hears `{ type: "migrating", pending }` first, so Welcome can say it's upgrading.
3. **Closes** the open library, if there is one, then **opens** the app's connection, and keeps the library in Tauri's state.
4. **Saves** the settings.

Each failure is an `invalid` error, in words. An I/O or locking error adds that SQLite can't keep a library on a network drive. The result carries the migration's backup and applied migrations, for a toast.

**Switching** opens the new library: the old one stays open until the new one is checked and migrated. **Closing** (`close_library`, and the app's exit) drops the connection, which checkpoints the WAL and removes the `-wal` and `-shm` files, when it's the last connection to the library. The settings still open the library at the next launch.

**One instance:** tauri-plugin-single-instance focuses the open window instead of starting a second app.

**`--library <path>`**, in debug builds only, opens a library at startup without touching the settings, for scripts: `npm run tauri dev -- -- -- --library /absolute/path/slunk.db`. The first `--` is npm's, the second ends the Tauri CLI's options, and the third the runner's.

### Serving files

`protocol.rs` serves `slunk://localhost/files/:id`, or `http://slunk.localhost/files/:id` on Windows: a registered file's bytes, by id, read-only, for `<img>` and `<audio>`. It answers on the blocking pool.
- An unknown id, no open library, or a file missing on disk gets a 404.
- The content type comes from the extension, by mime_guess, except `audio/mp4` for `m4a`, as slunk's backend sent it.
- Range requests get a 206 with `Content-Range`, so audio can seek. A response is a whole body in memory, so a range with no end (`bytes=0-`) gets 1 MB, and the media element asks for the rest as it plays. A request with no range gets the whole file.

The app's CSP allows `slunk:` and `http://slunk.localhost` in `img-src` and `media-src`. A page's `fetch()` can't read them: they're for media elements. Scripts and workers come only from the app's own files, by `default-src 'self'`: the player's worker is one of them, bundled by Vite ([frontend.md](frontend.md#nearly-gapless)).

### The menu bar

`menu.rs` builds the menu bar: Tauri's default menu, with a Controls menu between View and Window, as in Music. The player is in the webview, so the menu only passes clicks to it, and shows what it's told.
- **Its items:** Play, or Pause while the player plays; Next and Previous; Shuffle, checked while it's on; Repeat, a submenu of Off, All and One, the current one checked; and Volume Up and Volume Down. They're all off while nothing is queued, and at launch, until the player says otherwise.
- **Key equivalents:** ⌘→ for Next, ⌘← for Previous, ⌘↑ and ⌘↓ for the volume. Play has none, since Space would take it from every field. WebKit gives the page a key first, and the menu gets it only if the page leaves it: a field moves its caret, and the player handles the rest itself ([The player](frontend.md#the-player)).
- **A click** emits `PlayerControl`, a typed event, to the webview: `{ type: "toggle" }`, `next`, `previous`, `shuffle`, `{ type: "repeat", mode }`, `volumeUp` or `volumeDown`. It's tauri-specta's, so it's in the bindings, as `events.playerControl`. A check item checks or unchecks itself when it's clicked, so the click handler puts it back as it was, and the player then says what's changed.
- **`set_controls_menu({ playing, queued, shuffle, repeat })`** sets what the items show. The items are in the app's state, as `ControlsMenu`. The command isn't `async`: it runs on the main thread, where the menu is, so the player's calls change it in the order they're made.

### Opening files

`open_file` opens a registered file in its default app, and `reveal_file` shows it in Finder, with the opener plugin, from its absolute path. `reveal_library` shows the open library's database in Finder. The webview has no opener permission: only these commands use it.

## Conventions

- **Formatting and linting:** `cargo fmt`, and `cargo clippy --workspace --all-targets` with no warnings.
- **Tests:** `cargo test --workspace`. Tests make their own temporary databases and files, and never read `dev-library/`. Audio and language tag tests use the committed fixtures in `crates/slunk/tests/fixtures/`.
- **Comments** never cite a design doc. One that needs more than it says points to the section of `docs/` that describes it, by the doc's file name and the section's heading: `(backend.md, "Running migrations")`. A module's comment points once, and what's in the module points again only to another section ([Design docs and these docs](../README.md#design-docs-and-these-docs)). slunk's docs aren't in the repository, so they aren't cited either.
