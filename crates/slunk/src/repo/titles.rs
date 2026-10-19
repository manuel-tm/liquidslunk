//! Alternate titles of releases, tracks and discs, which have a table each,
//! with the same columns.

use std::collections::HashMap;

use rusqlite::{Connection, params, params_from_iter};
use sea_query::Expr;

use super::helpers::{chunks, group_by, placeholders};
use crate::Result;
use crate::catalog::AlternateTitle;
use crate::paging::like_pattern;

/// What has alternate titles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleOwner {
    Release,
    Track,
    /// A disc's title, in `release_disc_titles`
    Disc,
}

impl TitleOwner {
    fn table(self) -> &'static str {
        match self {
            TitleOwner::Release => "release_alternate_titles",
            TitleOwner::Track => "track_alternate_titles",
            TitleOwner::Disc => "release_disc_alternate_titles",
        }
    }

    fn column(self) -> &'static str {
        match self {
            TitleOwner::Release => "release_id",
            TitleOwner::Track => "track_id",
            TitleOwner::Disc => "disc_title_id",
        }
    }
}

/// Alternate titles by owner, in order.
pub fn alternate_titles_of(
    conn: &Connection,
    owner: TitleOwner,
    ids: &[i64],
) -> Result<HashMap<i64, Vec<AlternateTitle>>> {
    let (table, column) = (owner.table(), owner.column());
    let mut rows = Vec::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select "{column}", "title", "language", "printed" from "{table}"
            where "{column}" in ({}) order by "{column}", "position""#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                AlternateTitle {
                    title: row.get(1)?,
                    language: row.get(2)?,
                    printed: row.get::<_, i64>(3)? == 1,
                },
            ))
        })? {
            rows.push(row?);
        }
    }
    Ok(group_by(rows, |(owner, _)| *owner, |(_, title)| title))
}

/// Replaces an owner's alternate titles, in order.
pub fn set_alternate_titles(
    conn: &Connection,
    owner: TitleOwner,
    id: i64,
    titles: &[AlternateTitle],
) -> Result<()> {
    let (table, column) = (owner.table(), owner.column());
    conn.execute(
        &format!(r#"delete from "{table}" where "{column}" = ?"#),
        [id],
    )?;
    for (position, title) in titles.iter().enumerate() {
        conn.execute(
            &format!(
                r#"insert into "{table}" ("{column}", "title", "language", "printed", "position")
                values (?, ?, ?, ?, ?)"#
            ),
            params![
                id,
                title.title,
                title.language,
                i64::from(title.printed),
                position as i64
            ],
        )?;
    }
    Ok(())
}

/// The releases or tracks whose title or any alternate title contains the
/// search, ignoring case, for a list's `q`.
pub fn title_matches(owner: TitleOwner, search: &str) -> Expr {
    let pattern = like_pattern(search);
    Expr::cust_with_values(
        format!(
            r#"("title" like ? escape '\' or "id" in (
            select "{}" from "{}" where "title" like ? escape '\'
          ))"#,
            owner.column(),
            owner.table()
        ),
        [pattern.clone(), pattern],
    )
}
