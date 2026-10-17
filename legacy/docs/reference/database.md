> **Legacy:** this is slunk's documentation, copied for the port. It describes `legacy/slunk-be/` and slunk's frontend before the port, not liquidslunk. Its citations ("design §4.3") mean slunk's design docs, in `legacy/docs/design/`. It's read-only, and phase 6 deletes it.

# Database

> Keep this page up to date: any migration that changes the schema updates it in the same change.

This is the schema as built, by these migrations in `slunk-be/priv/repo/migrations/`:

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

The reasoning behind it is in [design §4](../design/1_introduction.md#4-data-model), and its appendices [A](../design/1_introduction.md#appendix-a-more-integrity-rules-in-the-database) and [D](../design/1_introduction.md#appendix-d-tag-values-are-checked-in-the-database). Credits follow [2_credits.md §4](../design/2_credits.md#4-data-model) instead, and alternate names [3_alternate_names.md §4](../design/3_alternate_names.md#4-data-model). Disc titles have no design doc: [Disc titles](#disc-titles) below explains them. Every column's type is in `slunk-be/src/repo/db_types.ts`, which is generated from the database.

How to connect, migrate and seed: [development/backend.md](../development/backend.md#the-database).

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
| `migrations` | Applied migrations (`name`, `applied_at`), kept by the migration runner |

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

The four artist link tables, `artists_releases`, `artists_tracks`, `artists_release_credits` and `artists_track_credits`, each have `credited_name_id`: the alternate name printed there, like "ALIKA" for Alika ([3_alternate_names.md §4.2](../design/3_alternate_names.md#42-design-decisions)). NULL is the artist's main name.
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
- **CASCADE:** deleting a row also deletes the link rows that describe it:
  - a release: its artists, labels, formats, media, credit lines, disc titles, alternate titles, file links and tag values;
  - a track: its artists, credit lines, alternate titles, file links and tag values;
  - a disc title: its alternate titles;
  - a credit line: its artists;
  - an artist: its alternate names and file links;
  - a label: its alternate names.

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
