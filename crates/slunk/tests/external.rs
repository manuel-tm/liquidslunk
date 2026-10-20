//! The external libraries context, on a temporary library: a track's tags
//! from the catalog, path templates and names on disk, as
//! docs/development/backend.md lists them ("The external libraries
//! context").

use std::collections::HashMap;
use std::sync::Arc;

use slunk::Error;
use slunk::dates::parse_date;
use slunk::external::{
    DEFAULT_TEMPLATE, External, SortFields, Tags, Template, safe_name, unique_paths,
};
use slunk::media::{Library, Media};
use slunk::repo::store::Store;
use slunk::repo::{connection, migrator};
use tempfile::TempDir;

const HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

struct Fixture {
    _dir: TempDir,
    store: Arc<Store>,
    external: External,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        migrator::migrate(&db, 10).unwrap();
        let store = Arc::new(Store::new(connection::open(&db).unwrap()));
        let media = Arc::new(Media::new(store.clone(), Library::new(&db)));
        Fixture {
            _dir: dir,
            external: External::new(store.clone(), media),
            store,
        }
    }

    fn sql(&self, sql: &str) {
        self.store.conn().execute_batch(sql).unwrap();
    }

    /// Music files 1 to `n`, at `music/<id>.flac`.
    fn files(&self, n: i64) {
        for id in 1..=n {
            self.sql(&format!(
                "insert into files (id, path, type, size_bytes, mtime, sha256)
                values ({id}, 'music/{id}.FLAC', 'music', 1, 0, '{HASH}')"
            ));
        }
    }

    fn tags(&self, id: i64) -> Tags {
        self.external.tags_of(&[id]).unwrap().remove(&id).unwrap()
    }
}

fn epoch(date: &str) -> i64 {
    parse_date(date).unwrap().epoch
}

#[test]
fn performers_are_the_tracks_own_or_on_a_regular_release_the_releases() {
    let f = Fixture::new();
    f.files(4);
    f.sql(
        "insert into artists (id, name) values (1, 'Ryo Fukui'), (2, 'Simon & Garfunkel'),
            (3, 'Various Artists'), (4, 'Mari'), (5, 'Sander'), (6, 'Alika');
        insert into releases (id, title, various_artists) values
            (1, 'Scenery', 0), (2, 'Sampler', 1);
        insert into artists_releases (artist_id, release_id, position) values
            (1, 1, 0), (2, 1, 1), (3, 2, 0);
        insert into tracks (id, title, track_number, cd_number, release_id, file_id) values
            (1, 'It Could Happen to You', 1, 1, 1, 1),
            (2, 'Bridges', 2, 1, 1, 2),
            (3, 'Survive', 1, 1, 2, 3),
            (4, 'Nobody', 2, 1, 2, 4);
        insert into artists_tracks (artist_id, track_id, featured, position) values
            (6, 2, 0, 0), (4, 2, 1, 1), (5, 2, 1, 2),
            (6, 3, 0, 0),
            (4, 4, 1, 0);",
    );

    // A regular release's track without its own artists is the release's,
    // joined with ", ": an "&" is part of a name
    let one = f.tags(1);
    assert_eq!(one.artist.as_deref(), Some("Ryo Fukui, Simon & Garfunkel"));
    assert_eq!(one.artists, ["Ryo Fukui", "Simon & Garfunkel"]);
    assert_eq!(
        one.album_artist.as_deref(),
        Some("Ryo Fukui, Simon & Garfunkel")
    );
    assert_eq!(one.title, "It Could Happen to You");
    assert!(!one.compilation);

    // Its own artists perform it, and features go in the title, not the
    // artist; ARTISTS has everyone
    let two = f.tags(2);
    assert_eq!(two.title, "Bridges (feat. Mari, Sander)");
    assert_eq!(two.artist.as_deref(), Some("Alika"));
    assert_eq!(two.artists, ["Alika", "Mari", "Sander"]);

    // On a various-artists release, only its own, or nobody, and the album
    // artist is the release artist
    let three = f.tags(3);
    assert!(three.compilation);
    assert_eq!(three.artist.as_deref(), Some("Alika"));
    assert_eq!(three.album_artist.as_deref(), Some("Various Artists"));
    let four = f.tags(4);
    assert_eq!(four.artist, None);
    assert_eq!(four.title, "Nobody (feat. Mari)");
    assert_eq!(four.artists, ["Mari"]);

    // Ids that aren't tracks are left out
    let tags = f.external.tags_of(&[1, 99]).unwrap();
    assert_eq!(tags.keys().collect::<Vec<_>>(), [&1]);
}

