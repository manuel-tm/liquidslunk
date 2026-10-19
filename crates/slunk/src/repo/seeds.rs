//! Sample data for development (`npm run db:seed`, or `npm run db:reset` to
//! start over): five releases with their artists, labels, formats and media,
//! and the tags Genre and Score. Three are the examples of alternate names
//! and titles: "ALIKA" and the real name "Alika Milova", "福居良" in
//! Japanese, a misprint, "シーナリィ" printed on Scenery, and a translation.
//! There are no files or tracks, which need real files on disk. The rows go
//! in directly, as slunk's seed put them, so their ids are the same.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};

use crate::Result;
use crate::dates::parse_date;

/// What a seed added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seeded {
    pub releases: usize,
    pub artists: usize,
    pub labels: usize,
    pub tags: usize,
}

const ARTISTS: [&str; 13] = [
    "Drake",
    "Majid Jordan",
    "The High School Musical Cast",
    "Zac Efron",
    "Vanessa Anne Hudgens",
    // With the alternate names below
    "Alika",
    "Ryo Fukui",
    "Ryo Fukui Trio",
    "Jimmy Van Heusen",
    "Ann Ronell",
    "Joseph Kosma",
    "Masataka Ito",
    "Teruo Isono",
];

const LABELS: [&str; 6] = [
    "October's Very Own",
    "Young Money",
    "Cash Money",
    "Republic Records",
    "Walt Disney Records",
    "Nadja",
];

/// An alternate name: its artist, the name, its language, whether it's a
/// real name, and a note.
type SeedName = (
    &'static str,
    &'static str,
    Option<&'static str>,
    bool,
    Option<&'static str>,
);

/// Alternate names: "ALIKA" differs from the main name only by case, and "A.
/// Ronnell" is misprinted on Scenery.
const ALTERNATE_NAMES: [SeedName; 10] = [
    ("Alika", "Alika Milova", None, true, None),
    ("Alika", "ALIKA", None, false, None),
    ("Ryo Fukui", "福居良", Some("ja"), false, None),
    ("Ryo Fukui", "R. Fukui", None, false, None),
    ("Ryo Fukui Trio", "福居良トリオ", Some("ja"), false, None),
    ("Jimmy Van Heusen", "J. Van Heusen", None, false, None),
    ("Ann Ronell", "A. Ronnell", None, false, Some("misprint")),
    ("Joseph Kosma", "J. Kosma", None, false, None),
    ("Masataka Ito", "いとう・まさたか", Some("ja"), false, None),
    ("Teruo Isono", "いソノてルヲ", Some("ja"), false, None),
];

/// A release's own columns, and what it links to, by name.
struct SeedRelease<'a> {
    title: &'a str,
    title_language: Option<&'a str>,
    /// As typed: its precision is how much of it there is
    released: &'a str,
    catalog_number: Option<&'a str>,
    various_artists: bool,
    artists: &'a [&'a str],
    labels: &'a [&'a str],
    formats: &'a [&'a str],
    media: &'a [&'a str],
}

/// Loads the sample data in one transaction, unless the database has
/// releases already: then it's `None`, and nothing changes.
pub fn seed(conn: &mut Connection) -> Result<Option<Seeded>> {
    let tx = conn.transaction()?;
    let has_releases = tx
        .query_row(r#"select 1 from "releases" limit 1"#, [], |_| Ok(()))
        .optional()?
        .is_some();
    if has_releases {
        return Ok(None);
    }

    let artist = insert_named(&tx, "artists", &ARTISTS)?;
    let label = insert_named(&tx, "labels", &LABELS)?;
    let format = upsert_named(&tx, "release_formats", &["Album", "EP", "LP", "Soundtrack"])?;
    let medium = upsert_named(&tx, "release_media", &["CD", "Digital", "Vinyl"])?;
    let genre = upsert_tag(&tx, "text_tags", "Genre", true)?;
    let score = upsert_tag(&tx, "real_number_tags", "Score", false)?;
    let ids = |found: &HashMap<String, i64>, names: &[&str]| -> Vec<i64> {
        names.iter().map(|name| found[*name]).collect()
    };
    let release = |values: SeedRelease| -> Result<i64> {
        insert_release(
            &tx,
            &values,
            &ids(&artist, values.artists),
            &ids(&label, values.labels),
            &ids(&format, values.formats),
            &ids(&medium, values.media),
        )
    };

    let nwts = release(SeedRelease {
        title: "Nothing Was the Same",
        title_language: None,
        released: "2013-09-24",
        catalog_number: Some("B0019141-02"),
        various_artists: false,
        artists: &["Drake"],
        labels: &[
            "October's Very Own",
            "Young Money",
            "Cash Money",
            "Republic Records",
        ],
        formats: &["Album"],
        media: &["CD"],
    })?;
    let hsm = release(SeedRelease {
        title: "High School Musical (Soundtrack)",
        title_language: None,
        released: "2006-01-10",
        catalog_number: Some("365 4622"),
        various_artists: true,
        artists: &["The High School Musical Cast"],
        labels: &["Walt Disney Records"],
        formats: &["Album", "Soundtrack"],
        media: &["CD"],
    })?;

    // Numbered per artist
    let mut positions: HashMap<&str, i64> = HashMap::new();
    for (owner, name, language, real_name, note) in ALTERNATE_NAMES {
        let position = positions.entry(owner).or_insert(0);
        tx.execute(
            r#"insert into "artist_alternate_names" ("artist_id", "name", "language", "real_name", "note", "position")
            values (?, ?, ?, ?, ?, ?)"#,
            params![
                artist[owner],
                name,
                language,
                i64::from(real_name),
                note,
                *position
            ],
        )?;
        *position += 1;
    }
    release(SeedRelease {
        title: "Alika",
        title_language: None,
        released: "2023",
        catalog_number: None,
        various_artists: false,
        artists: &["Alika"],
        labels: &[],
        formats: &[],
        media: &[],
    })?;
    let virmalised = release(SeedRelease {
        title: "virmalised",
        title_language: Some("et"),
        released: "2024",
        catalog_number: None,
        various_artists: false,
        artists: &["Alika"],
        labels: &[],
        formats: &["EP"],
        media: &["Digital"],
    })?;
    // Credited as printed
    tx.execute(
        r#"update "artists_releases" set "credited_name_id" = (
          select "id" from "artist_alternate_names" where "artist_id" = ? and "name" = 'ALIKA'
        ) where "release_id" = ?"#,
        params![artist["Alika"], virmalised],
    )?;
    let scenery = release(SeedRelease {
        title: "Scenery",
        title_language: Some("en"),
        released: "1976",
        catalog_number: Some("PA-7148"),
        various_artists: false,
        artists: &["Ryo Fukui", "Ryo Fukui Trio"],
        labels: &["Nadja"],
        formats: &["LP", "Album"],
        media: &["Vinyl"],
    })?;
    // Alternate titles: printed on Scenery, and a translation added for
    // virmalised
    for (release_id, title, language, printed) in [
        (scenery, "シーナリィ", "ja", true),
        (virmalised, "Northern Lights", "en", false),
    ] {
        tx.execute(
            r#"insert into "release_alternate_titles" ("release_id", "title", "language", "printed", "position")
            values (?, ?, ?, ?, 0)"#,
            params![release_id, title, language, i64::from(printed)],
        )?;
    }

    for (release_id, value) in [(nwts, "Hip Hop"), (hsm, "Pop"), (hsm, "Stage & Screen")] {
        tx.execute(
            r#"insert into "release_text_tags" ("release_id", "tag_id", "value") values (?, ?, ?)"#,
            params![release_id, genre, value],
        )?;
    }
    for (release_id, value) in [(nwts, 9.0), (hsm, 7.5)] {
        tx.execute(
            r#"insert into "release_real_number_tags" ("release_id", "tag_id", "value") values (?, ?, ?)"#,
            params![release_id, score, value],
        )?;
    }

    tx.commit()?;
    Ok(Some(Seeded {
        releases: 5,
        artists: ARTISTS.len(),
        labels: LABELS.len(),
        tags: 2,
    }))
}

