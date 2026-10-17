//! File links: images and other files linked to releases, artists and
//! tracks, each with an optional note. slunk's links repository, for files.

use std::collections::HashMap;

use rusqlite::{Connection, params, params_from_iter};

use super::helpers::{chunks, group_by, placeholders};
use crate::Result;
use crate::media::{FileLink, LinkOwner};

/// Each owner's table, and its column.
fn table_of(owner: LinkOwner) -> (&'static str, &'static str) {
    match owner {
        LinkOwner::Release => ("files_releases", "release_id"),
        LinkOwner::Artist => ("artists_files", "artist_id"),
        LinkOwner::Track => ("files_tracks", "track_id"),
    }
}

/// Links by owner, in the order they were added.
pub fn of(conn: &Connection, owner: LinkOwner, ids: &[i64]) -> Result<HashMap<i64, Vec<FileLink>>> {
    let (table, column) = table_of(owner);
    let mut rows: Vec<(i64, FileLink)> = Vec::new();
    for chunk in chunks(ids) {
        let sql = format!(
            "select {column} as ownerId, file_id as fileId, note from {table} where {column} in ({}) order by {column}, id",
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok((
                row.get(0)?,
                FileLink {
                    file_id: row.get(1)?,
                    note: row.get(2)?,
                },
            ))
        })? {
            rows.push(row?);
        }
    }
    Ok(group_by(rows, |(owner, _)| *owner, |(_, link)| link))
}

/// Replaces the owner's links. Run it in a transaction.
pub fn set(conn: &Connection, owner: LinkOwner, id: i64, links: &[FileLink]) -> Result<()> {
    let (table, column) = table_of(owner);
    conn.execute(&format!("delete from {table} where {column} = ?"), [id])?;
    let mut insert = conn.prepare(&format!(
        "insert into {table} ({column}, file_id, note) values (?, ?, ?)"
    ))?;
    for link in links {
        insert.execute(params![id, link.file_id, link.note])?;
    }
    Ok(())
}