#[test]
fn artists_are_named_as_credited() {
    let f = Fixture::new();
    f.files(1);
    f.sql(
        "insert into artists (id, name) values (1, 'Alika'), (2, 'Mari');
        insert into artist_alternate_names (id, artist_id, name, real_name, position) values
            (10, 1, 'ALIKA', 0, 0), (11, 2, 'MARI', 0, 0);
        insert into releases (id, title, various_artists) values (1, 'virmalised', 0);
        insert into artists_releases (artist_id, release_id, position, credited_name_id)
            values (1, 1, 0, 10);
        insert into tracks (id, title, track_number, cd_number, release_id, file_id)
            values (1, 'Bridges', 1, 1, 1, 1);
        insert into artists_tracks (artist_id, track_id, featured, position, credited_name_id)
            values (2, 1, 1, 0, 11);",
    );
    let tags = f.tags(1);
    assert_eq!(tags.album_artist.as_deref(), Some("ALIKA"));
    // The release's artists perform it, as credited there
    assert_eq!(tags.artist.as_deref(), Some("ALIKA"));
    assert_eq!(tags.title, "Bridges (feat. MARI)");
    assert_eq!(tags.artists, ["ALIKA", "MARI"]);
}

#[test]
fn sort_fields_come_from_latin_script_forms() {
    let f = Fixture::new();
    f.files(5);
    f.sql(
        "insert into artists (id, name, name_language) values
            (1, 'Ryo Fukui', null), (2, '椎名林檎', 'ja'), (3, 'Mari', null), (4, '이소라', 'ko');
        insert into artist_alternate_names (id, artist_id, name, language, real_name, position) values
            (10, 1, '福居良', 'ja', 0, 0),
            (20, 2, 'Yumiko Shiina', 'ja-Latn', 1, 0),
            (21, 2, 'Ringo Shiina', 'ja-Latn', 0, 1),
            (40, 4, 'Lee Sora', 'en', 0, 0);
        insert into releases (id, title, title_language, various_artists) values
            (1, '初夏', 'ja', 0), (2, 'Movement', null, 0);
        insert into release_alternate_titles (release_id, title, language, printed, position) values
            (1, 'Early Summer', 'en', 0, 0), (1, 'Shoka', 'ja-Latn', 0, 1);
        insert into artists_releases (artist_id, release_id, position, credited_name_id) values
            (1, 1, 0, 10), (3, 1, 1, null), (2, 2, 0, null);
        insert into tracks (id, title, title_language, track_number, cd_number, release_id, file_id) values
            (1, '初夏', null, 1, 1, 1, 1),
            (2, '新しい光', null, 1, 1, 2, 2),
            (3, 'Survive', null, 2, 1, 2, 3),
            (4, '荒地', 'zh', 3, 1, 2, 4),
            (5, '바람', null, 4, 1, 2, 5);
        insert into track_alternate_titles (track_id, title, language, printed, position) values
            (1, 'Early Summer', 'en', 0, 0), (1, 'Shoka', 'ja-Latn', 1, 1),
            (2, 'Atarashii Hikari', 'ja-Latn', 1, 0),
            (4, 'Kōchi', 'ja-Latn', 0, 0);
        insert into artists_tracks (artist_id, track_id, featured, position) values (4, 5, 0, 0);",
    );

    // "初夏" sorts as "Shoka", its form in ja-Latn, and not as its
    // translation. "福居良" sorts as the artist's main name, and names joined
    // with ", " sort as each one's sort name, or the name itself.
    assert_eq!(
        f.tags(1).sort,
        SortFields {
            artist: Some("Ryo Fukui, Mari".into()),
            album_artist: Some("Ryo Fukui, Mari".into()),
            album: Some("Shoka".into()),
            title: Some("Shoka".into()),
        }
    );
    // A title whose language isn't known, nor its release's, takes any form
    // tagged with the Latin script. A main name that isn't in Latin script
    // sorts as its first form, but a real name isn't one.
    let two = f.tags(2);
    assert_eq!(two.sort.title.as_deref(), Some("Atarashii Hikari"));
    assert_eq!(two.sort.artist.as_deref(), Some("Ringo Shiina"));
    assert_eq!(two.sort.album, None);
    // Latin script needs none
    assert_eq!(f.tags(3).sort.title, None);
    // A form in another language isn't one: "Kōchi" is ja-Latn, "荒地" zh
    assert_eq!(f.tags(4).sort.title, None);
    // Nor is a name in en: "이소라" has no form, so it sorts as it is
    let five = f.tags(5);
    assert_eq!(five.sort.title, None);
    assert_eq!(five.sort.artist, None);
}