/// Inserts rows by name, and returns their ids by name.
fn insert_named(conn: &Connection, table: &str, names: &[&str]) -> Result<HashMap<String, i64>> {
    let mut ids = HashMap::new();
    for name in names {
        let id: i64 = conn.query_row(
            &format!(r#"insert into "{table}" ("name") values (?) returning "id""#),
            [name],
            |row| row.get(0),
        )?;
        ids.insert(name.to_string(), id);
    }
    Ok(ids)
}

/// Like `insert_named`, for lists whose names are unique: keeps the rows
/// there already, which the migrations add.
fn upsert_named(conn: &Connection, table: &str, names: &[&str]) -> Result<HashMap<String, i64>> {
    for name in names {
        conn.execute(
            &format!(
                r#"insert into "{table}" ("name") values (?) on conflict ("name") do nothing"#
            ),
            [name],
        )?;
    }
    let mut ids = HashMap::new();
    for name in names {
        let (id, stored): (i64, String) = conn.query_row(
            &format!(r#"select "id", "name" from "{table}" where "name" = ?"#),
            [name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        ids.insert(stored, id);
    }
    Ok(ids)
}

/// A tag by name, created if it's missing. With no releases yet, no tag has
/// values, so `multiple` can be set on one that's there.
fn upsert_tag(conn: &Connection, table: &str, name: &str, multiple: bool) -> Result<i64> {
    conn.execute(
        &format!(
            r#"insert into "{table}" ("name", "multiple") values (?1, ?2)
            on conflict ("name") do update set "multiple" = ?2"#
        ),
        params![name, i64::from(multiple)],
    )?;
    Ok(conn.query_row(
        &format!(r#"select "id" from "{table}" where "name" = ?"#),
        [name],
        |row| row.get(0),
    )?)
}

/// A release, then its artists and labels in order, then its formats and
/// media.
fn insert_release(
    conn: &Connection,
    release: &SeedRelease,
    artists: &[i64],
    labels: &[i64],
    formats: &[i64],
    media: &[i64],
) -> Result<i64> {
    let date = parse_date(release.released)?;
    let id: i64 = conn.query_row(
        r#"insert into "releases" ("title", "title_language", "release_date", "release_date_precision", "catalog_number", "various_artists")
        values (?, ?, ?, ?, ?, ?) returning "id""#,
        params![
            release.title,
            release.title_language,
            date.epoch,
            date.precision.as_str(),
            release.catalog_number,
            i64::from(release.various_artists)
        ],
        |row| row.get(0),
    )?;
    for (position, artist) in artists.iter().enumerate() {
        conn.execute(
            r#"insert into "artists_releases" ("artist_id", "release_id", "position") values (?, ?, ?)"#,
            params![artist, id, position as i64],
        )?;
    }
    for (position, label) in labels.iter().enumerate() {
        conn.execute(
            r#"insert into "labels_releases" ("label_id", "release_id", "position") values (?, ?, ?)"#,
            params![label, id, position as i64],
        )?;
    }
    for format in formats {
        conn.execute(
            r#"insert into "release_formats_releases" ("format_id", "release_id") values (?, ?)"#,
            params![format, id],
        )?;
    }
    for medium in media {
        conn.execute(
            r#"insert into "release_media_releases" ("medium_id", "release_id") values (?, ?)"#,
            params![medium, id],
        )?;
    }
    Ok(id)
}
