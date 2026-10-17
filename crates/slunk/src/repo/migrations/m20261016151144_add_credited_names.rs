//! Credited names: each artist link says which of the artist's alternate
//! names was printed there, or NULL for the main name. A key over
//! `(artist_id, credited_name_id)` keeps the name the linked artist's own.
//! `ADD COLUMN` can't add a key over two columns, so the four link tables are
//! rebuilt. Every existing link keeps the main name, as it shows now.

use rusqlite::Transaction;

use super::run;
use crate::Result;
use crate::repo::rebuild_table::{Rebuild, rebuild_table};

/// One link table's rebuild: the index dropped first, since the rebuild would
/// bring it back as it was, the table's new definition, and the index made
/// after.
struct Step {
    table: &'static str,
    drop_index: &'static str,
    define: &'static str,
    create_index: &'static str,
}

pub fn up(tx: &Transaction) -> Result<()> {
    rebuild(tx, &UP)
}

pub fn down(tx: &Transaction) -> Result<()> {
    rebuild(tx, &DOWN)
}

fn rebuild(tx: &Transaction, steps: &[Step]) -> Result<()> {
    for step in steps {
        run(tx, &[step.drop_index])?;
        rebuild_table(
            tx,
            step.table,
            Rebuild {
                define: step.define,
                copy: &[],
            },
        )?;
        run(tx, &[step.create_index])?;
    }
    Ok(())
}

// NULL is the main name, and SQLite doesn't check a key with a NULL in it.
// RESTRICT, since SET NULL would clear artist_id too.
const UP: [Step; 4] = [
    Step {
        table: "artists_releases",
        drop_index: r#"drop index "artists_releases_artist_id_index""#,
        define: r#"("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "release_id" integer not null references "releases" ("id") on delete cascade, "position" integer not null check (position >= 0), "credited_name_id" integer, constraint "artists_releases_credited_name_is_the_artists" foreign key ("artist_id", "credited_name_id") references "artist_alternate_names" ("artist_id", "id") on delete restrict) strict"#,
        create_index: r#"create index "artists_releases_artist_id_credited_name_id_index" on "artists_releases" ("artist_id", "credited_name_id")"#,
    },
    Step {
        table: "artists_tracks",
        drop_index: r#"drop index "artists_tracks_artist_id_index""#,
        define: r#"("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "track_id" integer not null references "tracks" ("id") on delete cascade, "featured" integer default 0 not null check (featured IN (0, 1)), "position" integer not null check (position >= 0), "credited_name_id" integer, constraint "artists_tracks_credited_name_is_the_artists" foreign key ("artist_id", "credited_name_id") references "artist_alternate_names" ("artist_id", "id") on delete restrict) strict"#,
        create_index: r#"create index "artists_tracks_artist_id_credited_name_id_index" on "artists_tracks" ("artist_id", "credited_name_id")"#,
    },
    Step {
        table: "artists_release_credits",
        drop_index: r#"drop index "artists_release_credits_artist_id_index""#,
        define: r#"("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "credit_id" integer not null references "release_credits" ("id") on delete cascade, "position" integer not null check (position >= 0), "credited_name_id" integer, constraint "artists_release_credits_credited_name_is_the_artists" foreign key ("artist_id", "credited_name_id") references "artist_alternate_names" ("artist_id", "id") on delete restrict) strict"#,
        create_index: r#"create index "artists_release_credits_artist_id_credited_name_id_index" on "artists_release_credits" ("artist_id", "credited_name_id")"#,
    },
    Step {
        table: "artists_track_credits",
        drop_index: r#"drop index "artists_track_credits_artist_id_index""#,
        define: r#"("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "credit_id" integer not null references "track_credits" ("id") on delete cascade, "position" integer not null check (position >= 0), "credited_name_id" integer, constraint "artists_track_credits_credited_name_is_the_artists" foreign key ("artist_id", "credited_name_id") references "artist_alternate_names" ("artist_id", "id") on delete restrict) strict"#,
        create_index: r#"create index "artists_track_credits_artist_id_credited_name_id_index" on "artists_track_credits" ("artist_id", "credited_name_id")"#,
    },
];

const DOWN: [Step; 4] = [
    Step {
        table: "artists_releases",
        drop_index: r#"drop index "artists_releases_artist_id_credited_name_id_index""#,
        define: r#"("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "release_id" integer not null references "releases" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict"#,
        create_index: r#"create index "artists_releases_artist_id_index" on "artists_releases" ("artist_id")"#,
    },
    Step {
        table: "artists_tracks",
        drop_index: r#"drop index "artists_tracks_artist_id_credited_name_id_index""#,
        define: r#"("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "track_id" integer not null references "tracks" ("id") on delete cascade, "featured" integer default 0 not null check (featured IN (0, 1)), "position" integer not null check (position >= 0)) strict"#,
        create_index: r#"create index "artists_tracks_artist_id_index" on "artists_tracks" ("artist_id")"#,
    },
    Step {
        table: "artists_release_credits",
        drop_index: r#"drop index "artists_release_credits_artist_id_credited_name_id_index""#,
        define: r#"("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "credit_id" integer not null references "release_credits" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict"#,
        create_index: r#"create index "artists_release_credits_artist_id_index" on "artists_release_credits" ("artist_id")"#,
    },
    Step {
        table: "artists_track_credits",
        drop_index: r#"drop index "artists_track_credits_artist_id_credited_name_id_index""#,
        define: r#"("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "credit_id" integer not null references "track_credits" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict"#,
        create_index: r#"create index "artists_track_credits_artist_id_index" on "artists_track_credits" ("artist_id")"#,
    },
];
