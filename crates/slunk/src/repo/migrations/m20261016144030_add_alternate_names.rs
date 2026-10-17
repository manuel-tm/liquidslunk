//! Alternate names: the other names an artist or label goes by, each with
//! a language, and the language of their main names. Names compare
//! exactly, case included, so "ALIKA" isn't "Alika".

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
    r#"create table "artist_alternate_names" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete cascade, "name" text not null check ("name" <> '' AND "name" = trim("name")), "language" text check (("language" GLOB '[a-z][a-z]' OR "language" GLOB '[a-z][a-z][a-z]'
    OR "language" GLOB '[a-z][a-z]-?*' OR "language" GLOB '[a-z][a-z][a-z]-?*')
    AND "language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "language" NOT GLOB '*--*' AND "language" NOT GLOB '*-'
    AND "language" <> 'und' AND "language" NOT GLOB 'und-*'), "real_name" integer default 0 not null check (real_name IN (0, 1)), "note" text check ("note" <> '' AND "note" = trim("note")), "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "artist_alternate_names_artist_id_name_index" on "artist_alternate_names" ("artist_id", "name")"#,
    r#"alter table "artists" add column "name_language" text check (("name_language" GLOB '[a-z][a-z]' OR "name_language" GLOB '[a-z][a-z][a-z]'
    OR "name_language" GLOB '[a-z][a-z]-?*' OR "name_language" GLOB '[a-z][a-z][a-z]-?*')
    AND "name_language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "name_language" NOT GLOB '*--*' AND "name_language" NOT GLOB '*-'
    AND "name_language" <> 'und' AND "name_language" NOT GLOB 'und-*')"#,
    r#"create table "label_alternate_names" ("id" integer not null primary key, "label_id" integer not null references "labels" ("id") on delete cascade, "name" text not null check ("name" <> '' AND "name" = trim("name")), "language" text check (("language" GLOB '[a-z][a-z]' OR "language" GLOB '[a-z][a-z][a-z]'
    OR "language" GLOB '[a-z][a-z]-?*' OR "language" GLOB '[a-z][a-z][a-z]-?*')
    AND "language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "language" NOT GLOB '*--*' AND "language" NOT GLOB '*-'
    AND "language" <> 'und' AND "language" NOT GLOB 'und-*'), "note" text check ("note" <> '' AND "note" = trim("note")), "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "label_alternate_names_label_id_name_index" on "label_alternate_names" ("label_id", "name")"#,
    r#"alter table "labels" add column "name_language" text check (("name_language" GLOB '[a-z][a-z]' OR "name_language" GLOB '[a-z][a-z][a-z]'
    OR "name_language" GLOB '[a-z][a-z]-?*' OR "name_language" GLOB '[a-z][a-z][a-z]-?*')
    AND "name_language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "name_language" NOT GLOB '*--*' AND "name_language" NOT GLOB '*-'
    AND "name_language" <> 'und' AND "name_language" NOT GLOB 'und-*')"#,
    r#"create unique index "artist_alternate_names_artist_id_id_index" on "artist_alternate_names" ("artist_id", "id")"#,
    r#"create index "artist_alternate_names_name_index" on "artist_alternate_names" (name COLLATE NOCASE)"#,
];

const DOWN: &[&str] = &[
    r#"alter table "artists" drop column "name_language""#,
    r#"drop table "artist_alternate_names""#,
    r#"alter table "labels" drop column "name_language""#,
    r#"drop table "label_alternate_names""#,
];
