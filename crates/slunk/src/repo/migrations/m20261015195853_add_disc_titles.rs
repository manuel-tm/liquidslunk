//! Disc titles: a release's disc can have a title of its own, as printed.
//! Only discs with a title have a row. A disc exists while it has tracks,
//! so a title needs a track on its disc, and goes when the disc's last
//! track leaves it.

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
    r#"create table "release_disc_titles" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "cd_number" integer not null check (cd_number > 0), "title" text not null check ("title" <> '' AND "title" = trim("title"))) strict"#,
    r#"create unique index "release_disc_titles_release_id_cd_number_index" on "release_disc_titles" ("release_id", "cd_number")"#,
    r#"CREATE TRIGGER release_disc_titles_disc_has_tracks_insert
        BEFORE INSERT ON release_disc_titles
        WHEN NOT EXISTS (
          SELECT 1 FROM tracks
          WHERE release_id = NEW.release_id AND cd_number = NEW.cd_number
        )
        BEGIN
          SELECT RAISE(ABORT, 'a disc title needs a track on its disc');
        END"#,
    r#"CREATE TRIGGER release_disc_titles_disc_has_tracks_update
        BEFORE UPDATE OF release_id, cd_number ON release_disc_titles
        WHEN NOT EXISTS (
          SELECT 1 FROM tracks
          WHERE release_id = NEW.release_id AND cd_number = NEW.cd_number
        )
        BEGIN
          SELECT RAISE(ABORT, 'a disc title needs a track on its disc');
        END"#,
    r#"CREATE TRIGGER tracks_empty_disc_loses_title_delete
        AFTER DELETE ON tracks
        
        BEGIN
          DELETE FROM release_disc_titles
          WHERE release_id = OLD.release_id AND cd_number = OLD.cd_number
            AND NOT EXISTS (
              SELECT 1 FROM tracks
              WHERE release_id = OLD.release_id AND cd_number = OLD.cd_number
            );
        END"#,
    r#"CREATE TRIGGER tracks_empty_disc_loses_title_update
        AFTER UPDATE OF release_id, cd_number ON tracks
        WHEN OLD.release_id <> NEW.release_id OR OLD.cd_number <> NEW.cd_number
        BEGIN
          DELETE FROM release_disc_titles
          WHERE release_id = OLD.release_id AND cd_number = OLD.cd_number
            AND NOT EXISTS (
              SELECT 1 FROM tracks
              WHERE release_id = OLD.release_id AND cd_number = OLD.cd_number
            );
        END"#,
];

const DOWN: &[&str] = &[
    r#"DROP TRIGGER tracks_empty_disc_loses_title_delete"#,
    r#"DROP TRIGGER tracks_empty_disc_loses_title_update"#,
    r#"drop table "release_disc_titles""#,
];
