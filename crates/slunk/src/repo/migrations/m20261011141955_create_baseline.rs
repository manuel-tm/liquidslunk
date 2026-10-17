//! The baseline schema: every table, index and trigger, except those the
//! later migrations add, and the starter rows: credit roles, release
//! formats and release media. Nothing may depend on the starter rows.

use rusqlite::Transaction;

use super::run;
use crate::Result;

pub fn up(tx: &Transaction) -> Result<()> {
    run(tx, UP)
}

pub fn down(tx: &Transaction) -> Result<()> {
    run(tx, DOWN)
}

const UP: &[&str] = &[
    r#"create table "files" ("id" integer not null primary key autoincrement, "path" text not null check (path <> '' AND substr(path, 1, 1) <> '/'), "type" text not null check (type IN ('music', 'image', 'other')), "size_bytes" integer not null check (size_bytes >= 0), "mtime" integer not null, "sha256" text not null check (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'), "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null) strict"#,
    r#"create unique index "files_path_index" on "files" ("path")"#,
    r#"create index "files_sha256_index" on "files" ("sha256")"#,
    r#"create table "artists" ("id" integer not null primary key autoincrement, "name" text not null check ("name" <> '' AND "name" = trim("name")), "picture" integer references "files" ("id") on delete restrict, "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null) strict"#,
    r#"create index "artists_name_index" on "artists" ("name" COLLATE NOCASE)"#,
    r#"create index "artists_picture_index" on "artists" ("picture")"#,
    r#"create table "labels" ("id" integer not null primary key autoincrement, "name" text not null check ("name" <> '' AND "name" = trim("name")), "picture" integer references "files" ("id") on delete restrict, "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null) strict"#,
    r#"create index "labels_name_index" on "labels" ("name" COLLATE NOCASE)"#,
    r#"create index "labels_picture_index" on "labels" ("picture")"#,
    r#"create table "releases" ("id" integer not null primary key autoincrement, "title" text not null check ("title" <> '' AND "title" = trim("title")), "release_date" integer, "release_date_precision" text check (release_date_precision IN ('year', 'month', 'day')), "catalog_number" text check ("catalog_number" <> '' AND "catalog_number" = trim("catalog_number")) COLLATE NOCASE, "cover_picture" integer references "files" ("id") on delete restrict, "various_artists" integer default 0 not null check (various_artists IN (0, 1)), "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null, constraint "releases_release_date_precision_set" check ((release_date IS NULL) = (release_date_precision IS NULL)), constraint "releases_release_date_matches_precision" check (release_date IS NULL OR (
        release_date % 86400 = 0
        AND (release_date_precision <> 'month'
             OR strftime('%d', release_date, 'unixepoch') = '01')
        AND (release_date_precision <> 'year'
             OR strftime('%m-%d', release_date, 'unixepoch') = '01-01')
      ))) strict"#,
    r#"create index "releases_title_index" on "releases" ("title" COLLATE NOCASE)"#,
    r#"create index "releases_release_date_index" on "releases" ("release_date")"#,
    r#"create index "releases_catalog_number_index" on "releases" ("catalog_number")"#,
    r#"create index "releases_cover_picture_index" on "releases" ("cover_picture")"#,
    r#"create table "tracks" ("id" integer not null primary key autoincrement, "title" text not null check ("title" <> '' AND "title" = trim("title")), "duration_ms" integer check (duration_ms >= 0), "track_number" integer not null check (track_number > 0), "cd_number" integer default 1 not null check (cd_number > 0), "release_id" integer not null references "releases" ("id") on delete restrict, "file_id" integer not null references "files" ("id") on delete restrict, "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null) strict"#,
    r#"create unique index "tracks_release_id_cd_number_track_number_index" on "tracks" ("release_id", "cd_number", "track_number")"#,
    r#"create unique index "tracks_file_id_index" on "tracks" ("file_id")"#,
    r#"create table "artists_releases" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "release_id" integer not null references "releases" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "artists_releases_release_id_artist_id_index" on "artists_releases" ("release_id", "artist_id")"#,
    r#"create index "artists_releases_artist_id_index" on "artists_releases" ("artist_id")"#,
    r#"create table "artists_tracks" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "track_id" integer not null references "tracks" ("id") on delete cascade, "featured" integer default 0 not null check (featured IN (0, 1)), "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "artists_tracks_track_id_artist_id_index" on "artists_tracks" ("track_id", "artist_id")"#,
    r#"create index "artists_tracks_artist_id_index" on "artists_tracks" ("artist_id")"#,
    r#"create table "labels_releases" ("id" integer not null primary key, "label_id" integer not null references "labels" ("id") on delete restrict, "release_id" integer not null references "releases" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "labels_releases_release_id_label_id_index" on "labels_releases" ("release_id", "label_id")"#,
    r#"create index "labels_releases_label_id_index" on "labels_releases" ("label_id")"#,
    r#"create table "credit_roles" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE) strict"#,
    r#"create unique index "credit_roles_name_index" on "credit_roles" ("name")"#,
    r#"create table "release_credits" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "artist_id" integer not null references "artists" ("id") on delete restrict, "role_id" integer not null references "credit_roles" ("id") on delete restrict, "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "release_credits_release_id_artist_id_role_id_index" on "release_credits" ("release_id", "artist_id", "role_id")"#,
    r#"create index "release_credits_artist_id_index" on "release_credits" ("artist_id")"#,
    r#"create index "release_credits_role_id_index" on "release_credits" ("role_id")"#,
    r#"create table "track_credits" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "artist_id" integer not null references "artists" ("id") on delete restrict, "role_id" integer not null references "credit_roles" ("id") on delete restrict, "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "track_credits_track_id_artist_id_role_id_index" on "track_credits" ("track_id", "artist_id", "role_id")"#,
    r#"create index "track_credits_artist_id_index" on "track_credits" ("artist_id")"#,
    r#"create index "track_credits_role_id_index" on "track_credits" ("role_id")"#,
    r#"create table "release_formats" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE) strict"#,
    r#"create unique index "release_formats_name_index" on "release_formats" ("name")"#,
    r#"create table "release_formats_releases" ("id" integer not null primary key, "format_id" integer not null references "release_formats" ("id") on delete restrict, "release_id" integer not null references "releases" ("id") on delete cascade) strict"#,
    r#"create unique index "release_formats_releases_release_id_format_id_index" on "release_formats_releases" ("release_id", "format_id")"#,
    r#"create index "release_formats_releases_format_id_index" on "release_formats_releases" ("format_id")"#,
    r#"create table "release_media" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE) strict"#,
    r#"create unique index "release_media_name_index" on "release_media" ("name")"#,
    r#"create table "release_media_releases" ("id" integer not null primary key, "medium_id" integer not null references "release_media" ("id") on delete restrict, "release_id" integer not null references "releases" ("id") on delete cascade) strict"#,
    r#"create unique index "release_media_releases_release_id_medium_id_index" on "release_media_releases" ("release_id", "medium_id")"#,
    r#"create index "release_media_releases_medium_id_index" on "release_media_releases" ("medium_id")"#,
    r#"create table "files_releases" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "file_id" integer not null references "files" ("id") on delete restrict, "note" text check ("note" <> '' AND "note" = trim("note"))) strict"#,
    r#"create unique index "files_releases_release_id_file_id_index" on "files_releases" ("release_id", "file_id")"#,
    r#"create index "files_releases_file_id_index" on "files_releases" ("file_id")"#,
    r#"create table "artists_files" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete cascade, "file_id" integer not null references "files" ("id") on delete restrict, "note" text check ("note" <> '' AND "note" = trim("note"))) strict"#,
    r#"create unique index "artists_files_artist_id_file_id_index" on "artists_files" ("artist_id", "file_id")"#,
    r#"create index "artists_files_file_id_index" on "artists_files" ("file_id")"#,
    r#"create table "files_tracks" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "file_id" integer not null references "files" ("id") on delete restrict, "note" text check ("note" <> '' AND "note" = trim("note"))) strict"#,
    r#"create unique index "files_tracks_track_id_file_id_index" on "files_tracks" ("track_id", "file_id")"#,
    r#"create index "files_tracks_file_id_index" on "files_tracks" ("file_id")"#,
    r#"create table "text_tags" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE, "multiple" integer default 0 not null check (multiple IN (0, 1))) strict"#,
    r#"create unique index "text_tags_name_index" on "text_tags" ("name")"#,
    r#"create table "release_text_tags" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "tag_id" integer not null references "text_tags" ("id") on delete restrict, "value" text not null check ("value" <> '' AND "value" = trim("value")) COLLATE NOCASE) strict"#,
    r#"create unique index "release_text_tags_release_id_tag_id_value_index" on "release_text_tags" ("release_id", "tag_id", "value")"#,
    r#"create index "release_text_tags_tag_id_value_index" on "release_text_tags" ("tag_id", "value")"#,
    r#"CREATE TRIGGER release_text_tags_single_value_insert
        BEFORE INSERT ON release_text_tags
        WHEN (SELECT multiple FROM text_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_text_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END"#,
    r#"CREATE TRIGGER release_text_tags_single_value_update
        BEFORE UPDATE OF release_id, tag_id ON release_text_tags
        WHEN (SELECT multiple FROM text_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_text_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END"#,
    r#"CREATE TRIGGER text_tags_multiple_off
        BEFORE UPDATE OF multiple ON text_tags
        WHEN OLD.multiple = 1 AND NEW.multiple = 0
          AND EXISTS (SELECT 1 FROM release_text_tags WHERE tag_id = NEW.id
                      GROUP BY release_id HAVING count(*) > 1)
        BEGIN
          SELECT RAISE(ABORT, 'some releases have several values for this tag');
        END"#,
    r#"create table "real_number_tags" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE, "multiple" integer default 0 not null check (multiple IN (0, 1))) strict"#,
    r#"create unique index "real_number_tags_name_index" on "real_number_tags" ("name")"#,
    r#"create table "release_real_number_tags" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "tag_id" integer not null references "real_number_tags" ("id") on delete restrict, "value" real not null) strict"#,
    r#"create unique index "release_real_number_tags_release_id_tag_id_value_index" on "release_real_number_tags" ("release_id", "tag_id", "value")"#,
    r#"create index "release_real_number_tags_tag_id_value_index" on "release_real_number_tags" ("tag_id", "value")"#,
    r#"CREATE TRIGGER release_real_number_tags_single_value_insert
        BEFORE INSERT ON release_real_number_tags
        WHEN (SELECT multiple FROM real_number_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_real_number_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END"#,
    r#"CREATE TRIGGER release_real_number_tags_single_value_update
        BEFORE UPDATE OF release_id, tag_id ON release_real_number_tags
        WHEN (SELECT multiple FROM real_number_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_real_number_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END"#,
    r#"CREATE TRIGGER real_number_tags_multiple_off
        BEFORE UPDATE OF multiple ON real_number_tags
        WHEN OLD.multiple = 1 AND NEW.multiple = 0
          AND EXISTS (SELECT 1 FROM release_real_number_tags WHERE tag_id = NEW.id
                      GROUP BY release_id HAVING count(*) > 1)
        BEGIN
          SELECT RAISE(ABORT, 'some releases have several values for this tag');
        END"#,
    r#"create table "date_tags" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE, "multiple" integer default 0 not null check (multiple IN (0, 1))) strict"#,
    r#"create unique index "date_tags_name_index" on "date_tags" ("name")"#,
    r#"create table "release_date_tags" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "tag_id" integer not null references "date_tags" ("id") on delete restrict, "value" integer not null) strict"#,
    r#"create unique index "release_date_tags_release_id_tag_id_value_index" on "release_date_tags" ("release_id", "tag_id", "value")"#,
    r#"create index "release_date_tags_tag_id_value_index" on "release_date_tags" ("tag_id", "value")"#,
    r#"CREATE TRIGGER release_date_tags_single_value_insert
        BEFORE INSERT ON release_date_tags
        WHEN (SELECT multiple FROM date_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_date_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END"#,
    r#"CREATE TRIGGER release_date_tags_single_value_update
        BEFORE UPDATE OF release_id, tag_id ON release_date_tags
        WHEN (SELECT multiple FROM date_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_date_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END"#,
    r#"CREATE TRIGGER date_tags_multiple_off
        BEFORE UPDATE OF multiple ON date_tags
        WHEN OLD.multiple = 1 AND NEW.multiple = 0
          AND EXISTS (SELECT 1 FROM release_date_tags WHERE tag_id = NEW.id
                      GROUP BY release_id HAVING count(*) > 1)
        BEGIN
          SELECT RAISE(ABORT, 'some releases have several values for this tag');
        END"#,
    r#"CREATE TRIGGER tracks_file_is_music_insert
    BEFORE INSERT ON tracks
    WHEN (SELECT type FROM files WHERE id = NEW.file_id) <> 'music'
    BEGIN
      SELECT RAISE(ABORT, 'a track''s file must be music');
    END"#,
    r#"CREATE TRIGGER tracks_file_is_music_update
    BEFORE UPDATE OF file_id ON tracks
    WHEN (SELECT type FROM files WHERE id = NEW.file_id) <> 'music'
    BEGIN
      SELECT RAISE(ABORT, 'a track''s file must be music');
    END"#,
    r#"CREATE TRIGGER files_type_stays_music
    BEFORE UPDATE OF type ON files
    WHEN NEW.type <> 'music'
      AND EXISTS (SELECT 1 FROM tracks WHERE file_id = NEW.id)
    BEGIN
      SELECT RAISE(ABORT, 'a track''s file must stay music');
    END"#,
    r#"CREATE TRIGGER releases_cover_picture_is_image_insert
          BEFORE INSERT ON releases
          WHEN (SELECT type FROM files WHERE id = NEW.cover_picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'a release cover must be an image');
          END"#,
    r#"CREATE TRIGGER releases_cover_picture_is_image_update
          BEFORE UPDATE OF cover_picture ON releases
          WHEN (SELECT type FROM files WHERE id = NEW.cover_picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'a release cover must be an image');
          END"#,
    r#"CREATE TRIGGER artists_picture_is_image_insert
          BEFORE INSERT ON artists
          WHEN (SELECT type FROM files WHERE id = NEW.picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'an artist picture must be an image');
          END"#,
    r#"CREATE TRIGGER artists_picture_is_image_update
          BEFORE UPDATE OF picture ON artists
          WHEN (SELECT type FROM files WHERE id = NEW.picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'an artist picture must be an image');
          END"#,
    r#"CREATE TRIGGER labels_picture_is_image_insert
          BEFORE INSERT ON labels
          WHEN (SELECT type FROM files WHERE id = NEW.picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'a label picture must be an image');
          END"#,
    r#"CREATE TRIGGER labels_picture_is_image_update
          BEFORE UPDATE OF picture ON labels
          WHEN (SELECT type FROM files WHERE id = NEW.picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'a label picture must be an image');
          END"#,
    r#"CREATE TRIGGER files_type_stays_image
    BEFORE UPDATE OF type ON files
    WHEN NEW.type <> 'image'
      AND (EXISTS (SELECT 1 FROM releases WHERE cover_picture = NEW.id)
           OR EXISTS (SELECT 1 FROM artists WHERE picture = NEW.id)
           OR EXISTS (SELECT 1 FROM labels WHERE picture = NEW.id))
    BEGIN
      SELECT RAISE(ABORT, 'a cover or picture must stay an image');
    END"#,
    r#"INSERT INTO credit_roles (name)
    VALUES ('composer'), ('conductor'), ('producer'), ('remixer'), ('writer')"#,
    r#"INSERT INTO release_formats (name)
    VALUES ('Album'), ('Compilation'), ('EP'), ('Live'), ('LP'), ('Mixtape'),
           ('Single'), ('Soundtrack')"#,
    r#"INSERT INTO release_media (name)
    VALUES ('Cassette'), ('CD'), ('Digital'), ('Vinyl')"#,
];

const DOWN: &[&str] = &[
    r#"drop table "release_date_tags""#,
    r#"drop table "date_tags""#,
    r#"drop table "release_real_number_tags""#,
    r#"drop table "real_number_tags""#,
    r#"drop table "release_text_tags""#,
    r#"drop table "text_tags""#,
    r#"drop table "files_tracks""#,
    r#"drop table "artists_files""#,
    r#"drop table "files_releases""#,
    r#"drop table "release_media_releases""#,
    r#"drop table "release_media""#,
    r#"drop table "release_formats_releases""#,
    r#"drop table "release_formats""#,
    r#"drop table "track_credits""#,
    r#"drop table "release_credits""#,
    r#"drop table "credit_roles""#,
    r#"drop table "labels_releases""#,
    r#"drop table "artists_tracks""#,
    r#"drop table "artists_releases""#,
    r#"drop table "tracks""#,
    r#"drop table "releases""#,
    r#"drop table "labels""#,
    r#"drop table "artists""#,
    r#"drop table "files""#,
];