#[test]
fn totals_are_the_last_numbers_and_dates_are_as_precise_as_known() {
    let f = Fixture::new();
    f.files(6);
    f.sql(&format!(
        "insert into releases (id, title, release_date, release_date_precision, various_artists) values
            (1, 'Double', {}, 'month', 0),
            (2, 'Dated', {}, 'day', 0),
            (3, 'Year', {}, 'year', 0),
            (4, 'Undated', null, null, 0);
        insert into tracks (id, title, track_number, cd_number, release_id, file_id) values
            (1, 'One', 1, 1, 1, 1),
            (2, 'Four', 4, 1, 1, 2),
            (3, 'Two One', 1, 2, 1, 3),
            (4, 'A', 1, 1, 2, 4),
            (5, 'B', 1, 1, 3, 5),
            (6, 'C', 1, 1, 4, 6);",
        epoch("2006-01"),
        epoch("2006-01-10"),
        epoch("1976"),
    ));

    // A disc's tracks run to its last number, and the release to its last
    // disc, gaps and all
    let one = f.tags(1);
    assert_eq!((one.track_number, one.track_total), (1, 4));
    assert_eq!((one.disc_number, one.disc_total), (1, 2));
    let three = f.tags(3);
    assert_eq!((three.track_number, three.track_total), (1, 1));
    assert_eq!((three.disc_number, three.disc_total), (2, 2));

    assert_eq!(one.date.as_deref(), Some("2006-01"));
    assert_eq!(one.year, Some(2006));
    assert_eq!(f.tags(4).date.as_deref(), Some("2006-01-10"));
    assert_eq!(f.tags(5).date.as_deref(), Some("1976"));
    let undated = f.tags(6);
    assert_eq!((undated.date, undated.year), (None, None));
}

