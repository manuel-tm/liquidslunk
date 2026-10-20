# Database

> Keep this page up to date: any migration that changes the schema updates it in the same change.

This is the schema as built, by these migrations in `crates/slunk/src/repo/migrations/`, one module each (`m20261011141955_create_baseline.rs`, …). The first eight are slunk's, ported statement by statement, so a library slunk made opens as it is, and a new one gets slunk's schema exactly ([How it's checked](#how-its-checked)). The ninth is liquidslunk's own: slunk refuses a library once it has run, so slunk can open it again only from a backup made before.

| Migration | Adds |
|---|---|
| `20261011141955_create_baseline` | Every table, index and trigger, except those below |
| `20261013060918_add_tag_value_rules` | The triggers that check date and number tag values |
| `20261014052413_add_track_tags` | Tag values on tracks, and the `<type>_tags_multiple_off` triggers that check them too |
| `20261015192802_free_text_credit_roles` | Credit lines: `release_credits` and `track_credits` again, now with a free-text `role`, and `artists_release_credits` and `artists_track_credits`. It drops `credit_roles` and the old credits. |
| `20261015195853_add_disc_titles` | Disc titles: `release_disc_titles`, and the triggers that keep each title on a disc with tracks |
| `20261016144030_add_alternate_names` | Alternate names: `artist_alternate_names` and `label_alternate_names`, and `name_language` on `artists` and `labels` |
| `20261016151144_add_credited_names` | Credited names: `credited_name_id` on the four artist link tables, which it rebuilds |
| `20261016180957_add_alternate_titles` | Alternate titles: `release_alternate_titles`, `track_alternate_titles` and `release_disc_alternate_titles`, and the main titles' languages |
| `20261020045756_add_external_libraries` | External libraries: `external_libraries` and their rules, `external_library_rules` ([4_external_libraries.md](../design/4_external_libraries.md)) |

The schema is slunk's, carried over unchanged ([design §2.1](../design/1_introduction.md#21-what-the-port-keeps)), with external libraries added. This page describes what it holds and the rules it enforces, with the reasons where they aren't plain, as for [disc titles](#disc-titles) and [credited names](#credited-names). Every column's type is in its migration's `create table`, and `sqlite3 slunk.db .schema` shows them all.

How to connect and migrate: [development/backend.md](../development/backend.md#the-database).

## Conventions

- Every table is `STRICT`, so SQLite rejects a value of the wrong type.
- `files`, `artists`, `labels`, `releases` and `tracks` use `INTEGER PRIMARY KEY AUTOINCREMENT`, so a deleted id is never reused. They also have `inserted_at` and `updated_at`, which default to `unixepoch()`. The app sets `updated_at`.
- Other tables use `INTEGER PRIMARY KEY`.
- Booleans are `INTEGER` 0 or 1, with a CHECK.
- Instants are Unix epoch seconds, UTC. Durations are milliseconds.
- Every foreign key column leads an index. Indexes are named `<table>_<columns>_index`.

## Tables

| Table | Holds |
|---|---|
| `files` | Registered files: `path` (relative to the database's folder), `type` (`music`, `image` or `other`), `size_bytes`, `mtime`, `sha256` |
| `artists` | `name`, its language `name_language`, and `picture` → `files` |
| `labels` | `name`, its language `name_language`, and `picture` → `files` (the logo) |
| `artist_alternate_names` | Other names an artist goes by, with `position`: other spellings ("ALIKA"), initials, names in other scripts ("福居良"). Each has a `language`, a `note`, and `real_name` (0 or 1) for a legal or birth name. |
| `label_alternate_names` | Other names a label goes by, built the same way, without `real_name` |
| `releases` | `title` and its language `title_language`, `release_date` with `release_date_precision`, `catalog_number`, `cover_picture` → `files`, `various_artists` |
| `tracks` | `title` and its language `title_language` (NULL: the release's), `duration_ms`, `track_number`, `cd_number` (default 1), `release_id`, and `file_id` (its audio) |
| `release_disc_titles` | A disc's own title, as printed: `release_id`, `cd_number`, `title` and its `language` (NULL: the release's). Only discs with a title have a row. |
| `release_alternate_titles`, `track_alternate_titles` | Other titles a release or track goes by, with `position`: printed in another script or language ("シーナリィ"), or added in Slunk, like a translation. Each has a `language`, and `printed` (0 or 1, default 1). |
| `release_disc_alternate_titles` | A disc's other titles, built the same way. They belong to its title: `disc_title_id` → `release_disc_titles` |
| `artists_releases` | Who a release is by, with `position`, and `credited_name_id` (below) |
| `artists_tracks` | Who performs a track, with `featured`, `position` and `credited_name_id` |
| `labels_releases` | A release's labels, with `position` |
| `release_credits`, `track_credits` | Credit lines: a `role` as printed on a release or track ("Mixed By"), with `position`, the line's order |
| `artists_release_credits`, `artists_track_credits` | The artists on each credit line, with `position`, their order on the line, from 0, and `credited_name_id` |
| `release_formats`, `release_media` | Lists the user manages |
| `release_formats_releases`, `release_media_releases` | A release's formats and media |
| `files_releases`, `artists_files`, `files_tracks` | File links, each with an optional `note` |
| `text_tags`, `real_number_tags`, `date_tags` | Tag definitions: `name`, and `multiple` (0 for single-value) |
| `release_text_tags`, `release_real_number_tags`, `release_date_tags` | A release's tag values. `value` is `TEXT`, `REAL` or `INTEGER` (epoch seconds) |
| `track_text_tags`, `track_real_number_tags`, `track_date_tags` | A track's tag values, built like the release ones, and using the same tag definitions |
| `external_libraries` | External libraries ([below](#external-libraries)): a `uuid` its destination's manifest names it by, a `name`, a `kind` (`folder` or `ipod`), a folder library's `folder` and `path_template`, its profile (`format`, `aac_kbps`, `cd_resolution`, `embed_covers`, `cover_px`, `cover_file`), and `last_synced_at` |
| `external_library_rules` | What's in each: a rule names a release, an artist, a track or a tag value, and `exclude` leaves a release or a track out |
| `migrations` | Applied migrations (`name`, `applied_at`), kept by the migration runner |
| `sqlite_sequence` | SQLite's own: the `AUTOINCREMENT` counters |

## What the database enforces

### Values

- **Files:**
  - `path` is unique, non-empty, and doesn't start with `/`.
  - `type` is `music`, `image` or `other`.
  - `size_bytes >= 0`.
  - `sha256` is 64 lowercase hex characters.
- **Text:** these values are non-empty, with no leading or trailing spaces:
  - names: of artists, labels, formats, media and tag definitions;
  - titles: of releases, tracks and discs;
  - alternate names, and their notes;
  - alternate titles;
  - credit lines' `role`, `catalog_number`, `release_text_tags.value`, `track_text_tags.value` and file link notes.

  A nullable one (`catalog_number`, `note`) may also be NULL.
- **Languages:** `name_language`, `title_language` and `language` are NULL (unknown, or for a track or disc title, the release's), or a BCP 47 tag in canonical shape: a 2- or 3-letter lowercase language, then letters, digits and single hyphens, not ending in one, like `ja` or `ja-Latn`. `und` isn't allowed: NULL is what unknown means.
- **Alternate names compare exactly,** case included, so "ALIKA" and "Alika" are different names. `(artist_id, name)` and `(label_id, name)` are unique. `real_name` is 0 or 1.
- **Alternate titles compare exactly** too. `(release_id, title)`, `(track_id, title)` and `(disc_title_id, title)` are unique. `printed` is 0 or 1.
- **Unique ignoring case** (`COLLATE NOCASE`):
  - names of formats, media and tag definitions;
  - a credit line's role, per release or track;
  - a text tag's value per release or track, and tag.

  `catalog_number` also compares ignoring case, but isn't unique. Artist names, label names and release titles aren't unique; their indexes ignore case, for search and sorting.
- **Release dates:**
  - `release_date` and `release_date_precision` are both set or both NULL (`releases_release_date_precision_set`);
  - the precision is `year`, `month` or `day`;
  - the date is midnight UTC, on the 1st for `month`, and on January 1st for `year` (`releases_release_date_matches_precision`).
- **Tracks:**
  - `(release_id, cd_number, track_number)` is unique;
  - `file_id` is unique: a file is the audio of at most one track;
  - `track_number > 0`, `cd_number > 0`, `duration_ms >= 0`.
- **Disc titles:** `(release_id, cd_number)` is unique, so a disc has one title, and `cd_number > 0`. A disc's alternate titles hang off its title, so it needs a title to have them, and when its last track leaves it, its title goes, and they go with it (CASCADE).
- **Links:** an artist, label, format, medium or file appears at most once per release or track. An artist appears once per credit line, and two artists on a line can't share a `position`. `position >= 0`.
- **Credited names:** see [Credited names](#credited-names).
- **Tag values**, on releases and tracks alike:
  - `(release_id, tag_id, value)` and `(track_id, tag_id, value)` are unique;
  - a date tag's value is a day, at midnight UTC, and a number tag's value is finite (triggers below).
- **Flags:** `various_artists`, `featured` and `multiple` are 0 or 1.

### Triggers

Each trigger aborts the statement with the error shown.

| Triggers | Error | Fires when |
|---|---|---|
| `release_<type>_tags_single_value_insert`, `…_update` | `single-value tag already set on this release` | A release would get a second value for a tag with `multiple = 0`: on insert, or on changing `release_id` or `tag_id`. Changing `value` always works. |
| `track_<type>_tags_single_value_insert`, `…_update` | `single-value tag already set on this track` | The same, for a track |
| `<type>_tags_multiple_off` | `some releases or tracks have several values for this tag` | `multiple` changes from 1 to 0 while a release or a track has several values |
| `tracks_file_is_music_insert`, `…_update` | `a track's file must be music` | A track's `file_id` points to a file that isn't `music` |
| `files_type_stays_music` | `a track's file must stay music` | A track's file changes `type` |
| `releases_cover_picture_is_image_insert`, `…_update` | `a release cover must be an image` | `cover_picture` points to a file that isn't an `image` |
| `artists_picture_is_image_insert`, `…_update` | `an artist picture must be an image` | `picture` points to a file that isn't an `image` |
| `labels_picture_is_image_insert`, `…_update` | `a label picture must be an image` | `picture` points to a file that isn't an `image` |
| `files_type_stays_image` | `a cover or picture must stay an image` | A file used as a cover or picture changes `type` |
| `<owner>_date_tags_value_is_day_insert`, `…_update` | `a date tag's value must be a day, at midnight UTC` | `value % 86400 <> 0`, on insert or on changing `value` |
| `<owner>_real_number_tags_value_is_finite_insert`, `…_update` | `a number tag's value must be finite` | `value` is infinite, on insert or on changing `value`. NaN is already NULL, which `NOT NULL` rejects. |
| `release_disc_titles_disc_has_tracks_insert`, `…_update` | `a disc title needs a track on its disc` | A title is inserted, or moved to another `release_id` or `cd_number`, where the release has no track on that disc |

`<type>` is `text`, `real_number` or `date`, and `<owner>` is `release` or `track`.

### External libraries

An external library is a selection of the library, for a folder or an iPod ([4_external_libraries.md](../design/4_external_libraries.md)). What's on each destination isn't in the database: it's the destination's own manifest.
- **`external_libraries`** has `INTEGER PRIMARY KEY AUTOINCREMENT`, `inserted_at` and `updated_at`, as the main tables do.
  - `uuid` is unique, in lowercase hex with its four hyphens, as UUIDs are written, and `name` is unique ignoring case, non-empty and trimmed.
  - `kind` is `folder` or `ipod`, and `format` is `original`, `flac`, `alac` or `aac`. `aac_kbps` is 128 to 320 (256 by default), and `cover_px` 100 to 3000 (600 by default). `cd_resolution`, `embed_covers` and `cover_file` are 0 or 1.
  - Each kind is checked (`external_libraries_kind_has_its_destination`): a folder library has a `folder`, an absolute path, and a `path_template`; an iPod has neither, and takes only what it plays: `alac` or `aac`, `cd_resolution` 1, and `embed_covers` and `cover_file` 0.
  - `folder` is unique, so a folder belongs to one external library.
- **`external_library_rules`** belong to their external library (CASCADE), with `exclude` 0 or 1.
  - A rule names exactly one thing (`external_library_rules_name_one_thing`): `release_id`, `artist_id`, `track_id`, or a tag value's tag, `text_tag_id`, `real_number_tag_id` or `date_tag_id`, one column per type, since each type has its own table.
  - A tag value rule has its filter, as typed, and only it has one (`external_library_rules_tag_value_has_its_filter`): a text tag's `tag_equals`, which compares ignoring case; a number or date tag's `tag_equals`, or a range, `tag_min` and `tag_max`, one of them or both. The app checks that they read as the tag's type.
  - Only a release or a track can be left out (`external_library_rules_exclude_a_release_or_track`).
  - Partial unique indexes keep each thing named once per external library: a release, an artist or a track, whether it's in or left out; a text tag's value, ignoring case; and a number or date tag's filter.
  - Deleting a release, an artist, a track or a tag deletes its rules (CASCADE).

Two more triggers don't abort anything. `tracks_empty_disc_loses_title_delete` and `…_update` delete a disc's title when its last track leaves it: see [Disc titles](#disc-titles).

### Disc titles

A disc isn't a table: it's the `cd_number` its tracks share. It exists while it has tracks, and a title belongs to it:
- A title needs a track on its disc. A title for a disc with no tracks fails with `a disc title needs a track on its disc`.
- When a disc's last track leaves it, its title is deleted, by `tracks_empty_disc_loses_title_delete` and `…_update`. A track leaves its disc when it's deleted, or when its `cd_number` or `release_id` changes.
- So the app saves a release's tracks first, then its disc titles.

### Credit lines

Every credit line has an artist. A line is inserted before its artists, so a deferred foreign key checks it at commit:
- `release_credits` and `track_credits` have `first_artist_position`, a generated column that's always 0.
- `(id, first_artist_position)` references the artist table's `(credit_id, position)`, `DEFERRABLE INITIALLY DEFERRED`. So the line has an artist at position 0, and `(credit_id, position)` is unique.
- A transaction that leaves a line without an artist at position 0 fails at commit with `FOREIGN KEY constraint failed`: a line inserted with no artists, a line whose artists were all removed, or one whose artists start at 1.
- Outside a transaction, each statement commits on its own, so a line can only be added in a transaction, together with its artists.
- Deleting a line, or its release or track, deletes its artists too.
- Migrations run with foreign keys off, and the runner's `foreign_key_check` reports a line without artists.

### Credited names

The four artist link tables, `artists_releases`, `artists_tracks`, `artists_release_credits` and `artists_track_credits`, each have `credited_name_id`: the alternate name printed there, like "ALIKA" for Alika. NULL is the artist's main name.
- `(artist_id, credited_name_id)` references `artist_alternate_names (artist_id, id)`, so the name is the linked artist's own. SQLite doesn't check a key with a NULL in it. Otherwise the database refuses, with `FOREIGN KEY constraint failed`:
  - crediting an artist with another artist's name;
  - moving a credited name to another artist;
  - changing a link's artist while keeping the old artist's name.
- A name a link is credited with can't be deleted (RESTRICT). `SET NULL` isn't possible: `artist_id` is part of the key, so it would be set to NULL too.
- `(artist_id, credited_name_id)` is indexed on each table, which also serves lookups by artist.

### Deletes

Foreign keys decide what a delete does.

- **RESTRICT:** the delete fails with `FOREIGN KEY constraint failed` while the row is in use:
  - a file used as a track's audio, a cover, a picture or in a file link;
  - a release with tracks;
  - an artist who is a release or track artist, or is credited;
  - an artist's alternate name that a link is credited with;
  - a label on a release;
  - a format, medium or tag definition in use (a tag is in use on a release or a track).
- **CASCADE:** deleting a row also deletes the link rows that describe it, and the external library rules that name it:
  - a release: its artists, labels, formats, media, credit lines, disc titles, alternate titles, file links and tag values;
  - a track: its artists, credit lines, alternate titles, file links and tag values;
  - a disc title: its alternate titles;
  - a credit line: its artists;
  - an artist: its alternate names and file links;
  - a label: its alternate names;
  - a tag definition: its external library rules;
  - an external library: its rules.

## Starter rows

The baseline migration adds these rows. Nothing depends on them, so the user can rename or delete them.

- `release_formats`: Album, Compilation, EP, Live, LP, Mixtape, Single, Soundtrack
- `release_media`: Cassette, CD, Digital, Vinyl

## Left to the app

- A tag name used by another type ("Score" as both a text and a real number tag) is allowed by the database. The app refuses it, ignoring case.
- Paths are only checked to be non-empty and relative. Normalizing them (forward slashes, no `./` or `a/../b`) is up to the app.
- `updated_at` isn't updated automatically.
- An alternate name can't be the same as its artist's or label's main name, except an artist's real name. An alternate title can't be the same as its main title.
- Text is in Unicode NFC. SQLite can't normalize it, so the app does it before saving.
- A language tag's canonical form, beyond its shape.

## How it's checked

The migrations' tests, in `crates/slunk/`, check slunk's eight migrations against slunk's own, with fixtures that slunk's migration runner made ([development/backend.md](../development/backend.md#parity-with-slunk)):
- **A new database** has slunk's `sqlite_schema`, row for row, and the same starter rows.
- **Upgrades:** from each state between migrations, with rows in every table, the Rust runner ends where slunk's does, rows and `AUTOINCREMENT` counters included.
- **Rollbacks** end where slunk's do, and every `down`, liquidslunk's own too, undoes its `up`. One `down` doesn't give back the exact text it found: `20261014052413_add_track_tags` recreates the `<type>_tags_multiple_off` triggers with the baseline's logic, but laid out differently, as slunk's did. So that check compares the SQL without its whitespace and parentheses.

`20261020045756_add_external_libraries` has no slunk version to check against. Its rules are tested in `tests/external_libraries.rs`, and its `down` with the others.
