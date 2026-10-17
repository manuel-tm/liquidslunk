//! Registered files: slunk's files repository.

use std::collections::HashMap;

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use sea_query::{Asterisk, Expr, ExprTrait, Iden, LikeExpr, Order, Query, SqliteQueryBuilder};

use super::helpers::{bind, chunks, placeholders};
use crate::Result;
use crate::media::{
    FileChanges, FileCounts, FileQuery, FileRecord, FileSort, FileTrack, FileUse, FileUseKind,
    NewFile,
};
use crate::paging::like_pattern;

#[derive(Iden)]
enum Files {
    Table,
    Id,
    Path,
    #[iden = "type"]
    Type,
}

/// Matches files nothing uses: the uses that block a delete (slunk's design
/// §4.3).
const UNUSED: &str = "not exists (select 1 from tracks where file_id = files.id)
  and not exists (select 1 from releases where cover_picture = files.id)
  and not exists (select 1 from artists where picture = files.id)
  and not exists (select 1 from labels where picture = files.id)
  and not exists (select 1 from files_releases where file_id = files.id)
  and not exists (select 1 from artists_files where file_id = files.id)
  and not exists (select 1 from files_tracks where file_id = files.id)";

const WITHOUT_TRACK: &str = "not exists (select 1 from tracks where file_id = files.id)";

const COLUMNS: &str =
    r#""id", "path", "type", "size_bytes", "mtime", "sha256", "inserted_at", "updated_at""#;

/// A page of files, and how many match in all.
pub fn list(conn: &Connection, query: &FileQuery) -> Result<(Vec<FileRecord>, i64)> {
    let mut select = Query::select();
    select.from(Files::Table);
    if let Some(search) = &query.search {
        select.and_where(
            Expr::col(Files::Path).like(LikeExpr::new(like_pattern(search)).escape('\\')),
        );
    }
    if let Some(file_type) = query.file_type {
        select.and_where(Expr::col(Files::Type).eq(file_type.as_str()));
    }
    if query.unused {
        select.and_where(Expr::cust(UNUSED));
    }
    if query.without_track {
        select.and_where(Expr::cust(WITHOUT_TRACK));
    }

    let (sql, values) = select
        .clone()
        .expr(Expr::cust("count(*)"))
        .build(SqliteQueryBuilder);
    let total = conn.query_row(&sql, params_from_iter(bind(values)), |row| row.get(0))?;

    let column = match query.sort {
        None => Files::Id,
        Some(FileSort::Path) => Files::Path,
    };
    let order = if query.descending {
        Order::Desc
    } else {
        Order::Asc
    };
    let (sql, values) = select
        .column(Asterisk)
        .order_by(column, order)
        .limit(query.paging.limit as u64)
        .offset(query.paging.offset as u64)
        .build(SqliteQueryBuilder);
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map(params_from_iter(bind(values)), record)?
        .collect::<rusqlite::Result<_>>()?;
    Ok((rows, total))
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<FileRecord>> {
    Ok(conn
        .query_row(
            &format!(r#"select {COLUMNS} from "files" where "id" = ?"#),
            [id],
            record,
        )
        .optional()?)
}

/// A registered file's path, relative to the database's folder, or `None` for
/// an unknown id.
pub fn path(conn: &Connection, id: i64) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            r#"select "path" from "files" where "id" = ?"#,
            [id],
            |row| row.get(0),
        )
        .optional()?)
}

/// The files with these ids. Unknown ids are left out.
pub fn get_many(conn: &Connection, ids: &[i64]) -> Result<Vec<FileRecord>> {
    let mut found = Vec::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select {COLUMNS} from "files" where "id" in ({})"#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), record)? {
            found.push(row?);
        }
    }
    Ok(found)
}

/// The files registered at these paths.
pub fn find_by_paths(conn: &Connection, paths: &[String]) -> Result<Vec<FileRecord>> {
    let mut found = Vec::new();
    for chunk in chunks(paths) {
        let sql = format!(
            r#"select {COLUMNS} from "files" where "path" in ({})"#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), record)? {
            found.push(row?);
        }
    }
    Ok(found)
}