#[test]
fn the_release_gives_labels_catalog_number_media_formats_disc_titles_and_cover() {
    let f = Fixture::new();
    f.files(3);
    f.sql(
        "insert into files (id, path, type, size_bytes, mtime, sha256)
            values (9, 'music/cover.jpg', 'image', 1, 0, '0000000000000000000000000000000000000000000000000000000000000000');
        insert into labels (id, name) values (1, 'Trio Records'), (2, 'Think! Records');
        insert into label_alternate_names (label_id, name, position) values (1, 'トリオ', 0);
        insert into releases (id, title, catalog_number, cover_picture, various_artists)
            values (1, 'Scenery', 'PA-7148', 9, 0), (2, 'Bare', null, null, 0);
        insert into labels_releases (label_id, release_id, position) values (2, 1, 0), (1, 1, 1);
        insert into release_formats_releases (format_id, release_id) values (4, 1), (1, 1);
        insert into release_media_releases (medium_id, release_id) values (4, 1), (2, 1);
        insert into tracks (id, title, track_number, cd_number, release_id, file_id) values
            (1, 'One', 1, 1, 1, 1), (2, 'Two', 1, 2, 1, 2), (3, 'Three', 1, 1, 2, 3);
        insert into release_disc_titles (release_id, cd_number, title)
            values (1, 2, 'Live At Madison Square Garden');",
    );
    let one = f.tags(1);
    assert_eq!(one.labels, ["Think! Records", "Trio Records"]);
    assert_eq!(one.catalog_number.as_deref(), Some("PA-7148"));
    // Formats and media as the release lists them, by their own id
    assert_eq!(one.release_types, ["Album", "Live"]);
    assert_eq!(one.media, ["CD", "Vinyl"]);
    assert_eq!(one.cover, Some(9));
    assert_eq!(one.disc_title, None);
    assert_eq!(
        f.tags(2).disc_title.as_deref(),
        Some("Live At Madison Square Garden")
    );

    // Nothing is written for a field with no value
    let bare = f.tags(3);
    assert!(bare.labels.is_empty() && bare.media.is_empty() && bare.release_types.is_empty());
    assert_eq!((bare.catalog_number, bare.cover), (None, None));
    assert_eq!((bare.artist, bare.album_artist), (None, None));
}

#[test]
fn nothing_comes_from_tag_values_or_credit_lines() {
    let f = Fixture::new();
    f.files(1);
    f.sql(
        "insert into artists (id, name) values (1, 'Ryo Fukui'), (2, 'Jimmy Van Heusen');
        insert into releases (id, title, various_artists) values (1, 'Scenery', 0);
        insert into artists_releases (artist_id, release_id, position) values (1, 1, 0);
        insert into tracks (id, title, track_number, cd_number, release_id, file_id)
            values (1, 'It Could Happen to You', 1, 1, 1, 1);",
    );
    let before = f.tags(1);
    f.sql(
        "begin;
        insert into text_tags (id, name, multiple) values (1, 'Genre', 1);
        insert into real_number_tags (id, name, multiple) values (1, 'Score', 0);
        insert into release_text_tags (release_id, tag_id, value) values (1, 1, 'Jazz');
        insert into track_text_tags (track_id, tag_id, value) values (1, 1, 'Jazz');
        insert into release_real_number_tags (release_id, tag_id, value) values (1, 1, 9);
        insert into release_credits (id, release_id, role, position) values (1, 1, 'Producer', 0);
        insert into artists_release_credits (artist_id, credit_id, position) values (2, 1, 0);
        insert into track_credits (id, track_id, role, position) values (1, 1, 'Composed By', 0);
        insert into artists_track_credits (artist_id, credit_id, position) values (2, 1, 0);
        commit;",
    );
    assert_eq!(f.tags(1), before);
}

#[test]
fn the_track_page_shows_tags_and_the_path_a_folder_library_keeping_originals_gives() {
    let f = Fixture::new();
    f.files(1);
    f.sql(&format!(
        "insert into artists (id, name) values (1, 'Ryo Fukui');
        insert into releases (id, title, release_date, release_date_precision, various_artists)
            values (1, 'Scenery', {}, 'year', 0);
        insert into artists_releases (artist_id, release_id, position) values (1, 1, 0);
        insert into tracks (id, title, track_number, cd_number, release_id, file_id)
            values (1, 'It Could Happen to You', 1, 1, 1, 1);",
        epoch("1976")
    ));
    let template = Template::parse(DEFAULT_TEMPLATE).unwrap();
    let exported = f.external.exported(1, &template).unwrap().unwrap();
    // The file keeps its extension, in lowercase
    assert_eq!(
        exported.path,
        "Ryo Fukui/1976 - Scenery/01 It Could Happen to You.flac"
    );
    assert_eq!(exported.tags, f.tags(1));
    assert_eq!(f.external.exported(99, &template).unwrap(), None);
}

