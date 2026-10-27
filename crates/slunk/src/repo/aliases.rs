//! Artist aliases: groups of artists that are the same person or band
//! (database.md, "Artist aliases"). Rows are only inserted and deleted, so
//! the trigger that deletes a group's last artist sees every way out of one.

use rusqlite::{Connection, params, params_from_iter};

use super::helpers::{chunks, placeholders};
use super::named::{JOINED_COLUMNS, named};
use crate::Result;
use crate::catalog::Artist;

/// The artist's aliases: the others in its group, by name ignoring case, then
/// by id.
pub fn of(conn: &Connection, artist_id: i64) -> Result<Vec<Artist>> {
    let sql = format!(
        r#"select {JOINED_COLUMNS} from "artist_aliases" "mine"
           join "artist_aliases" "others" on "others"."alias_group" = "mine"."alias_group"
           join "artists" "owners" on "owners"."id" = "others"."artist_id"
           where "mine"."artist_id" = ?1 and "others"."artist_id" <> ?1
           order by "owners"."name" collate nocase, "owners"."id""#
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map([artist_id], named)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Takes the artists out of their groups. A group left with one artist loses
/// it too, through the trigger.
pub fn remove(conn: &Connection, artist_ids: &[i64]) -> Result<()> {
    for chunk in chunks(artist_ids) {
        let sql = format!(
            r#"delete from "artist_aliases" where "artist_id" in ({})"#,
            placeholders(chunk.len())
        );
        conn.execute(&sql, params_from_iter(chunk))?;
    }
    Ok(())
}

/// Puts artists that are in no group into a new one, numbered after the
/// highest. Nothing refers to the number, so one whose group has gone can
/// come back.
pub fn insert_group(conn: &Connection, artist_ids: &[i64]) -> Result<()> {
    let group: i64 = conn.query_row(
        r#"select coalesce(max("alias_group"), 0) + 1 from "artist_aliases""#,
        [],
        |row| row.get(0),
    )?;
    let mut statement =
        conn.prepare(r#"insert into "artist_aliases" ("artist_id", "alias_group") values (?, ?)"#)?;
    for artist_id in artist_ids {
        statement.execute(params![artist_id, group])?;
    }
    Ok(())
}
