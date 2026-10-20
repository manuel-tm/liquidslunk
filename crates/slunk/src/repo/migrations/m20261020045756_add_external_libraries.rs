//! External libraries (database.md, "External libraries"): a destination, a
//! folder or an iPod, with its profile, and its rules, which say what's in it.
//! What's on each destination is the destination's own manifest, not a table.
//! It's the first migration since slunk's eight, so slunk can't open a library
//! once it has run.
//!
//! - The database checks each kind: a folder library has its folder and its
//!   path template, and an iPod has neither, and takes only what it plays:
//!   Apple Lossless or AAC, at CD resolution, with no embedded covers or
//!   `cover.jpg`s.
//! - A folder belongs to one external library.
//! - A rule names exactly one thing: a release, an artist, a track, or a tag
//!   value, with its filter as typed. Only a release or a track can be left
//!   out. Each thing is named once per external library.
//! - Deleting a release, an artist, a track or a tag deletes its rules.

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
    r#"create table "external_libraries" ("id" integer not null primary key autoincrement, "uuid" text not null check ("uuid" GLOB '[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]'), "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE, "kind" text not null check ("kind" IN ('folder', 'ipod')), "folder" text check ("folder" GLOB '/*'), "format" text not null check ("format" IN ('original', 'flac', 'alac', 'aac')), "aac_kbps" integer default 256 not null check ("aac_kbps" BETWEEN 128 AND 320), "cd_resolution" integer default 0 not null check ("cd_resolution" IN (0, 1)), "path_template" text check ("path_template" <> '' AND "path_template" = trim("path_template")), "embed_covers" integer default 1 not null check ("embed_covers" IN (0, 1)), "cover_px" integer default 600 not null check ("cover_px" BETWEEN 100 AND 3000), "cover_file" integer default 0 not null check ("cover_file" IN (0, 1)), "last_synced_at" integer, "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null, constraint "external_libraries_kind_has_its_destination" check (("kind" = 'folder' AND "folder" IS NOT NULL AND "path_template" IS NOT NULL) OR ("kind" = 'ipod' AND "folder" IS NULL AND "path_template" IS NULL AND "format" IN ('alac', 'aac') AND "cd_resolution" = 1 AND "embed_covers" = 0 AND "cover_file" = 0))) strict"#,
    r#"create unique index "external_libraries_uuid_index" on "external_libraries" ("uuid")"#,
    r#"create unique index "external_libraries_name_index" on "external_libraries" ("name")"#,
    r#"create unique index "external_libraries_folder_index" on "external_libraries" ("folder")"#,
    r#"create table "external_library_rules" ("id" integer not null primary key, "external_library_id" integer not null references "external_libraries" ("id") on delete cascade, "exclude" integer default 0 not null check ("exclude" IN (0, 1)), "release_id" integer references "releases" ("id") on delete cascade, "artist_id" integer references "artists" ("id") on delete cascade, "track_id" integer references "tracks" ("id") on delete cascade, "text_tag_id" integer references "text_tags" ("id") on delete cascade, "real_number_tag_id" integer references "real_number_tags" ("id") on delete cascade, "date_tag_id" integer references "date_tags" ("id") on delete cascade, "tag_equals" text check ("tag_equals" <> '' AND "tag_equals" = trim("tag_equals")) COLLATE NOCASE, "tag_min" text check ("tag_min" <> '' AND "tag_min" = trim("tag_min")), "tag_max" text check ("tag_max" <> '' AND "tag_max" = trim("tag_max")), constraint "external_library_rules_name_one_thing" check (("release_id" IS NOT NULL) + ("artist_id" IS NOT NULL) + ("track_id" IS NOT NULL) + ("text_tag_id" IS NOT NULL) + ("real_number_tag_id" IS NOT NULL) + ("date_tag_id" IS NOT NULL) = 1), constraint "external_library_rules_tag_value_has_its_filter" check (CASE WHEN "text_tag_id" IS NOT NULL THEN "tag_equals" IS NOT NULL AND "tag_min" IS NULL AND "tag_max" IS NULL WHEN "real_number_tag_id" IS NOT NULL OR "date_tag_id" IS NOT NULL THEN ("tag_equals" IS NOT NULL) <> ("tag_min" IS NOT NULL OR "tag_max" IS NOT NULL) ELSE "tag_equals" IS NULL AND "tag_min" IS NULL AND "tag_max" IS NULL END), constraint "external_library_rules_exclude_a_release_or_track" check ("exclude" = 0 OR "release_id" IS NOT NULL OR "track_id" IS NOT NULL)) strict"#,
    r#"create index "external_library_rules_external_library_id_index" on "external_library_rules" ("external_library_id")"#,
    r#"create index "external_library_rules_release_id_index" on "external_library_rules" ("release_id")"#,
    r#"create index "external_library_rules_artist_id_index" on "external_library_rules" ("artist_id")"#,
    r#"create index "external_library_rules_track_id_index" on "external_library_rules" ("track_id")"#,
    r#"create index "external_library_rules_text_tag_id_index" on "external_library_rules" ("text_tag_id")"#,
    r#"create index "external_library_rules_real_number_tag_id_index" on "external_library_rules" ("real_number_tag_id")"#,
    r#"create index "external_library_rules_date_tag_id_index" on "external_library_rules" ("date_tag_id")"#,
    r#"create unique index "external_library_rules_external_library_id_release_id_index" on "external_library_rules" ("external_library_id", "release_id") where "release_id" IS NOT NULL"#,
    r#"create unique index "external_library_rules_external_library_id_artist_id_index" on "external_library_rules" ("external_library_id", "artist_id") where "artist_id" IS NOT NULL"#,
    r#"create unique index "external_library_rules_external_library_id_track_id_index" on "external_library_rules" ("external_library_id", "track_id") where "track_id" IS NOT NULL"#,
    r#"create unique index "external_library_rules_external_library_id_text_tag_id_tag_equals_index" on "external_library_rules" ("external_library_id", "text_tag_id", "tag_equals") where "text_tag_id" IS NOT NULL"#,
    r#"create unique index "external_library_rules_external_library_id_real_number_tag_id_tag_equals_tag_min_tag_max_index" on "external_library_rules" ("external_library_id", "real_number_tag_id", coalesce("tag_equals", ''), coalesce("tag_min", ''), coalesce("tag_max", '')) where "real_number_tag_id" IS NOT NULL"#,
    r#"create unique index "external_library_rules_external_library_id_date_tag_id_tag_equals_tag_min_tag_max_index" on "external_library_rules" ("external_library_id", "date_tag_id", coalesce("tag_equals", ''), coalesce("tag_min", ''), coalesce("tag_max", '')) where "date_tag_id" IS NOT NULL"#,
];

const DOWN: &[&str] = &[
    r#"drop table "external_library_rules""#,
    r#"drop table "external_libraries""#,
];