/// Tags for templates, as a track on a release with one disc would have.
fn tags() -> Tags {
    Tags {
        title: "It Could Happen to You".into(),
        artist: Some("Ryo Fukui Trio".into()),
        artists: vec!["Ryo Fukui Trio".into()],
        album: "Scenery".into(),
        album_artist: Some("Ryo Fukui".into()),
        compilation: false,
        date: Some("1976-05".into()),
        year: Some(1976),
        track_number: 3,
        track_total: 12,
        disc_number: 1,
        disc_total: 1,
        disc_title: None,
        labels: vec!["Trio Records".into(), "Think! Records".into()],
        catalog_number: Some("PA-7148".into()),
        media: vec![],
        release_types: vec![],
        sort: SortFields::default(),
        cover: None,
    }
}

fn path(template: &str, tags: &Tags) -> String {
    Template::parse(template).unwrap().path(tags, "flac").safe()
}

fn refused(template: &str) -> String {
    match Template::parse(template) {
        Err(Error::Invalid(message)) => message,
        other => panic!("{template} wasn't refused: {other:?}"),
    }
}

#[test]
fn the_default_template_leaves_out_what_a_release_doesnt_have() {
    let mut tags = tags();
    assert_eq!(
        path(DEFAULT_TEMPLATE, &tags),
        "Ryo Fukui/1976 - Scenery/03 It Could Happen to You.flac"
    );
    tags.disc_number = 2;
    tags.disc_total = 2;
    assert_eq!(
        path(DEFAULT_TEMPLATE, &tags),
        "Ryo Fukui/1976 - Scenery/2-03 It Could Happen to You.flac"
    );
    tags.year = None;
    tags.date = None;
    assert_eq!(
        path(DEFAULT_TEMPLATE, &tags),
        "Ryo Fukui/Scenery/2-03 It Could Happen to You.flac"
    );
}

#[test]
fn templates_have_variables_padding_and_optional_parts() {
    let mut tags = tags();
    tags.disc_title = Some("Side A".into());
    assert_eq!(
        path(
            "${artist}/${year} - ${album}/${cd_number}_${track_number}-${track_title}.${format}",
            &tags
        ),
        "Ryo Fukui Trio/1976 - Scenery/1_3-It Could Happen to You.flac"
    );
    assert_eq!(
        path(
            "${date} ${label}[ (${catalog_number})]/${cd_number:2}.${cd_count}.${track_number:3}.${track_count} ${disc_title}.${format}",
            &tags
        ),
        "1976-05 Trio Records (PA-7148)/01.1.003.12 Side A.flac"
    );
    // A bracket is left out when a variable in it is empty, and kept with
    // none
    tags.catalog_number = None;
    tags.labels.clear();
    assert_eq!(
        path(
            "[${label} ][(${catalog_number}) ][x ]${track_number:2}.${format}",
            &tags
        ),
        "x 03.flac"
    );
    // A folder can be optional too, and an empty one is "_"
    assert_eq!(
        path("[${label}/]${label}/${track_title}.${format}", &tags),
        "_/It Could Happen to You.flac"
    );
    // ${format} can name a folder
    assert_eq!(
        path("${format}/${track_title}.${format}", &tags),
        "flac/It Could Happen to You.flac"
    );
    // A number wider than its padding stays whole
    tags.track_number = 123;
    assert_eq!(path("${track_number:2}.${format}", &tags), "123.flac");
}

