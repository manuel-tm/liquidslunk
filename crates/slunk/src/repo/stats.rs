//! Figures across the catalog: slunk's stats repository.

use rusqlite::Connection;

use crate::Result;

/// Language tags in use on names and titles, the most used first.
pub fn languages(conn: &Connection) -> Result<Vec<String>> {
    let mut statement = conn.prepare(
        "select language from (
           select name_language as language from artists
           union all select name_language from labels
           union all select language from artist_alternate_names
           union all select language from label_alternate_names
           union all select title_language from releases
           union all select title_language from tracks
           union all select language from release_disc_titles
           union all select language from release_alternate_titles
           union all select language from track_alternate_titles
           union all select language from release_disc_alternate_titles
         )
         where language is not null
         group by language
         order by count(*) desc, language",
    )?;
    let languages = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(languages)
}
