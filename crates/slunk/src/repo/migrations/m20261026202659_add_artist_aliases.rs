//! Artist aliases (database.md, "Artist aliases"): artists that are the same
//! person or band under other names, kept as groups. Every artist in a group
//! is an alias of every other one.
//!
//! - An artist is in one group at most: the table is keyed by the artist.
//! - A group is only a number, which nothing else refers to.
//! - A group has two artists or more: when one leaves and one is left, the
//!   trigger deletes that one too, whether its row or its artist was deleted.
//! - Rolling back deletes the groups.

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
    r#"create table "artist_aliases" ("artist_id" integer not null primary key references "artists" ("id") on delete cascade, "alias_group" integer not null check ("alias_group" > 0)) strict"#,
    r#"create index "artist_aliases_alias_group_index" on "artist_aliases" ("alias_group")"#,
    r#"CREATE TRIGGER artist_aliases_group_of_one_delete
        AFTER DELETE ON artist_aliases
        WHEN (SELECT count(*) FROM artist_aliases WHERE alias_group = OLD.alias_group) = 1
        BEGIN
          DELETE FROM artist_aliases WHERE alias_group = OLD.alias_group;
        END"#,
];

const DOWN: &[&str] = &[r#"drop table "artist_aliases""#];
