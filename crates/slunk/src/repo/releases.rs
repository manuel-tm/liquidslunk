//! Releases, and the rows that link them to artists, labels, formats and
//! media: slunk's releases repository. Their alternate titles are
//! `titles.rs`, and their tags `tags.rs`.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use sea_query::{Expr, Order, Query, SelectStatement, SqliteQueryBuilder};

use super::helpers::{bind, chunks, group_by, placeholders};
use super::tags::tag_matches;
use super::titles::{self, TitleOwner, title_matches};
use crate::Result;
use crate::catalog::{
    ArtistLink, DiscTitle, Release, ReleaseFilters, ReleaseLink, ReleaseQuery, ReleaseSort,
    ReleaseValues,
};
use crate::dates::DatePrecision;
use crate::tagging::TagOwner;
use crate::text::trim;

const COLUMNS: &str = r#""id", "title", "title_language", "release_date", "release_date_precision", "catalog_number", "cover_picture", "various_artists", "inserted_at", "updated_at""#;

/// A link's table, its column, and whether its rows have a position. Formats
/// and media have none: they're listed by their own id, as slunk's query
/// listed them, ordering by `id`, which SQLite read as its alias for the
/// format's or medium's id.
fn link_table(link: ReleaseLink) -> (&'static str, &'static str, bool) {
    match link {
        ReleaseLink::Artists => ("artists_releases", "artist_id", true),
        ReleaseLink::Labels => ("labels_releases", "label_id", true),
        ReleaseLink::Formats => ("release_formats_releases", "format_id", false),
        ReleaseLink::Media => ("release_media_releases", "medium_id", false),
    }
}

