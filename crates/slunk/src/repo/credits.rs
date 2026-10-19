//! Credit lines of releases and tracks: a role as printed, and its artists in
//! order, each as credited. slunk's credits repository.

use std::collections::HashMap;

use rusqlite::{Connection, params, params_from_iter};

use super::helpers::{chunks, placeholders};
use crate::Result;
use crate::catalog::{ArtistLink, Credit, CreditOwner};
use crate::paging::like_pattern;

/// An owner's lines, the table of their artists, and the owner's column.
fn tables(owner: CreditOwner) -> (&'static str, &'static str, &'static str) {
    match owner {
        CreditOwner::Release => ("release_credits", "artists_release_credits", "release_id"),
        CreditOwner::Track => ("track_credits", "artists_track_credits", "track_id"),
    }
}

/// Credit lines by release or track, in display order.
pub fn of(conn: &Connection, owner: CreditOwner, ids: &[i64]) -> Result<HashMap<i64, Vec<Credit>>> {
    let (lines, artists, column) = tables(owner);
    let mut credits: HashMap<i64, Vec<Credit>> = HashMap::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select l."{column}", l."id", l."role", a."artist_id", a."credited_name_id"
            from "{lines}" l
            join "{artists}" a on a."credit_id" = l."id"
            where l."{column}" in ({})
            order by l."{column}", l."position", a."position""#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        let mut last_line = None;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                ArtistLink {
                    artist_id: row.get(3)?,
                    credited_name_id: row.get(4)?,
                },
            ))
        })? {
            let (owner_id, line_id, role, link) = row?;
            let owned = credits.entry(owner_id).or_default();
            match owned.last_mut() {
                Some(line) if last_line == Some(line_id) => line.artists.push(link),
                _ => owned.push(Credit {
                    role,
                    artists: vec![link],
                }),
            }
            last_line = Some(line_id);
        }
    }
    Ok(credits)
}

/// Replaces an owner's lines, in display order. Run it in a transaction:
/// the database checks that each line has an artist at commit.
pub fn set(conn: &Connection, owner: CreditOwner, id: i64, credits: &[Credit]) -> Result<()> {
    let (lines, artists, column) = tables(owner);
    // Each line's artists go with it (CASCADE)
    conn.execute(
        &format!(r#"delete from "{lines}" where "{column}" = ?"#),
        [id],
    )?;
    for (position, credit) in credits.iter().enumerate() {
        let line: i64 = conn.query_row(
            &format!(
                r#"insert into "{lines}" ("{column}", "role", "position") values (?, ?, ?) returning "id""#
            ),
            params![id, credit.role, position as i64],
            |row| row.get(0),
        )?;
        // Numbered from 0: the line's key needs an artist at position 0
        for (place, link) in credit.artists.iter().enumerate() {
            conn.execute(
                &format!(
                    r#"insert into "{artists}" ("credit_id", "artist_id", "credited_name_id", "position")
                    values (?, ?, ?, ?)"#
                ),
                params![line, link.artist_id, link.credited_name_id, place as i64],
            )?;
        }
    }
    Ok(())
}

/// Roles in use on releases and tracks, each spelling once, the most used
/// first, for the credits editor's suggestions.
pub fn roles(conn: &Connection, search: Option<&str>, limit: i64) -> Result<Vec<String>> {
    let pattern = search.map(like_pattern);
    let filter = if pattern.is_some() {
        r#"where "role" like ? escape '\'"#
    } else {
        ""
    };
    // `role` ignores case, so grouping by its exact text keeps each spelling
    // apart: "Mixed By" and "Mixed by" are two suggestions
    let sql = format!(
        r#"select "role" from (
          select "role" from "release_credits" {filter}
          union all
          select "role" from "track_credits" {filter}
        )
        group by "role" collate binary
        order by count(*) desc, "role" collate nocase, "role" collate binary
        limit ?"#
    );
    let mut values: Vec<rusqlite::types::Value> = Vec::new();
    for _ in 0..2 {
        values.extend(pattern.clone().map(rusqlite::types::Value::Text));
    }
    values.push(rusqlite::types::Value::Integer(limit));
    let mut statement = conn.prepare(&sql)?;
    let roles = statement
        .query_map(params_from_iter(values), |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(roles)
}
