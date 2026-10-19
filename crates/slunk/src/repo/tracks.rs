//! Tracks, and who performs them: slunk's tracks repository. Their
//! alternate titles are `titles.rs`, and their tags `tags.rs`.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use sea_query::{Expr, ExprTrait, Order, Query, SqliteQueryBuilder};

use super::helpers::{bind, chunks, group_by, placeholders};
use super::tags::tag_matches;
use super::titles::{TitleOwner, title_matches};
use crate::Result;
use crate::catalog::{NewTrack, Track, TrackArtist, TrackChanges, TrackQuery, TrackSort};
use crate::tagging::TagOwner;

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

/// A page, and how many match in all.
pub fn list(conn: &Connection, query: &TrackQuery) -> Result<(Vec<Track>, i64)> {
    let mut select = Query::select();
    select.from("tracks");
    if let Some(search) = &query.search {
        select.and_where(title_matches(TitleOwner::Track, search));
    }
    if let Some(release) = query.release {
        select.and_where(Expr::col("release_id").eq(release));
    }
    for filter in &query.tags {
        select.and_where(tag_matches(TagOwner::Track, filter));
    }
    let (sql, values) = select
        .clone()
        .expr(Expr::cust("count(*)"))
        .build(SqliteQueryBuilder);
    let total = conn.query_row(&sql, params_from_iter(bind(values)), |row| row.get(0))?;

    let order = if query.sort.descending {
        Order::Desc
    } else {
        Order::Asc
    };
    select.expr(Expr::cust(COLUMNS));
    if query.sort.column == Some(TrackSort::Title) {
        select.order_by_expr(Expr::cust(r#""title" collate nocase"#), order.clone());
    }
    let (sql, values) = select
        .order_by("id", order)
        .limit(query.paging.limit as u64)
        .offset(query.paging.offset as u64)
        .build(SqliteQueryBuilder);
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map(params_from_iter(bind(values)), track)?
        .collect::<rusqlite::Result<_>>()?;
    Ok((rows, total))
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<Track>> {
    let sql = format!(r#"select {COLUMNS} from "tracks" where "id" = ?"#);
    Ok(conn.query_row(&sql, [id], track).optional()?)
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

/// The track each of these files is the audio of, by file id.
pub fn of_files(conn: &Connection, file_ids: &[i64]) -> Result<HashMap<i64, Track>> {
    let mut found = HashMap::new();
    for chunk in chunks(file_ids) {
        let sql = format!(
            r#"select {COLUMNS} from "tracks" where "file_id" in ({})"#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), track)? {
            let track = row?;
            found.insert(track.file_id, track);
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

pub fn insert(conn: &Connection, values: &NewTrack) -> Result<Track> {
    let sql = format!(
        r#"insert into "tracks" ("title", "title_language", "duration_ms", "track_number", "cd_number", "release_id", "file_id")
        values (?, ?, ?, ?, ?, ?, ?) returning {COLUMNS}"#
    );
    Ok(conn.query_row(
        &sql,
        params![
            values.title,
            values.title_language,
            values.duration_ms,
            values.track_number,
            values.cd_number,
            values.release_id,
            values.file_id
        ],
        track,
    )?)
}

/// Changes the columns given, and sets `updated_at`. `None` if there's no
/// such track.
pub fn update(conn: &Connection, id: i64, changes: &TrackChanges) -> Result<Option<Track>> {
    let mut update = Query::update();
    update.table("tracks");
    if let Some(title) = &changes.title {
        update.value("title", title.clone());
    }
    if let Some(language) = &changes.title_language {
        update.value("title_language", language.clone());
    }
    if let Some(duration) = changes.duration_ms {
        update.value("duration_ms", duration);
    }
    if let Some(number) = changes.track_number {
        update.value("track_number", number);
    }
    if let Some(number) = changes.cd_number {
        update.value("cd_number", number);
    }
    if let Some(release) = changes.release_id {
        update.value("release_id", release);
    }
    if let Some(file) = changes.file_id {
        update.value("file_id", file);
    }
    let (sql, values) = update
        .value("updated_at", Expr::cust("unixepoch()"))
        .and_where(Expr::col("id").eq(id))
        .build(SqliteQueryBuilder);
    let sql = format!("{sql} returning {COLUMNS}");
    Ok(conn
        .query_row(&sql, params_from_iter(bind(values)), track)
        .optional()?)
}

/// Deletes a track. Its artists, credit lines, alternate titles, file links
/// and tags go with it, and its file stays registered. An unknown id deletes
/// nothing.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(r#"delete from "tracks" where "id" = ?"#, [id])?;
    Ok(())
}

/// Adds to every track number of a release, without touching `updated_at`.
pub fn shift_numbers(conn: &Connection, release_id: i64, by: i64) -> Result<()> {
    conn.execute(
        r#"update "tracks" set "track_number" = "track_number" + ? where "release_id" = ?"#,
        params![by, release_id],
    )?;
    Ok(())
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

/// Replaces a track's artists and features. Their order is the display
/// order.
pub fn set_artists(conn: &Connection, track_id: i64, artists: &[TrackArtist]) -> Result<()> {
    conn.execute(
        r#"delete from "artists_tracks" where "track_id" = ?"#,
        [track_id],
    )?;
    for (position, artist) in artists.iter().enumerate() {
        conn.execute(
            r#"insert into "artists_tracks" ("track_id", "artist_id", "credited_name_id", "featured", "position")
            values (?, ?, ?, ?, ?)"#,
            params![
                track_id,
                artist.artist_id,
                artist.credited_name_id,
                i64::from(artist.featured),
                position as i64
            ],
        )?;
    }
    Ok(())
}