/// Every registered file, by path, for the library check.
pub fn all(conn: &Connection) -> Result<Vec<FileRecord>> {
    let mut statement =
        conn.prepare(&format!(r#"select {COLUMNS} from "files" order by "path""#))?;
    let rows = statement
        .query_map([], record)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Inserts files, in order, so their ids follow it. Run it in a
/// transaction to insert all or none.
pub fn insert(conn: &Connection, files: &[NewFile]) -> Result<Vec<FileRecord>> {
    let mut statement = conn.prepare(&format!(
        r#"insert into "files" ("path", "type", "size_bytes", "mtime", "sha256") values (?, ?, ?, ?, ?) returning {COLUMNS}"#
    ))?;
    let mut inserted = Vec::with_capacity(files.len());
    for file in files {
        inserted.push(statement.query_row(
            params![
                file.path,
                file.file_type,
                file.facts.size_bytes,
                file.facts.mtime,
                file.facts.sha256
            ],
            record,
        )?);
    }
    Ok(inserted)
}

/// Changes a file, and sets `updated_at`. `None` if there's no such file.
pub fn update(conn: &Connection, id: i64, changes: &FileChanges) -> Result<Option<FileRecord>> {
    let mut sets = Vec::new();
    let mut values: Vec<SqlValue> = Vec::new();
    if let Some(path) = &changes.path {
        sets.push(r#""path" = ?"#);
        values.push(SqlValue::Text(path.clone()));
    }
    if let Some(file_type) = changes.file_type {
        sets.push(r#""type" = ?"#);
        values.push(SqlValue::Text(file_type.as_str().into()));
    }
    if let Some(facts) = &changes.facts {
        sets.extend([r#""size_bytes" = ?"#, r#""mtime" = ?"#, r#""sha256" = ?"#]);
        values.extend([
            SqlValue::Integer(facts.size_bytes),
            SqlValue::Integer(facts.mtime),
            SqlValue::Text(facts.sha256.clone()),
        ]);
    }
    sets.push(r#""updated_at" = unixepoch()"#);
    values.push(SqlValue::Integer(id));
    let sql = format!(
        r#"update "files" set {} where "id" = ? returning {COLUMNS}"#,
        sets.join(", ")
    );
    Ok(conn
        .query_row(&sql, params_from_iter(values), record)
        .optional()?)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(r#"delete from "files" where "id" = ?"#, [id])?;
    Ok(())
}

/// Everything that uses a file.
pub fn uses(conn: &Connection, id: i64) -> Result<Vec<FileUse>> {
    let mut statement = conn.prepare(
        "select 'track_audio' as kind, id as entityId, title as name, null as note
        from tracks where file_id = ?1
        union all
        select 'release_cover', id, title, null
        from releases where cover_picture = ?1
        union all
        select 'artist_picture', id, name, null
        from artists where picture = ?1
        union all
        select 'label_picture', id, name, null
        from labels where picture = ?1
        union all
        select 'release_link', r.id, r.title, l.note
        from files_releases l join releases r on r.id = l.release_id
        where l.file_id = ?1
        union all
        select 'artist_link', a.id, a.name, l.note
        from artists_files l join artists a on a.id = l.artist_id
        where l.file_id = ?1
        union all
        select 'track_link', t.id, t.title, l.note
        from files_tracks l join tracks t on t.id = l.track_id
        where l.file_id = ?1",
    )?;
    let rows = statement
        .query_map([id], |row| {
            let kind: String = row.get(0)?;
            Ok(FileUse {
                kind: FileUseKind::parse(&kind).expect("a kind uses() selects"),
                entity_id: row.get(1)?,
                name: row.get(2)?,
                note: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn counts(conn: &Connection) -> Result<FileCounts> {
    Ok(conn.query_row(
        &format!("select count(*) as total, count(*) filter (where {UNUSED}) as unused from files"),
        [],
        |row| {
            Ok(FileCounts {
                total: row.get(0)?,
                unused: row.get(1)?,
            })
        },
    )?)
}

/// Whether a registered path starts with `prefix`, which ends in `/`, or,
/// with `negate`, whether one doesn't.
pub fn has_path_starting_with(conn: &Connection, prefix: &str, negate: bool) -> Result<bool> {
    // A range on the path index: "0" sorts right after "/"
    let end = format!("{}0", &prefix[..prefix.len() - 1]);
    let in_range = r#""path" >= ?1 and "path" < ?2"#;
    let condition = if negate {
        format!("not ({in_range})")
    } else {
        in_range.to_string()
    };
    let sql = format!(r#"select "id" from "files" where {condition} limit 1"#);
    Ok(conn
        .query_row(&sql, [prefix, &end], |_| Ok(()))
        .optional()?
        .is_some())
}

/// The track each of these files is the audio of, by file id.
pub fn tracks_of(conn: &Connection, file_ids: &[i64]) -> Result<HashMap<i64, FileTrack>> {
    let mut found = HashMap::new();
    for chunk in chunks(file_ids) {
        let sql = format!(
            r#"select "file_id", "id", "title" from "tracks" where "file_id" in ({})"#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(chunk), |row| {
            Ok((
                row.get(0)?,
                FileTrack {
                    id: row.get(1)?,
                    title: row.get(2)?,
                },
            ))
        })?;
        for row in rows {
            let (file_id, track) = row?;
            found.insert(file_id, track);
        }
    }
    Ok(found)
}

fn record(row: &Row<'_>) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        id: row.get("id")?,
        path: row.get("path")?,
        file_type: row.get("type")?,
        size_bytes: row.get("size_bytes")?,
        mtime: row.get("mtime")?,
        sha256: row.get("sha256")?,
        inserted_at: row.get("inserted_at")?,
        updated_at: row.get("updated_at")?,
    })
}