#[test]
fn templates_are_checked_when_theyre_read() {
    assert_eq!(
        refused("${genre}/${track_title}.${format}"),
        "${genre} isn't a template variable"
    );
    assert_eq!(
        refused("${album:2}/${track_title}.${format}"),
        "Only numbers can be padded, and ${album} isn't one"
    );
    assert_eq!(
        refused("${track_number:x}.${format}"),
        "Pad a number to 1 to 9 digits, like ${track_number:2}"
    );
    assert_eq!(
        refused("${track_number:0}.${format}"),
        "Pad a number to 1 to 9 digits, like ${track_number:2}"
    );
    assert_eq!(
        refused("${album/${track_title}.${format}"),
        "${album/${track_title} isn't a template variable"
    );
    assert_eq!(
        refused("${track_title}.${format"),
        "The template has a ${ that isn't closed"
    );
    assert_eq!(
        refused("[${disc}-${track_title}.${format}"),
        "The template has a [ that isn't closed"
    );
    assert_eq!(
        refused("${disc}]-${track_title}.${format}"),
        "The template has a ] with no [ before it"
    );
    assert_eq!(
        refused("[[${disc}]]${track_title}.${format}"),
        "A template can't have brackets inside brackets"
    );
    assert_eq!(
        refused("${track_title}.flac"),
        "The template has to end in .${format}"
    );
    assert_eq!(refused(""), "The template has to end in .${format}");
    assert_eq!(
        refused("${album}/${album}.${format}"),
        "The template needs ${track_number} or ${track_title}, or every track of a release gets the same name"
    );
    // Either one will do, and the template is cleaned like other text
    assert!(Template::parse(" ${track_number}.${format} ").is_ok());
    assert!(Template::parse("[${track_title}].${format}").is_ok());
}

#[test]
fn names_are_made_safe_for_fat32_and_exfat() {
    let mut tags = tags();
    // A value's own "/" can't make a folder
    tags.album_artist = Some("AC/DC".into());
    tags.album = "...And Justice for All".into();
    tags.title = "What? \"No\" <a|b> 1:2 *\\\u{7}.".into();
    assert_eq!(
        path(DEFAULT_TEMPLATE, &tags),
        "AC_DC/1976 - ...And Justice for All/03 What_ _No_ _a_b_ 1_2 ___..flac"
    );
    // Leading dots, and trailing dots and spaces, go
    assert_eq!(safe_name("...And Justice for All"), "And Justice for All");
    assert_eq!(safe_name("Vol. 2. . "), "Vol. 2");
    assert_eq!(safe_name(".."), "_");
    assert_eq!(safe_name(""), "_");
    // In NFC: "é" as "e" and an accent becomes one code point
    assert_eq!(safe_name("Cafe\u{301}"), "Caf\u{e9}");
    // At most 255 UTF-16 code units, cut at a character: "𝄞" is two
    let long = "𝄞".repeat(200);
    assert_eq!(safe_name(&long), "𝄞".repeat(127));
    // A file's name is cut before its extension
    tags.title = "あ".repeat(300);
    let path = path("${track_title}.${format}", &tags);
    assert_eq!(path, format!("{}.flac", "あ".repeat(250)));
    assert_eq!(path.encode_utf16().count(), 255);
    // A name left empty is "_"
    tags.title = "...".into();
    assert_eq!(
        Template::parse("${track_title}.${format}")
            .unwrap()
            .path(&tags, "flac")
            .safe(),
        "_.flac"
    );
}

#[test]
fn tracks_with_the_same_path_are_told_apart_by_id() {
    let template = Template::parse("${album_artist}/${track_title}.${format}").unwrap();
    let path = |artist: &str, title: &str| {
        let mut tags = tags();
        tags.album_artist = Some(artist.into());
        tags.title = title.into();
        template.path(&tags, "flac")
    };
    let paths = vec![
        (5, path("Alika", "Intro")),
        (3, path("Alika", "Intro")),
        // Case and Unicode normalization don't tell paths apart
        (4, path("ALIKA", "intro")),
        // A track's own path isn't taken by a number
        (7, path("Alika", "Intro (2)")),
        (8, path("Alika", "Cafe\u{301}")),
        (9, path("Alika", "Caf\u{e9}")),
        (1, path("Alika", "Outro")),
    ];
    let unique = unique_paths(&paths);
    let expected: HashMap<i64, String> = [
        (3, "Alika/Intro.flac"),
        (4, "ALIKA/intro (3).flac"),
        (5, "Alika/Intro (4).flac"),
        (7, "Alika/Intro (2).flac"),
        (8, "Alika/Caf\u{e9}.flac"),
        (9, "Alika/Caf\u{e9} (2).flac"),
        (1, "Alika/Outro.flac"),
    ]
    .into_iter()
    .map(|(id, path)| (id, path.to_string()))
    .collect();
    assert_eq!(unique, expected);
}