fn release(row: &Row) -> rusqlite::Result<Release> {
    let precision: Option<String> = row.get(4)?;
    Ok(Release {
        id: row.get(0)?,
        title: row.get(1)?,
        title_language: row.get(2)?,
        release_date: row.get(3)?,
        release_date_precision: precision.as_deref().and_then(DatePrecision::named),
        catalog_number: row.get(5)?,
        cover_picture: row.get(6)?,
        various_artists: row.get::<_, i64>(7)? == 1,
        inserted_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

/// The releases a list's search and filters keep.
fn filtered(filters: &ReleaseFilters) -> SelectStatement {
    let mut select = Query::select();
    select.from("releases");
    if let Some(search) = &filters.search {
        select.and_where(title_matches(TitleOwner::Release, search));
    }
    for (link, id) in [
        (ReleaseLink::Formats, filters.format),
        (ReleaseLink::Media, filters.medium),
        (ReleaseLink::Labels, filters.label),
        (ReleaseLink::Artists, filters.artist),
    ] {
        if let Some(id) = id {
            let (table, column, _) = link_table(link);
            select.and_where(Expr::cust_with_values(
                format!(
                    r#"exists (select 1 from "{table}" where "release_id" = "releases"."id" and "{column}" = ?)"#
                ),
                [id],
            ));
        }
    }
    for filter in &filters.tags {
        select.and_where(tag_matches(TagOwner::Release, filter));
    }
    select
}

/// A page, and how many match in all.
pub fn list(conn: &Connection, query: &ReleaseQuery) -> Result<(Vec<Release>, i64)> {
    let select = filtered(&query.filters);
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
    let mut select = select;
    select.expr(Expr::cust(COLUMNS));
    match query.sort.column {
        // Undated releases go last either way
        Some(ReleaseSort::Released) => {
            select
                .order_by_expr(Expr::cust(r#""release_date" is null"#), Order::Asc)
                .order_by("release_date", order.clone());
        }
        Some(ReleaseSort::Title) => {
            select.order_by_expr(Expr::cust(r#""title" collate nocase"#), order.clone());
        }
        None => {}
    }
    let (sql, values) = select
        .order_by("id", order)
        .limit(query.paging.limit as u64)
        .offset(query.paging.offset as u64)
        .build(SqliteQueryBuilder);
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map(params_from_iter(bind(values)), release)?
        .collect::<rusqlite::Result<_>>()?;
    Ok((rows, total))
}

/// Every matching release's id, newest first, to select them all.
pub fn ids(conn: &Connection, filters: &ReleaseFilters) -> Result<Vec<i64>> {
    let (sql, values) = filtered(filters)
        .column("id")
        .order_by("id", Order::Desc)
        .build(SqliteQueryBuilder);
    let mut statement = conn.prepare(&sql)?;
    let ids = statement
        .query_map(params_from_iter(bind(values)), |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<Release>> {
    let sql = format!(r#"select {COLUMNS} from "releases" where "id" = ?"#);
    Ok(conn.query_row(&sql, [id], release).optional()?)
}

/// The releases of these ids that exist, in no particular order.
pub fn get_many(conn: &Connection, ids: &[i64]) -> Result<Vec<Release>> {
    let mut found = Vec::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select {COLUMNS} from "releases" where "id" in ({})"#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), release)? {
            found.push(row?);
        }
    }
    Ok(found)
}

/// Releases with this title, or this alternate title, ignoring case, in no
/// particular order: the import checks it so a release isn't imported
/// twice. The title is trimmed as JavaScript trims.
pub fn find_by_title(conn: &Connection, title: &str) -> Result<Vec<Release>> {
    let sql = format!(
        r#"select {COLUMNS} from "releases" where title = ?1 collate nocase or id in (
          select release_id from release_alternate_titles
          where title = ?1 collate nocase
        )"#
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map([trim(title)], release)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// A new release, with no cover.
pub fn insert(conn: &Connection, values: &ReleaseValues) -> Result<Release> {
    let sql = format!(
        r#"insert into "releases" ("title", "title_language", "release_date", "release_date_precision", "catalog_number", "cover_picture", "various_artists")
        values (?, ?, ?, ?, ?, null, ?) returning {COLUMNS}"#
    );
    Ok(conn.query_row(
        &sql,
        params![
            values.title,
            values.title_language,
            values.release_date,
            values.release_date_precision.map(DatePrecision::as_str),
            values.catalog_number,
            i64::from(values.various_artists)
        ],
        release,
    )?)
}

/// Saves a release's details, and its title's language unless
/// `keep_language`, and sets `updated_at`. `None` if there's no such
/// release.
pub fn update(
    conn: &Connection,
    id: i64,
    values: &ReleaseValues,
    keep_language: bool,
) -> Result<Option<Release>> {
    let language = if keep_language {
        ""
    } else {
        r#""title_language" = :language, "#
    };
    let sql = format!(
        r#"update "releases" set "title" = :title, {language}"release_date" = :date,
          "release_date_precision" = :precision, "catalog_number" = :catalog,
          "various_artists" = :various, "updated_at" = unixepoch()
        where "id" = :id returning {COLUMNS}"#
    );
    let mut statement = conn.prepare(&sql)?;
    let precision = values.release_date_precision.map(DatePrecision::as_str);
    let various = i64::from(values.various_artists);
    let mut params: Vec<(&str, &dyn rusqlite::ToSql)> = vec![
        (":title", &values.title),
        (":date", &values.release_date),
        (":precision", &precision),
        (":catalog", &values.catalog_number),
        (":various", &various),
        (":id", &id),
    ];
    if !keep_language {
        params.push((":language", &values.title_language));
    }
    Ok(statement.query_row(&params[..], release).optional()?)
}

/// Sets only the cover, and `updated_at`. `None` if there's no such release.
pub fn set_cover(conn: &Connection, id: i64, cover: Option<i64>) -> Result<Option<Release>> {
    let sql = format!(
        r#"update "releases" set "cover_picture" = ?, "updated_at" = unixepoch() where "id" = ? returning {COLUMNS}"#
    );
    Ok(conn
        .query_row(&sql, params![cover, id], release)
        .optional()?)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(r#"delete from "releases" where "id" = ?"#, [id])?;
    Ok(())
}

/// Linked ids by release, in display order.
pub fn links_of(
    conn: &Connection,
    link: ReleaseLink,
    release_ids: &[i64],
) -> Result<HashMap<i64, Vec<i64>>> {
    let (table, column, positioned) = link_table(link);
    let order = if positioned { "position" } else { column };
    let mut rows = Vec::new();
    for chunk in chunks(release_ids) {
        let sql = format!(
            r#"select "release_id", "{column}" from "{table}"
            where "release_id" in ({}) order by "release_id", "{order}""#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })? {
            rows.push(row?);
        }
    }
    Ok(group_by(rows, |(release, _)| *release, |(_, id)| id))
}

/// Replaces a release's labels, formats or media. Their order is the
/// display order.
pub fn set_links(conn: &Connection, link: ReleaseLink, release_id: i64, ids: &[i64]) -> Result<()> {
    let (table, column, positioned) = link_table(link);
    conn.execute(
        &format!(r#"delete from "{table}" where "release_id" = ?"#),
        [release_id],
    )?;
    for (position, id) in ids.iter().enumerate() {
        if positioned {
            conn.execute(
                &format!(
                    r#"insert into "{table}" ("release_id", "{column}", "position") values (?, ?, ?)"#
                ),
                params![release_id, id, position as i64],
            )?;
        } else {
            conn.execute(
                &format!(r#"insert into "{table}" ("release_id", "{column}") values (?, ?)"#),
                params![release_id, id],
            )?;
        }
    }
    Ok(())
}

/// Release artists by release, in display order, as credited.
pub fn artists_of(conn: &Connection, release_ids: &[i64]) -> Result<HashMap<i64, Vec<ArtistLink>>> {
    let mut rows = Vec::new();
    for chunk in chunks(release_ids) {
        let sql = format!(
            r#"select "release_id", "artist_id", "credited_name_id" from "artists_releases"
            where "release_id" in ({}) order by "release_id", "position""#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                ArtistLink {
                    artist_id: row.get(1)?,
                    credited_name_id: row.get(2)?,
                },
            ))
        })? {
            rows.push(row?);
        }
    }
    Ok(group_by(rows, |(release, _)| *release, |(_, link)| link))
}

/// Replaces a release's artists. Their order is the display order.
pub fn set_artists(conn: &Connection, release_id: i64, artists: &[ArtistLink]) -> Result<()> {
    conn.execute(
        r#"delete from "artists_releases" where "release_id" = ?"#,
        [release_id],
    )?;
    for (position, link) in artists.iter().enumerate() {
        conn.execute(
            r#"insert into "artists_releases" ("release_id", "artist_id", "credited_name_id", "position")
            values (?, ?, ?, ?)"#,
            params![
                release_id,
                link.artist_id,
                link.credited_name_id,
                position as i64
            ],
        )?;
    }
    Ok(())
}

/// Disc titles by release, by disc number, with their alternate titles.
pub fn disc_titles_of(
    conn: &Connection,
    release_ids: &[i64],
) -> Result<HashMap<i64, Vec<DiscTitle>>> {
    let mut rows: Vec<(i64, i64, DiscTitle)> = Vec::new();
    for chunk in chunks(release_ids) {
        let sql = format!(
            r#"select "id", "release_id", "cd_number", "title", "language" from "release_disc_titles"
            where "release_id" in ({}) order by "release_id", "cd_number""#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                DiscTitle {
                    cd_number: row.get(2)?,
                    title: row.get(3)?,
                    language: row.get(4)?,
                    alternate_titles: Vec::new(),
                },
            ))
        })? {
            rows.push(row?);
        }
    }
    let ids: Vec<i64> = rows.iter().map(|(id, ..)| *id).collect();
    let mut alternates = titles::alternate_titles_of(conn, TitleOwner::Disc, &ids)?;
    let rows: Vec<(i64, DiscTitle)> = rows
        .into_iter()
        .map(|(id, release, mut disc)| {
            disc.alternate_titles = alternates.remove(&id).unwrap_or_default();
            (release, disc)
        })
        .collect();
    Ok(group_by(rows, |(release, _)| *release, |(_, disc)| disc))
}

/// Replaces a release's disc titles, with their alternate titles. Run it
/// after its tracks are saved: a title needs a track on its disc.
pub fn set_disc_titles(conn: &Connection, release_id: i64, titles: &[DiscTitle]) -> Result<()> {
    // A disc's alternate titles go with its title (CASCADE)
    conn.execute(
        r#"delete from "release_disc_titles" where "release_id" = ?"#,
        [release_id],
    )?;
    for disc in titles {
        let id: i64 = conn.query_row(
            r#"insert into "release_disc_titles" ("release_id", "cd_number", "title", "language")
            values (?, ?, ?, ?) returning "id""#,
            params![release_id, disc.cd_number, disc.title, disc.language],
            |row| row.get(0),
        )?;
        titles::set_alternate_titles(conn, TitleOwner::Disc, id, &disc.alternate_titles)?;
    }
    Ok(())
}

/// Releases with no artist, by title, for the library check.
pub fn without_artists(conn: &Connection) -> Result<Vec<Release>> {
    let sql = format!(
        r#"select {COLUMNS} from "releases" where not exists (
          select 1 from artists_releases where release_id = releases.id
        ) order by title collate nocase"#
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map([], release)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// How many tracks the release has.
pub fn track_count(conn: &Connection, id: i64) -> Result<i64> {
    Ok(conn.query_row(
        r#"select count(*) from "tracks" where "release_id" = ?"#,
        [id],
        |row| row.get(0),
    )?)
}
