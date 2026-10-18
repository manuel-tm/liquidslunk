//! What only artists have: names credited on links, and what keeps an artist
//! from being deleted. slunk's artists repository; the rest is `named.rs`.

use std::collections::HashMap;

use rusqlite::{Connection, params, params_from_iter};

use super::helpers::{chunks, placeholders};
use crate::Result;
use crate::catalog::{AlternateName, ArtistCredit, ArtistUsage, FormatCount};

/// Every table that links an artist to a release, a track or a credit line,
/// each with the alternate name it's credited with.
const ARTIST_LINKS: [&str; 4] = [
    "artists_releases",
    "artists_tracks",
    "artists_release_credits",
    "artists_track_credits",
];

/// The new name to add after the artist's others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAlternateName {
    pub name: String,
    pub language: Option<String>,
    pub real_name: bool,
    pub note: Option<String>,
}

/// Adds an alternate name after the artist's others.
pub fn add_alternate_name(
    conn: &Connection,
    artist_id: i64,
    name: &NewAlternateName,
) -> Result<AlternateName> {
    Ok(conn.query_row(
        r#"insert into "artist_alternate_names" ("artist_id", "name", "language", "real_name", "note", "position")
           values (?1, ?2, ?3, ?4, ?5, (select coalesce(max(position) + 1, 0) from artist_alternate_names where artist_id = ?1))
           returning "id", "name", "language", "real_name", "note""#,
        params![
            artist_id,
            name.name,
            name.language,
            i64::from(name.real_name),
            name.note
        ],
        |row| {
            Ok(AlternateName {
                id: row.get(0)?,
                name: row.get(1)?,
                language: row.get(2)?,
                real_name: row.get::<_, i64>(3)? == 1,
                note: row.get(4)?,
            })
        },
    )?)
}

/// How many artist links are credited with each name. A name with none is
/// left out.
pub fn name_uses(conn: &Connection, name_ids: &[i64]) -> Result<HashMap<i64, i64>> {
    let mut uses = HashMap::new();
    for chunk in chunks(name_ids) {
        let list = placeholders(chunk.len());
        let selects: Vec<String> = ARTIST_LINKS
            .iter()
            .map(|table| {
                format!("select credited_name_id from {table} where credited_name_id in ({list})")
            })
            .collect();
        let sql = format!(
            "select credited_name_id as nameId, count(*) as uses from ({}) group by credited_name_id",
            selects.join(" union all ")
        );
        let values = ARTIST_LINKS.iter().flat_map(|_| chunk.iter());
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(values), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })? {
            let (name_id, count) = row?;
            uses.insert(name_id, count);
        }
    }
    Ok(uses)
}

/// How many of the artist's links show its main name.
pub fn main_name_uses(conn: &Connection, artist_id: i64) -> Result<i64> {
    let counts: Vec<String> = ARTIST_LINKS
        .iter()
        .map(|table| {
            format!(
                "(select count(*) from {table} where artist_id = ?1 and credited_name_id is null)"
            )
        })
        .collect();
    let sql = format!("select {} as uses", counts.join(" + "));
    Ok(conn.query_row(&sql, [artist_id], |row| row.get(0))?)
}

/// Credits the artist's links that show its main name with a name instead.
pub fn credit_main_name_links(conn: &Connection, artist_id: i64, name_id: i64) -> Result<()> {
    for table in ARTIST_LINKS {
        conn.execute(
            &format!(
                "update {table} set credited_name_id = ? where artist_id = ? and credited_name_id is null"
            ),
            params![name_id, artist_id],
        )?;
    }
    Ok(())
}

/// Credits the artist's links that use these names with the main name.
pub fn uncredit(conn: &Connection, artist_id: i64, name_ids: &[i64]) -> Result<()> {
    for chunk in chunks(name_ids) {
        for table in ARTIST_LINKS {
            let sql = format!(
                "update {table} set credited_name_id = null where artist_id = ? and credited_name_id in ({})",
                placeholders(chunk.len())
            );
            let values = std::iter::once(&artist_id).chain(chunk);
            conn.execute(&sql, params_from_iter(values))?;
        }
    }
    Ok(())
}

/// Where the artist is used, which keeps it from being deleted.
pub fn usage(conn: &Connection, id: i64) -> Result<ArtistUsage> {
    Ok(conn.query_row(
        "select
           (select count(*) from artists_releases where artist_id = ?1) as releases,
           (select count(*) from artists_tracks where artist_id = ?1) as tracks,
           (select count(*) from artists_release_credits where artist_id = ?1)
             + (select count(*) from artists_track_credits where artist_id = ?1)
             as credits",
        [id],
        |row| {
            Ok(ArtistUsage {
                releases: row.get(0)?,
                tracks: row.get(1)?,
                credits: row.get(2)?,
            })
        },
    )?)
}

/// Alternate names, by their ids, in no particular order.
pub fn alternate_names_by_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<AlternateName>> {
    let mut found = Vec::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select "id", "name", "language", "real_name", "note" from "artist_alternate_names"
            where "id" in ({})"#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok(AlternateName {
                id: row.get(0)?,
                name: row.get(1)?,
                language: row.get(2)?,
                real_name: row.get::<_, i64>(3)? == 1,
                note: row.get(4)?,
            })
        })? {
            found.push(row?);
        }
    }
    Ok(found)
}

/// The tracks, by release, where the artist performs or is featured without
/// being a release artist: release and track ids, by release, disc and
/// number.
pub fn appearances(conn: &Connection, id: i64) -> Result<Vec<(i64, i64)>> {
    let mut statement = conn.prepare(
        "select t.release_id as releaseId, t.id as trackId
         from artists_tracks a join tracks t on t.id = a.track_id
         where a.artist_id = ?1
           and not exists (
             select 1 from artists_releases r
             where r.release_id = t.release_id and r.artist_id = ?1
           )
         order by t.release_id, t.cd_number, t.track_number",
    )?;
    let rows = statement
        .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// A row per credit line the artist is on: the release's lines, then the
/// tracks'.
pub fn credits(conn: &Connection, id: i64) -> Result<Vec<ArtistCredit>> {
    let mut statement = conn.prepare(
        "select l.role, l.release_id as releaseId, null as trackId,
           a.credited_name_id as creditedNameId
         from artists_release_credits a
         join release_credits l on l.id = a.credit_id
         where a.artist_id = ?1
         union all
         select l.role, t.release_id, t.id, a.credited_name_id
         from artists_track_credits a
         join track_credits l on l.id = a.credit_id
         join tracks t on t.id = l.track_id
         where a.artist_id = ?1",
    )?;
    let rows = statement
        .query_map([id], |row| {
            Ok(ArtistCredit {
                role: row.get(0)?,
                release_id: row.get(1)?,
                track_id: row.get(2)?,
                credited_name_id: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// How many of the artist's releases have each format.
pub fn format_counts(conn: &Connection, id: i64) -> Result<Vec<FormatCount>> {
    let mut statement = conn.prepare(
        "select f.format_id as formatId, count(*) as count
         from release_formats_releases f
         join artists_releases a on a.release_id = f.release_id
         where a.artist_id = ?
         group by f.format_id",
    )?;
    let rows = statement
        .query_map([id], |row| {
            Ok(FormatCount {
                format_id: row.get(0)?,
                count: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}
