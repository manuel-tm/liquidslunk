//! Artists and labels: slunk's named repository. They have the same columns,
//! so they share it, except that only artists' alternate names can be real
//! names.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use sea_query::{Expr, ExprTrait, LikeExpr, Order, Query, SqliteQueryBuilder};

use super::helpers::{bind, chunks, group_by, placeholders};
use crate::catalog::{
    AlternateName, AlternateNameValues, NameMatch, NameQuery, NameSort, Named, NamedValues,
};
use crate::paging::like_pattern;
use crate::{Error, Result};

/// The artists' tables or the labels'.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Table {
    Artists,
    Labels,
}

impl Table {
    pub(crate) fn table(self) -> &'static str {
        match self {
            Table::Artists => "artists",
            Table::Labels => "labels",
        }
    }

    pub(crate) fn names(self) -> &'static str {
        match self {
            Table::Artists => "artist_alternate_names",
            Table::Labels => "label_alternate_names",
        }
    }

    pub(crate) fn owner(self) -> &'static str {
        match self {
            Table::Artists => "artist_id",
            Table::Labels => "label_id",
        }
    }

    fn has_real_names(self) -> bool {
        self == Table::Artists
    }
}

const COLUMNS: &str = r#""id", "name", "name_language", "picture", "inserted_at", "updated_at""#;

/// `COLUMNS`, of the table joined as "owners".
const JOINED_COLUMNS: &str = r#""owners"."id", "owners"."name", "owners"."name_language", "owners"."picture", "owners"."inserted_at", "owners"."updated_at""#;

/// A page, and how many match in all. The search matches the main name or an
/// alternate name.
pub fn list(conn: &Connection, table: Table, query: &NameQuery) -> Result<(Vec<Named>, i64)> {
    let mut select = Query::select();
    select.from(table.table());
    if let Some(search) = &query.search {
        let like = || LikeExpr::new(like_pattern(search)).escape('\\');
        let alternates = Query::select()
            .column(table.owner())
            .from(table.names())
            .and_where(Expr::col("name").like(like()))
            .take();
        select.and_where(
            Expr::col("name")
                .like(like())
                .or(Expr::col("id").in_subquery(alternates)),
        );
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
    let first = match query.sort.column {
        Some(NameSort::Name) => Expr::cust(r#""name" collate nocase"#),
        None => Expr::col("id"),
    };
    let (sql, values) = select
        .expr(Expr::cust(COLUMNS))
        .order_by_expr(first, order.clone())
        .order_by("id", order)
        .limit(query.paging.limit as u64)
        .offset(query.paging.offset as u64)
        .build(SqliteQueryBuilder);
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map(params_from_iter(bind(values)), named)?
        .collect::<rusqlite::Result<_>>()?;
    Ok((rows, total))
}

pub fn get(conn: &Connection, table: Table, id: i64) -> Result<Option<Named>> {
    let sql = format!(
        r#"select {COLUMNS} from "{}" where "id" = ?"#,
        table.table()
    );
    Ok(conn.query_row(&sql, [id], named).optional()?)
}

pub fn insert(conn: &Connection, table: Table, values: &NamedValues) -> Result<Named> {
    let sql = format!(
        r#"insert into "{}" ("name", "name_language", "picture") values (?, ?, ?) returning {COLUMNS}"#,
        table.table()
    );
    Ok(conn.query_row(
        &sql,
        params![values.name, values.name_language, values.picture],
        named,
    )?)
}

/// The artists or labels of these ids that exist, in no particular order.
pub fn get_many(conn: &Connection, table: Table, ids: &[i64]) -> Result<Vec<Named>> {
    let mut found = Vec::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select {COLUMNS} from "{}" where "id" in ({})"#,
            table.table(),
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), named)? {
            found.push(row?);
        }
    }
    Ok(found)
}

/// The artists or labels whose main name or an alternate name is one of
/// these, ignoring case as SQLite's NOCASE does: a match for each such name,
/// main names first, in no particular order.
pub fn find_by_names(conn: &Connection, table: Table, wanted: &[String]) -> Result<Vec<NameMatch>> {
    let mut unique: Vec<&String> = Vec::new();
    for name in wanted {
        if !unique.contains(&name) {
            unique.push(name);
        }
    }
    let (owners, names, owner) = (table.table(), table.names(), table.owner());
    let real_name = if table.has_real_names() {
        r#""names"."real_name""#
    } else {
        "0"
    };
    let mut matches = Vec::new();
    for chunk in chunks(&unique) {
        let list = placeholders(chunk.len());
        let sql =
            format!(r#"select {COLUMNS} from "{owners}" where name collate nocase in ({list})"#);
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), named)? {
            matches.push(NameMatch {
                item: row?,
                alternate: None,
            });
        }
        let sql = format!(
            r#"select {JOINED_COLUMNS}, "names"."id", "names"."name", "names"."language", {real_name}, "names"."note"
            from "{names}" as "names" inner join "{owners}" as "owners" on "owners"."id" = "names"."{owner}"
            where "names"."name" collate nocase in ({list})"#
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok(NameMatch {
                item: named(row)?,
                alternate: Some(AlternateName {
                    id: row.get(6)?,
                    name: row.get(7)?,
                    language: row.get(8)?,
                    real_name: row.get::<_, i64>(9)? == 1,
                    note: row.get(10)?,
                }),
            })
        })? {
            matches.push(row?);
        }
    }
    Ok(matches)
}

