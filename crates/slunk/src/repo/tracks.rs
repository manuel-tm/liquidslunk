//! Tracks, as the release pages read them: slunk's tracks repository, so
//! far only its reads. Their alternate titles are `titles.rs`.

use std::collections::HashMap;

use rusqlite::{Connection, Row, params_from_iter};

use super::helpers::{chunks, group_by, placeholders};
use crate::Result;
use crate::catalog::{Track, TrackArtist};

const COLUMNS: &str = r#""id", "title", "title_language", "duration_ms", "track_number", "cd_number", "release_id", "file_id", "inserted_at", "updated_at""#;

fn track(row: &Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get(0)?,
        title: row.get(1)?,
        title_language: row.get(2)?,
        duration_ms: row.get(3)?,
        track_number: row.get(4)?,
        cd_number: row.get(5)?,
        release_id: row.get(6)?,
        file_id: row.get(7)?,
        inserted_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

/// The tracks of these ids that exist, in no particular order.
pub fn get_many(conn: &Connection, ids: &[i64]) -> Result<Vec<Track>> {
    let mut found = Vec::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select {COLUMNS} from "tracks" where "id" in ({})"#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), track)? {
            found.push(row?);
        }
    }
    Ok(found)
}

/// Tracks by release, in order: disc, then number.
pub fn of_releases(conn: &Connection, release_ids: &[i64]) -> Result<HashMap<i64, Vec<Track>>> {
    let mut found = Vec::new();
    for chunk in chunks(release_ids) {
        let sql = format!(
            r#"select {COLUMNS} from "tracks" where "release_id" in ({})
            order by "release_id", "cd_number", "track_number""#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), track)? {
            found.push(row?);
        }
    }
    Ok(group_by(found, |track| track.release_id, |track| track))
}

/// Each track's own artists and features, in display order, as credited.
pub fn artists_of(conn: &Connection, track_ids: &[i64]) -> Result<HashMap<i64, Vec<TrackArtist>>> {
    let mut rows = Vec::new();
    for chunk in chunks(track_ids) {
        let sql = format!(
            r#"select "track_id", "artist_id", "credited_name_id", "featured" from "artists_tracks"
            where "track_id" in ({}) order by "track_id", "position""#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                TrackArtist {
                    artist_id: row.get(1)?,
                    credited_name_id: row.get(2)?,
                    featured: row.get::<_, i64>(3)? == 1,
                },
            ))
        })? {
            rows.push(row?);
        }
    }
    Ok(group_by(rows, |(track, _)| *track, |(_, artist)| artist))
}
