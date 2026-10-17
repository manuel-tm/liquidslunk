//! Alternate titles: the other titles a release, a track or a disc goes
//! by, each with a language and whether it's printed, and the language of
//! their main titles. Titles compare exactly, case included. A disc's
//! alternate titles belong to its title, so they go when it goes.

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
    r#"create table "release_alternate_titles" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "title" text not null check ("title" <> '' AND "title" = trim("title")), "language" text check (("language" GLOB '[a-z][a-z]' OR "language" GLOB '[a-z][a-z][a-z]'
    OR "language" GLOB '[a-z][a-z]-?*' OR "language" GLOB '[a-z][a-z][a-z]-?*')
    AND "language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "language" NOT GLOB '*--*' AND "language" NOT GLOB '*-'
    AND "language" <> 'und' AND "language" NOT GLOB 'und-*'), "printed" integer default 1 not null check (printed IN (0, 1)), "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "release_alternate_titles_release_id_title_index" on "release_alternate_titles" ("release_id", "title")"#,
    r#"create table "track_alternate_titles" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "title" text not null check ("title" <> '' AND "title" = trim("title")), "language" text check (("language" GLOB '[a-z][a-z]' OR "language" GLOB '[a-z][a-z][a-z]'
    OR "language" GLOB '[a-z][a-z]-?*' OR "language" GLOB '[a-z][a-z][a-z]-?*')
    AND "language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "language" NOT GLOB '*--*' AND "language" NOT GLOB '*-'
    AND "language" <> 'und' AND "language" NOT GLOB 'und-*'), "printed" integer default 1 not null check (printed IN (0, 1)), "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "track_alternate_titles_track_id_title_index" on "track_alternate_titles" ("track_id", "title")"#,
    r#"create table "release_disc_alternate_titles" ("id" integer not null primary key, "disc_title_id" integer not null references "release_disc_titles" ("id") on delete cascade, "title" text not null check ("title" <> '' AND "title" = trim("title")), "language" text check (("language" GLOB '[a-z][a-z]' OR "language" GLOB '[a-z][a-z][a-z]'
    OR "language" GLOB '[a-z][a-z]-?*' OR "language" GLOB '[a-z][a-z][a-z]-?*')
    AND "language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "language" NOT GLOB '*--*' AND "language" NOT GLOB '*-'
    AND "language" <> 'und' AND "language" NOT GLOB 'und-*'), "printed" integer default 1 not null check (printed IN (0, 1)), "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "release_disc_alternate_titles_disc_title_id_title_index" on "release_disc_alternate_titles" ("disc_title_id", "title")"#,
    r#"alter table "releases" add column "title_language" text check (("title_language" GLOB '[a-z][a-z]' OR "title_language" GLOB '[a-z][a-z][a-z]'
    OR "title_language" GLOB '[a-z][a-z]-?*' OR "title_language" GLOB '[a-z][a-z][a-z]-?*')
    AND "title_language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "title_language" NOT GLOB '*--*' AND "title_language" NOT GLOB '*-'
    AND "title_language" <> 'und' AND "title_language" NOT GLOB 'und-*')"#,
    r#"alter table "tracks" add column "title_language" text check (("title_language" GLOB '[a-z][a-z]' OR "title_language" GLOB '[a-z][a-z][a-z]'
    OR "title_language" GLOB '[a-z][a-z]-?*' OR "title_language" GLOB '[a-z][a-z][a-z]-?*')
    AND "title_language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "title_language" NOT GLOB '*--*' AND "title_language" NOT GLOB '*-'
    AND "title_language" <> 'und' AND "title_language" NOT GLOB 'und-*')"#,
    r#"alter table "release_disc_titles" add column "language" text check (("language" GLOB '[a-z][a-z]' OR "language" GLOB '[a-z][a-z][a-z]'
    OR "language" GLOB '[a-z][a-z]-?*' OR "language" GLOB '[a-z][a-z][a-z]-?*')
    AND "language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "language" NOT GLOB '*--*' AND "language" NOT GLOB '*-'
    AND "language" <> 'und' AND "language" NOT GLOB 'und-*')"#,
];

const DOWN: &[&str] = &[
    r#"alter table "releases" drop column "title_language""#,
    r#"alter table "tracks" drop column "title_language""#,
    r#"alter table "release_disc_titles" drop column "language""#,
    r#"drop table "release_disc_alternate_titles""#,
    r#"drop table "track_alternate_titles""#,
    r#"drop table "release_alternate_titles""#,
];
