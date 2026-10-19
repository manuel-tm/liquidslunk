//! The sample data: what `npm run db:seed` loads.

use slunk::repo::seeds::{self, Seeded};
use slunk::repo::{connection, migrator};

#[test]
fn the_seed_loads_once_and_credits_names_as_printed() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("slunk.db");
    migrator::migrate(&db, 10).unwrap();
    let mut conn = connection::open(&db).unwrap();
    assert_eq!(
        seeds::seed(&mut conn).unwrap(),
        Some(Seeded {
            releases: 5,
            artists: 13,
            labels: 6,
            tags: 2
        })
    );
    let number = |conn: &rusqlite::Connection, sql: &str| -> i64 {
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    };
    assert_eq!(
        number(&conn, "select count(*) from artist_alternate_names"),
        10
    );
    // virmalised's artist is credited as "ALIKA"
    assert_eq!(
        conn.query_row(
            "select n.name from artists_releases a join artist_alternate_names n on n.id = a.credited_name_id
             join releases r on r.id = a.release_id where r.title = 'virmalised'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "ALIKA"
    );
    // The formats the migrations add are used, not added again
    assert_eq!(
        number(
            &conn,
            "select count(*) from release_formats where name = 'Album'"
        ),
        1
    );

    // A database with releases is left as it is
    assert_eq!(seeds::seed(&mut conn).unwrap(), None);
    assert_eq!(number(&conn, "select count(*) from releases"), 5);
}
