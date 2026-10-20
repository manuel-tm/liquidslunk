//! External libraries and their rules (4_external_libraries.md §9), and the
//! tracks the rules come to.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use sea_query::{Condition, Expr, Order, Query, SqliteQueryBuilder};

use super::helpers::{bind, chunks, group_by, placeholders};
use super::tags::tag_matches;
use crate::Result;
use crate::external::{
    ExternalKind, ExternalLibrary, ExternalLibraryValues, OutputFormat, Profile, Rule, RuleTarget,
    RuleValues,
};
use crate::tagging::{TagFilter, TagOwner, TagType};

const COLUMNS: &str = r#""id", "uuid", "name", "kind", "folder", "format", "aac_kbps", "cd_resolution", "path_template", "embed_covers", "cover_px", "cover_file", "last_synced_at", "inserted_at", "updated_at""#;

const RULE_COLUMNS: &str = r#""id", "external_library_id", "exclude", "release_id", "artist_id", "track_id", "text_tag_id", "real_number_tag_id", "date_tag_id", "tag_equals", "tag_min", "tag_max""#;

fn library(row: &Row) -> rusqlite::Result<ExternalLibrary> {
    let text = |index: usize| -> rusqlite::Result<String> { row.get(index) };
    let unknown = |index: usize, value: &str| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            format!("an unknown value: {value}").into(),
        )
    };
    let kind = text(3)?;
    let format = text(5)?;
    Ok(ExternalLibrary {
        id: row.get(0)?,
        uuid: row.get(1)?,
        name: row.get(2)?,
        kind: ExternalKind::named(&kind).ok_or_else(|| unknown(3, &kind))?,
        folder: row.get(4)?,
        profile: Profile {
            format: OutputFormat::named(&format).ok_or_else(|| unknown(5, &format))?,
            aac_kbps: row.get(6)?,
            cd_resolution: row.get::<_, i64>(7)? == 1,
            path_template: row.get(8)?,
            embed_covers: row.get::<_, i64>(9)? == 1,
            cover_px: row.get(10)?,
            cover_file: row.get::<_, i64>(11)? == 1,
        },
        last_synced_at: row.get(12)?,
        inserted_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

/// Every external library, by name ignoring case.
pub fn list(conn: &Connection) -> Result<Vec<ExternalLibrary>> {
    let sql = format!(
        r#"select {COLUMNS} from "external_libraries" order by "name" collate nocase, "id""#
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map([], library)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<ExternalLibrary>> {
    let sql = format!(r#"select {COLUMNS} from "external_libraries" where "id" = ?"#);
    Ok(conn.query_row(&sql, [id], library).optional()?)
}

pub fn insert(
    conn: &Connection,
    uuid: &str,
    values: &ExternalLibraryValues,
) -> Result<ExternalLibrary> {
    let profile = &values.profile;
    let sql = format!(
        r#"insert into "external_libraries" ("uuid", "name", "kind", "folder", "format", "aac_kbps", "cd_resolution", "path_template", "embed_covers", "cover_px", "cover_file")
        values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) returning {COLUMNS}"#
    );
    Ok(conn.query_row(
        &sql,
        params![
            uuid,
            values.name,
            values.kind.as_str(),
            values.folder,
            profile.format.as_str(),
            profile.aac_kbps,
            i64::from(profile.cd_resolution),
            profile.path_template,
            i64::from(profile.embed_covers),
            profile.cover_px,
            i64::from(profile.cover_file),
        ],
        library,
    )?)
}

/// Saves an external library's name, folder and profile, and sets
/// `updated_at`. Its kind stays as it is. `None` if there's no such one.
pub fn update(
    conn: &Connection,
    id: i64,
    values: &ExternalLibraryValues,
) -> Result<Option<ExternalLibrary>> {
    let profile = &values.profile;
    let sql = format!(
        r#"update "external_libraries" set "name" = ?, "folder" = ?, "format" = ?, "aac_kbps" = ?,
          "cd_resolution" = ?, "path_template" = ?, "embed_covers" = ?, "cover_px" = ?,
          "cover_file" = ?, "updated_at" = unixepoch()
        where "id" = ? returning {COLUMNS}"#
    );
    Ok(conn
        .query_row(
            &sql,
            params![
                values.name,
                values.folder,
                profile.format.as_str(),
                profile.aac_kbps,
                i64::from(profile.cd_resolution),
                profile.path_template,
                i64::from(profile.embed_covers),
                profile.cover_px,
                i64::from(profile.cover_file),
                id,
            ],
            library,
        )
        .optional()?)
}

/// Records when an external library's sync ended, in epoch seconds.
pub fn set_last_synced(conn: &Connection, id: i64, at: i64) -> Result<()> {
    conn.execute(
        r#"update "external_libraries" set "last_synced_at" = ? where "id" = ?"#,
        params![at, id],
    )?;
    Ok(())
}

/// Deletes an external library, and its rules with it. An unknown id
/// deletes nothing.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(r#"delete from "external_libraries" where "id" = ?"#, [id])?;
    Ok(())
}

fn rule(row: &Row) -> rusqlite::Result<(i64, Rule)> {
    let target = if let Some(release) = row.get::<_, Option<i64>>(3)? {
        RuleTarget::Release(release)
    } else if let Some(artist) = row.get::<_, Option<i64>>(4)? {
        RuleTarget::Artist(artist)
    } else if let Some(track) = row.get::<_, Option<i64>>(5)? {
        RuleTarget::Track(track)
    } else {
        let mut found = None;
        for (index, kind) in [(6, TagType::Text), (7, TagType::Number), (8, TagType::Date)] {
            if let Some(tag_id) = row.get::<_, Option<i64>>(index)? {
                found = Some((kind, tag_id));
                break;
            }
        }
        // The database makes each rule name exactly one thing
        let (kind, tag_id) = found.ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(8, "date_tag_id".into(), rusqlite::types::Type::Null)
        })?;
        RuleTarget::Tag {
            kind,
            tag_id,
            equals: row.get(9)?,
            min: row.get(10)?,
            max: row.get(11)?,
        }
    };
    Ok((
        row.get(1)?,
        Rule {
            id: row.get(0)?,
            exclude: row.get::<_, i64>(2)? == 1,
            target,
        },
    ))
}

/// Rules by external library, in the order they were added.
pub fn rules_of(conn: &Connection, library_ids: &[i64]) -> Result<HashMap<i64, Vec<Rule>>> {
    let mut rows = Vec::new();
    for chunk in chunks(library_ids) {
        let sql = format!(
            r#"select {RULE_COLUMNS} from "external_library_rules"
            where "external_library_id" in ({}) order by "external_library_id", "id""#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), rule)? {
            rows.push(row?);
        }
    }
    Ok(group_by(rows, |(library, _)| *library, |(_, rule)| rule))
}

/// Adds a rule, and gives its id.
pub fn insert_rule(conn: &Connection, library_id: i64, values: &RuleValues) -> Result<i64> {
    let (mut release, mut artist, mut track) = (None, None, None);
    let mut tags: [Option<i64>; 3] = [None; 3];
    let (mut equals, mut min, mut max) = (None, None, None);
    match &values.target {
        RuleTarget::Release(id) => release = Some(*id),
        RuleTarget::Artist(id) => artist = Some(*id),
        RuleTarget::Track(id) => track = Some(*id),
        RuleTarget::Tag {
            kind,
            tag_id,
            equals: e,
            min: low,
            max: high,
        } => {
            let index = match kind {
                TagType::Text => 0,
                TagType::Number => 1,
                TagType::Date => 2,
            };
            tags[index] = Some(*tag_id);
            (equals, min, max) = (e.clone(), low.clone(), high.clone());
        }
    }
    conn.execute(
        r#"insert into "external_library_rules" ("external_library_id", "exclude", "release_id", "artist_id", "track_id", "text_tag_id", "real_number_tag_id", "date_tag_id", "tag_equals", "tag_min", "tag_max")
        values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        params![
            library_id,
            i64::from(values.exclude),
            release,
            artist,
            track,
            tags[0],
            tags[1],
            tags[2],
            equals,
            min,
            max
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Replaces an external library's rules, in order.
pub fn set_rules(conn: &Connection, library_id: i64, rules: &[RuleValues]) -> Result<()> {
    conn.execute(
        r#"delete from "external_library_rules" where "external_library_id" = ?"#,
        [library_id],
    )?;
    for values in rules {
        insert_rule(conn, library_id, values)?;
    }
    Ok(())
}

pub fn delete_rule(conn: &Connection, rule_id: i64) -> Result<()> {
    conn.execute(
        r#"delete from "external_library_rules" where "id" = ?"#,
        [rule_id],
    )?;
    Ok(())
}

/// The tracks an external library's rules come to: those a rule includes,
/// and no rule leaves out. Tag value rules are given as the filters they
/// are. With `within`, only among those tracks, in no particular order;
/// otherwise every one, by release title ignoring case, then release, disc
/// and number.
pub fn members(
    conn: &Connection,
    library_id: i64,
    tags: &[TagFilter],
    within: Option<&[i64]>,
) -> Result<Vec<i64>> {
    // An id isn't text, so it can go in the SQL as it is
    let named = |exclude: i64, column: &str| {
        format!(
            r#"select "{column}" from "external_library_rules"
            where "external_library_id" = {library_id} and "exclude" = {exclude} and "{column}" is not null"#
        )
    };
    let mut included = Condition::any()
        .add(Expr::cust(format!(
            r#""tracks"."release_id" in ({})"#,
            named(0, "release_id")
        )))
        .add(Expr::cust(format!(
            r#""tracks"."release_id" in (select "release_id" from "artists_releases" where "artist_id" in ({}))"#,
            named(0, "artist_id")
        )))
        .add(Expr::cust(format!(
            r#""tracks"."id" in ({})"#,
            named(0, "track_id")
        )));
    for filter in tags {
        included = included.add(tag_value(filter));
    }
    let mut select = Query::select();
    select
        .expr(Expr::cust(r#""tracks"."id""#))
        .from("tracks")
        .inner_join(
            "releases",
            Expr::cust(r#""releases"."id" = "tracks"."release_id""#),
        )
        .cond_where(included)
        .and_where(Expr::cust(format!(
            r#""tracks"."release_id" not in ({})"#,
            named(1, "release_id")
        )))
        .and_where(Expr::cust(format!(
            r#""tracks"."id" not in ({})"#,
            named(1, "track_id")
        )));
    match within {
        None => {
            select
                .order_by_expr(
                    Expr::cust(r#""releases"."title" collate nocase"#),
                    Order::Asc,
                )
                .order_by_expr(Expr::cust(r#""releases"."id""#), Order::Asc)
                .order_by_expr(Expr::cust(r#""tracks"."cd_number""#), Order::Asc)
                .order_by_expr(Expr::cust(r#""tracks"."track_number""#), Order::Asc);
            ids(conn, &select)
        }
        Some(within) => {
            let mut found = Vec::new();
            for chunk in chunks(within) {
                let mut scoped = select.clone();
                scoped.and_where(Expr::cust(format!(
                    r#""tracks"."id" in ({})"#,
                    joined(chunk)
                )));
                found.extend(ids(conn, &scoped)?);
            }
            Ok(found)
        }
    }
}

/// Which of these tracks a tag value rule includes, on their release or on
/// themselves.
pub fn tag_value_matches(
    conn: &Connection,
    filter: &TagFilter,
    track_ids: &[i64],
) -> Result<HashSet<i64>> {
    let mut found = HashSet::new();
    for chunk in chunks(track_ids) {
        let mut select = Query::select();
        select
            .expr(Expr::cust(r#""tracks"."id""#))
            .from("tracks")
            .inner_join(
                "releases",
                Expr::cust(r#""releases"."id" = "tracks"."release_id""#),
            )
            .cond_where(Condition::any().add(tag_value(filter)))
            .and_where(Expr::cust(format!(
                r#""tracks"."id" in ({})"#,
                joined(chunk)
            )));
        found.extend(ids(conn, &select)?);
    }
    Ok(found)
}

/// A track's size and length, and its file's path, for estimates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackFileFacts {
    pub track_id: i64,
    pub release_id: i64,
    pub duration_ms: Option<i64>,
    pub path: String,
    pub size_bytes: i64,
    pub sha256: String,
}

/// Each of these tracks' file facts, in no particular order.
pub fn file_facts(conn: &Connection, track_ids: &[i64]) -> Result<Vec<TrackFileFacts>> {
    let mut found = Vec::new();
    for chunk in chunks(track_ids) {
        let sql = format!(
            r#"select "tracks"."id", "tracks"."release_id", "tracks"."duration_ms", "files"."path", "files"."size_bytes", "files"."sha256"
            from "tracks" inner join "files" on "files"."id" = "tracks"."file_id"
            where "tracks"."id" in ({})"#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok(TrackFileFacts {
                track_id: row.get(0)?,
                release_id: row.get(1)?,
                duration_ms: row.get(2)?,
                path: row.get(3)?,
                size_bytes: row.get(4)?,
                sha256: row.get(5)?,
            })
        })? {
            found.push(row?);
        }
    }
    Ok(found)
}

/// A tag value rule: its value on the release, or on the track.
fn tag_value(filter: &TagFilter) -> Condition {
    Condition::any()
        .add(tag_matches(TagOwner::Release, filter))
        .add(tag_matches(TagOwner::Track, filter))
}

fn ids(conn: &Connection, select: &sea_query::SelectStatement) -> Result<Vec<i64>> {
    let (sql, values) = select.build(SqliteQueryBuilder);
    let mut statement = conn.prepare(&sql)?;
    let ids = statement
        .query_map(params_from_iter(bind(values)), |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

/// Ids, for an `in (…)` list. They're numbers, so they need no binding.
fn joined(ids: &[i64]) -> String {
    ids.iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