/// Changes the name, its language and the picture, and sets `updated_at`.
/// `None` if there's no such row.
pub fn update(
    conn: &Connection,
    table: Table,
    id: i64,
    values: &NamedValues,
) -> Result<Option<Named>> {
    let sql = format!(
        r#"update "{}" set "name" = ?, "name_language" = ?, "picture" = ?, "updated_at" = unixepoch() where "id" = ? returning {COLUMNS}"#,
        table.table()
    );
    Ok(conn
        .query_row(
            &sql,
            params![values.name, values.name_language, values.picture, id],
            named,
        )
        .optional()?)
}

/// Changes only the picture, and sets `updated_at`.
pub fn set_picture(
    conn: &Connection,
    table: Table,
    id: i64,
    file_id: i64,
) -> Result<Option<Named>> {
    let sql = format!(
        r#"update "{}" set "picture" = ?, "updated_at" = unixepoch() where "id" = ? returning {COLUMNS}"#,
        table.table()
    );
    Ok(conn
        .query_row(&sql, params![file_id, id], named)
        .optional()?)
}

pub fn delete(conn: &Connection, table: Table, id: i64) -> Result<()> {
    conn.execute(
        &format!(r#"delete from "{}" where "id" = ?"#, table.table()),
        [id],
    )?;
    Ok(())
}

/// Alternate names by owner, in order.
pub fn alternate_names_of(
    conn: &Connection,
    table: Table,
    ids: &[i64],
) -> Result<HashMap<i64, Vec<AlternateName>>> {
    let owner = table.owner();
    let real_name = if table.has_real_names() {
        r#""real_name""#
    } else {
        "0"
    };
    let mut rows = Vec::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select "{owner}", "id", "name", "language", {real_name}, "note" from "{}" where "{owner}" in ({}) order by "{owner}", "position""#,
            table.names(),
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                AlternateName {
                    id: row.get(1)?,
                    name: row.get(2)?,
                    language: row.get(3)?,
                    real_name: row.get::<_, i64>(4)? == 1,
                    note: row.get(5)?,
                },
            ))
        })? {
            rows.push(row?);
        }
    }
    Ok(group_by(rows, |(owner, _)| *owner, |(_, name)| name))
}

/// Replaces an owner's alternate names, in order. A name with an id keeps
/// its row, the others are added, and the names left out are deleted. Run it
/// in a transaction.
pub fn set_alternate_names(
    conn: &Connection,
    table: Table,
    id: i64,
    values: &[AlternateNameValues],
) -> Result<()> {
    let names = table.names();
    let owner = table.owner();
    let mut statement = conn.prepare(&format!(
        r#"select "id", "name" from "{names}" where "{owner}" = ?"#
    ))?;
    let existing: Vec<(i64, String)> = statement
        .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let current: HashMap<i64, &str> = existing
        .iter()
        .map(|(id, name)| (*id, name.as_str()))
        .collect();
    let mut kept = HashSet::new();
    for value in values {
        let Some(name_id) = value.id else { continue };
        if !current.contains_key(&name_id) {
            return Err(Error::Invalid(format!(
                "Name {name_id} isn't one of its alternate names"
            )));
        }
        kept.insert(name_id);
    }
    let removed: Vec<i64> = existing
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| !kept.contains(id))
        .collect();
    for chunk in chunks(&removed) {
        conn.execute(
            &format!(
                r#"delete from "{names}" where "id" in ({})"#,
                placeholders(chunk.len())
            ),
            params_from_iter(chunk),
        )?;
    }
    // Names can trade places between rows, so renamed rows first move clear
    // of the unique (owner, name) index, as tracks are renumbered. U+FFFF is
    // never typed, and passes the trim CHECK.
    for value in values {
        if let Some(name_id) = value.id
            && current.get(&name_id) != Some(&value.name.as_str())
        {
            conn.execute(
                &format!(r#"update "{names}" set "name" = ? where "id" = ?"#),
                params![format!("\u{ffff}{name_id}"), name_id],
            )?;
        }
    }
    for (position, value) in values.iter().enumerate() {
        let position = position as i64;
        let real_name = i64::from(value.real_name);
        match (value.id, table.has_real_names()) {
            (Some(name_id), true) => conn.execute(
                &format!(r#"update "{names}" set "name" = ?, "language" = ?, "note" = ?, "position" = ?, "real_name" = ? where "id" = ?"#),
                params![value.name, value.language, value.note, position, real_name, name_id],
            )?,
            (Some(name_id), false) => conn.execute(
                &format!(r#"update "{names}" set "name" = ?, "language" = ?, "note" = ?, "position" = ? where "id" = ?"#),
                params![value.name, value.language, value.note, position, name_id],
            )?,
            (None, true) => conn.execute(
                &format!(r#"insert into "{names}" ("name", "language", "note", "position", "real_name", "{owner}") values (?, ?, ?, ?, ?, ?)"#),
                params![value.name, value.language, value.note, position, real_name, id],
            )?,
            (None, false) => conn.execute(
                &format!(r#"insert into "{names}" ("name", "language", "note", "position", "{owner}") values (?, ?, ?, ?, ?)"#),
                params![value.name, value.language, value.note, position, id],
            )?,
        };
    }
    Ok(())
}

fn named(row: &Row<'_>) -> rusqlite::Result<Named> {
    Ok(Named {
        id: row.get(0)?,
        name: row.get(1)?,
        name_language: row.get(2)?,
        picture: row.get(3)?,
        inserted_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}
