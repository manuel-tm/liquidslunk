//! External libraries, their rules, and what they come to, on a temporary
//! library, as docs/development/backend.md lists them ("External libraries
//! and their rules").

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use slunk::Error;
use slunk::external::{
    Additions, DEFAULT_TEMPLATE, External, ExternalKind, ExternalLibrary, ExternalLibraryInput,
    OutputFormat, Reason, RuleTarget, RuleValues, Subject,
};
use slunk::media::{Library, Media};
use slunk::repo::store::Store;
use slunk::repo::{connection, migrator};
use slunk::tagging::TagType;
use tempfile::TempDir;

const HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

struct Fixture {
    dir: TempDir,
    store: Arc<Store>,
    media: Arc<Media>,
    external: External,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("library");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slunk.db");
        migrator::migrate(&db, 10).unwrap();
        let store = Arc::new(Store::new(connection::open(&db).unwrap()));
        let media = Arc::new(Media::new(store.clone(), Library::new(&db)));
        Fixture {
            dir,
            external: External::new(store.clone(), media.clone()),
            media,
            store,
        }
    }

    /// A path in the temporary folder, outside the library's.
    fn outside(&self, path: &str) -> String {
        let base = self.dir.path().canonicalize().unwrap();
        base.join(path).to_string_lossy().to_string()
    }

    fn root(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap().join("library")
    }

    fn sql(&self, sql: &str) {
        self.store.conn().execute_batch(sql).unwrap();
    }

    fn folder_library(&self, name: &str) -> ExternalLibrary {
        let path = self.outside(&name.to_lowercase());
        self.external.create(&folder(name, &path)).unwrap()
    }

    fn rules(&self, id: i64, rules: &[(bool, RuleTarget)]) {
        let rules: Vec<RuleValues> = rules
            .iter()
            .map(|(exclude, target)| RuleValues {
                exclude: *exclude,
                target: target.clone(),
            })
            .collect();
        self.external.set_rules(id, &rules).unwrap();
    }

    /// The rules, as (exclude, target), in order.
    fn rules_of(&self, id: i64) -> Vec<(bool, RuleTarget)> {
        self.external
            .rules(id)
            .unwrap()
            .into_iter()
            .map(|rule| (rule.exclude, rule.target))
            .collect()
    }

    /// Four releases and eight tracks: Pink Floyd's The Wall and Animals,
    /// Ryo Fukui's Scenery, and a various-artists Sampler, where Pink Floyd
    /// performs a track. Genre and Score are set on some.
    fn catalog(&self) {
        for id in 1..=8 {
            let extension = if id == 8 { "dsf" } else { "flac" };
            self.sql(&format!(
                "insert into files (id, path, type, size_bytes, mtime, sha256)
                values ({id}, 'music/{id}.{extension}', 'music', 10000000, 0, '{HASH}')"
            ));
        }
        self.sql(
            "insert into artists (id, name) values (1, 'Pink Floyd'), (2, 'Ryo Fukui'), (3, 'Guest');
            insert into releases (id, title, various_artists) values
                (1, 'The Wall', 0), (2, 'Animals', 0), (3, 'Scenery', 0), (4, 'Sampler', 1);
            insert into artists_releases (artist_id, release_id, position)
                values (1, 1, 0), (1, 2, 0), (2, 3, 0);
            insert into tracks (id, title, track_number, cd_number, release_id, file_id, duration_ms) values
                (1, 'In the Flesh?', 1, 1, 1, 1, 240000),
                (2, 'The Thin Ice', 2, 1, 1, 2, 240000),
                (3, 'Pigs on the Wing', 1, 1, 2, 3, 240000),
                (4, 'Dogs', 2, 1, 2, 4, 240000),
                (5, 'It Could Happen to You', 1, 1, 3, 5, 240000),
                (6, 'Early Summer', 2, 1, 3, 6, 240000),
                (7, 'Song', 1, 1, 4, 7, 240000),
                (8, 'Other', 2, 1, 4, 8, 240000);
            insert into artists_tracks (artist_id, track_id, featured, position) values (1, 7, 0, 0);
            insert into text_tags (id, name, multiple) values (1, 'Genre', 1);
            insert into real_number_tags (id, name, multiple) values (1, 'Score', 0);
            insert into release_text_tags (release_id, tag_id, value) values (3, 1, 'Jazz');
            insert into track_text_tags (track_id, tag_id, value) values (7, 1, 'jazz');
            insert into release_real_number_tags (release_id, tag_id, value) values (1, 1, 7), (2, 1, 9);",
        );
    }
}

fn folder(name: &str, folder: &str) -> ExternalLibraryInput {
    ExternalLibraryInput {
        name: name.into(),
        kind: ExternalKind::Folder,
        folder: Some(folder.into()),
        format: OutputFormat::Aac,
        aac_kbps: 256,
        cd_resolution: false,
        path_template: None,
        embed_covers: true,
        cover_px: 600,
        cover_file: false,
    }
}

fn ipod(name: &str) -> ExternalLibraryInput {
    ExternalLibraryInput {
        name: name.into(),
        kind: ExternalKind::Ipod,
        folder: None,
        format: OutputFormat::Alac,
        aac_kbps: 256,
        cd_resolution: true,
        path_template: None,
        embed_covers: false,
        cover_px: 600,
        cover_file: false,
    }
}

fn invalid<T: std::fmt::Debug>(result: slunk::Result<T>) -> String {
    match result {
        Err(Error::Invalid(message)) => message,
        other => panic!("not refused: {other:?}"),
    }
}

fn release(id: i64) -> RuleTarget {
    RuleTarget::Release(id)
}

fn artist(id: i64) -> RuleTarget {
    RuleTarget::Artist(id)
}

fn track(id: i64) -> RuleTarget {
    RuleTarget::Track(id)
}

fn genre(value: &str) -> RuleTarget {
    RuleTarget::Tag {
        kind: TagType::Text,
        tag_id: 1,
        equals: Some(value.into()),
        min: None,
        max: None,
    }
}

fn score(equals: Option<&str>, min: Option<&str>) -> RuleTarget {
    RuleTarget::Tag {
        kind: TagType::Number,
        tag_id: 1,
        equals: equals.map(str::to_string),
        min: min.map(str::to_string),
        max: None,
    }
}

#[test]
fn external_libraries_are_created_with_their_profile() {
    let f = Fixture::new();
    let car = f
        .external
        .create(&folder("  Car ", &f.outside("car")))
        .unwrap();
    assert_eq!(car.name, "Car");
    assert_eq!(car.kind, ExternalKind::Folder);
    assert_eq!(car.folder, Some(f.outside("car")));
    // A blank template is the default
    assert_eq!(car.profile.path_template.as_deref(), Some(DEFAULT_TEMPLATE));
    assert_eq!(car.profile.format, OutputFormat::Aac);
    // A UUID, version 4, for its destination's manifest
    assert_eq!(car.uuid.len(), 36);
    assert_eq!(&car.uuid[14..15], "4");
    assert_eq!(car.uuid.matches('-').count(), 4);

    // An iPod takes what it plays, and has no folder or template
    let ipod = f
        .external
        .create(&ExternalLibraryInput {
            folder: Some(f.outside("x")),
            cd_resolution: false,
            embed_covers: true,
            cover_file: true,
            path_template: Some("${track_title}.${format}".into()),
            ..ipod("iPod")
        })
        .unwrap();
    assert_eq!(ipod.folder, None);
    assert_eq!(ipod.profile.path_template, None);
    assert!(ipod.profile.cd_resolution);
    assert!(!ipod.profile.embed_covers && !ipod.profile.cover_file);
    assert_ne!(ipod.uuid, car.uuid);

    let names: Vec<String> = f
        .external
        .list()
        .unwrap()
        .into_iter()
        .map(|library| library.name)
        .collect();
    assert_eq!(names, ["Car", "iPod"]);

    // Names are unique ignoring case, and a folder belongs to one
    let taken = f
        .external
        .create(&folder("CAR", &f.outside("other")))
        .unwrap_err();
    assert!(
        taken.to_string().contains("external_libraries.name"),
        "{taken}"
    );
    let shared = f
        .external
        .create(&folder("Phone", &f.outside("car")))
        .unwrap_err();
    assert!(
        shared.to_string().contains("external_libraries.folder"),
        "{shared}"
    );
}

#[test]
fn profiles_are_checked() {
    let f = Fixture::new();
    let card = f.outside("card");
    assert_eq!(
        invalid(f.external.create(&folder(" ", &card))),
        "The name can't be empty"
    );
    assert_eq!(
        invalid(f.external.create(&ExternalLibraryInput {
            aac_kbps: 127,
            ..folder("Car", &card)
        })),
        "AAC's bitrate is 128 to 320 kbps"
    );
    assert_eq!(
        invalid(f.external.create(&ExternalLibraryInput {
            cover_px: 99,
            ..folder("Car", &card)
        })),
        "Embedded covers are 100 to 3000 pixels"
    );
    assert_eq!(
        invalid(f.external.create(&ExternalLibraryInput {
            path_template: Some("${genre}/${track_title}.${format}".into()),
            ..folder("Car", &card)
        })),
        "${genre} isn't a template variable"
    );
    assert_eq!(
        invalid(f.external.create(&ExternalLibraryInput {
            format: OutputFormat::Original,
            ..ipod("iPod")
        })),
        "An iPod plays Apple Lossless or AAC"
    );

    let car = f.external.create(&folder("Car", &card)).unwrap();
    let saved = f
        .external
        .update(
            car.id,
            &ExternalLibraryInput {
                format: OutputFormat::Flac,
                cd_resolution: true,
                path_template: Some(" ${artist}/${track_title}.${format} ".into()),
                ..folder("Card", &card)
            },
        )
        .unwrap();
    assert_eq!(saved.name, "Card");
    assert_eq!(saved.profile.format, OutputFormat::Flac);
    assert!(saved.profile.cd_resolution);
    assert_eq!(
        saved.profile.path_template.as_deref(),
        Some("${artist}/${track_title}.${format}")
    );
    assert_eq!(saved.uuid, car.uuid);
    assert_eq!(
        invalid(f.external.update(car.id, &ipod("Card"))),
        "An external library can't change from a folder to an iPod, or back"
    );
    assert!(matches!(
        f.external.update(99, &folder("X", &card)),
        Err(Error::NotFound(message)) if message == "There's no external library 99"
    ));
}

#[test]
fn a_folder_library_is_never_where_the_users_files_are() {
    let f = Fixture::new();
    let shared = PathBuf::from(f.outside("shared"));
    fs::create_dir_all(&shared).unwrap();
    fs::write(shared.join("a.flac"), b"audio").unwrap();
    f.media
        .register_files(&["../shared/a.flac".into()])
        .unwrap();
    let root = f.root().to_string_lossy().to_string();
    let refused = |path: &str| invalid(f.external.create(&folder("Car", path)));

    assert_eq!(
        refused("music"),
        "The folder needs an absolute path, like /Volumes/CARD/Music"
    );
    assert_eq!(
        refused(&root),
        "That's the library's own folder. Choose one outside it."
    );
    assert_eq!(
        refused(&format!("{root}/music/")),
        "That folder is inside the library's folder. Choose one outside it."
    );
    assert_eq!(
        refused(&f.outside("")),
        "That folder holds the library. Choose one outside it."
    );
    assert_eq!(
        refused("/"),
        "That folder holds the library. Choose one outside it."
    );
    assert_eq!(
        refused(&f.outside("shared")),
        "That folder holds files the library has registered. Choose one without them."
    );
    assert_eq!(
        invalid(f.external.create(&ExternalLibraryInput {
            folder: None,
            ..folder("Car", "")
        })),
        "A folder library needs a folder"
    );

    // A folder beside them is fine, resolved
    let car = f
        .external
        .create(&folder("Car", &f.outside("car/../cards")))
        .unwrap();
    assert_eq!(car.folder, Some(f.outside("cards")));
}

#[test]
fn a_folder_library_is_connected_while_its_folder_is_there() {
    let f = Fixture::new();
    let car = f.folder_library("Car");
    assert!(!f.external.connected(&car));
    fs::create_dir_all(f.outside("car")).unwrap();
    assert!(f.external.connected(&car));
    // An iPod library isn't tied to an iPod yet
    let ipod = f.external.create(&ipod("iPod")).unwrap();
    assert!(!f.external.connected(&ipod));
}

#[test]
fn rules_include_and_exclusions_win() {
    let f = Fixture::new();
    f.catalog();
    let car = f.folder_library("Car").id;
    let tracks = || f.external.track_ids(car).unwrap();
    assert!(tracks().is_empty());

    // An artist's releases, by release title, then disc and number. Its
    // appearance on Sampler isn't one of them.
    f.rules(car, &[(false, artist(1))]);
    assert_eq!(tracks(), [3, 4, 1, 2]);
    // "Pink Floyd, but not The Wall"
    f.rules(car, &[(false, artist(1)), (true, release(1))]);
    assert_eq!(tracks(), [3, 4]);
    // A track rule adds one
    f.rules(
        car,
        &[(false, artist(1)), (true, release(1)), (false, track(7))],
    );
    assert_eq!(tracks(), [3, 4, 7]);
    // A left-out track wins over its release's rule
    f.rules(car, &[(false, release(3)), (true, track(6))]);
    assert_eq!(tracks(), [5]);

    // A tag value takes in the releases with it, whole, and the tracks with
    // it, ignoring case
    f.rules(car, &[(false, genre("JAZZ"))]);
    assert_eq!(tracks(), [7, 5, 6]);
    // Numbers, by a value or a range
    f.rules(car, &[(false, score(None, Some("8")))]);
    assert_eq!(tracks(), [3, 4]);
    f.rules(car, &[(false, score(Some("7"), None))]);
    assert_eq!(tracks(), [1, 2]);
}

#[test]
fn rules_are_checked_and_kept_once() {
    let f = Fixture::new();
    f.catalog();
    let car = f.folder_library("Car").id;
    let set = |rules: &[(bool, RuleTarget)]| {
        let rules: Vec<RuleValues> = rules
            .iter()
            .map(|(exclude, target)| RuleValues {
                exclude: *exclude,
                target: target.clone(),
            })
            .collect();
        f.external.set_rules(car, &rules)
    };
    assert_eq!(
        invalid(set(&[(true, artist(1))])),
        "Only a release or a track can be left out"
    );
    assert_eq!(
        invalid(set(&[(true, genre("Jazz"))])),
        "Only a release or a track can be left out"
    );
    assert_eq!(
        invalid(set(&[(false, release(1)), (true, release(1))])),
        "A rule can't both include something and leave it out"
    );
    let text_range = RuleTarget::Tag {
        kind: TagType::Text,
        tag_id: 1,
        equals: Some("Jazz".into()),
        min: Some("a".into()),
        max: None,
    };
    assert_eq!(
        invalid(set(&[(false, text_range)])),
        "A text tag value rule takes a value"
    );
    assert_eq!(
        invalid(set(&[(false, score(None, None))])),
        "A number tag value rule takes a value, or a range"
    );
    assert!(matches!(
        set(&[(false, score(Some("abc"), None))]),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        set(&[(false, release(99))]),
        Err(Error::NotFound(message)) if message == "There's no release 99"
    ));
    let unknown_tag = RuleTarget::Tag {
        kind: TagType::Text,
        tag_id: 9,
        equals: Some("Jazz".into()),
        min: None,
        max: None,
    };
    assert!(matches!(
        set(&[(false, unknown_tag)]),
        Err(Error::NotFound(message)) if message == "There's no text tag 9"
    ));
    // Nothing was saved
    assert!(f.rules_of(car).is_empty());

    // A rule given twice is kept once, text values ignoring case, and a
    // value is cleaned like text
    set(&[
        (false, release(1)),
        (false, genre(" Jazz ")),
        (false, release(1)),
        (false, genre("jazz")),
    ])
    .unwrap();
    assert_eq!(
        f.rules_of(car),
        [(false, release(1)), (false, genre("Jazz"))]
    );
    assert!(matches!(
        f.external.set_rules(99, &[]),
        Err(Error::NotFound(message)) if message == "There's no external library 99"
    ));
}

#[test]
fn rules_stay_live_and_go_with_what_they_name() {
    let f = Fixture::new();
    f.catalog();
    let car = f.folder_library("Car").id;
    f.rules(
        car,
        &[
            (false, artist(1)),
            (false, artist(3)),
            (false, release(3)),
            (false, track(7)),
            (false, genre("Jazz")),
        ],
    );
    // A release added later for an included artist is in the next sync
    f.sql(&format!(
        "insert into files (id, path, type, size_bytes, mtime, sha256)
            values (9, 'music/9.flac', 'music', 1, 0, '{HASH}');
        insert into releases (id, title, various_artists) values (5, 'Meddle', 0);
        insert into artists_releases (artist_id, release_id, position) values (1, 5, 0);
        insert into tracks (id, title, track_number, cd_number, release_id, file_id)
            values (9, 'Echoes', 1, 1, 5, 9);"
    ));
    assert!(f.external.track_ids(car).unwrap().contains(&9));

    // Deleting a track, a release, an artist or a tag takes its rules with
    // it, and the tracks leave
    f.sql(
        "delete from tracks where id = 7;
        delete from tracks where release_id = 3;
        delete from releases where id = 3;
        delete from artists where id = 3;
        delete from release_text_tags; delete from track_text_tags;
        delete from text_tags where id = 1;",
    );
    assert_eq!(f.rules_of(car), [(false, artist(1))]);
    assert_eq!(f.external.track_ids(car).unwrap(), [3, 4, 9, 1, 2]);
}

#[test]
fn adding_from_the_catalogs_pages() {
    let f = Fixture::new();
    f.catalog();
    let car = f.folder_library("Car").id;
    let add = |releases: &[i64], artists: &[i64], tracks: &[i64]| {
        f.external
            .add(
                car,
                &Additions {
                    releases: releases.to_vec(),
                    artists: artists.to_vec(),
                    tracks: tracks.to_vec(),
                },
            )
            .unwrap()
    };

    // A release comes in whole, with a rule of its own. Ids that aren't
    // there are skipped.
    f.rules(
        car,
        &[(false, artist(1)), (true, release(1)), (true, track(2))],
    );
    add(&[1, 99], &[], &[]);
    assert_eq!(f.rules_of(car), [(false, artist(1)), (false, release(1))]);
    assert_eq!(f.external.track_ids(car).unwrap(), [3, 4, 1, 2]);

    // A track's exclusion goes, and it gets a rule
    f.rules(car, &[(false, release(2)), (true, track(4))]);
    add(&[], &[], &[4]);
    assert_eq!(f.rules_of(car), [(false, release(2)), (false, track(4))]);

    // A track of a left-out release: its other tracks are left out instead
    f.rules(car, &[(false, artist(1)), (true, release(2))]);
    add(&[], &[], &[3]);
    assert_eq!(
        f.rules_of(car),
        [(false, artist(1)), (true, track(4)), (false, track(3))]
    );
    assert_eq!(f.external.track_ids(car).unwrap(), [3, 1, 2]);

    // An artist gets a rule, once
    add(&[], &[2, 2, 99], &[]);
    add(&[], &[2], &[]);
    assert_eq!(f.rules_of(car).last(), Some(&(false, artist(2))));
    assert_eq!(f.rules_of(car).len(), 4);
}

#[test]
fn removing_from_the_catalogs_pages() {
    let f = Fixture::new();
    f.catalog();
    let car = f.folder_library("Car").id;
    let remove = |target: RuleTarget| f.external.remove(car, &target).unwrap();

    // Its own rule goes, and its tracks', and since the artist still brings
    // it in, it's left out
    f.rules(
        car,
        &[(false, artist(1)), (false, release(1)), (false, track(1))],
    );
    remove(release(1));
    assert_eq!(f.rules_of(car), [(false, artist(1)), (true, release(1))]);
    assert_eq!(f.external.track_ids(car).unwrap(), [3, 4]);

    // Leaving out the release covers its tracks
    f.rules(car, &[(false, artist(1)), (true, track(2))]);
    remove(release(1));
    assert_eq!(f.rules_of(car), [(false, artist(1)), (true, release(1))]);

    // With nothing else bringing it in, its rule just goes
    f.rules(car, &[(false, release(3))]);
    remove(release(3));
    assert!(f.rules_of(car).is_empty());

    // A track that comes in with its release is left out
    f.rules(car, &[(false, release(3))]);
    remove(track(5));
    assert_eq!(f.rules_of(car), [(false, release(3)), (true, track(5))]);
    f.rules(car, &[(false, track(5))]);
    remove(track(5));
    assert!(f.rules_of(car).is_empty());

    // An artist's rule goes
    f.rules(car, &[(false, artist(1)), (false, genre("Jazz"))]);
    remove(artist(1));
    remove(genre("Jazz"));
    assert!(f.rules_of(car).is_empty());
}

#[test]
fn memberships_say_why_something_is_in() {
    let f = Fixture::new();
    f.catalog();
    let car = f.folder_library("Car").id;
    let ipod = f.external.create(&ipod("iPod")).unwrap().id;
    f.rules(car, &[(false, artist(1)), (true, track(2))]);
    f.rules(ipod, &[(false, track(5)), (false, genre("Jazz"))]);
    let of = |subject| {
        f.external
            .memberships(subject)
            .unwrap()
            .into_iter()
            .map(|membership| {
                let reason = match membership.reason {
                    Reason::Own => "own".to_string(),
                    Reason::Through(rule) => format!("through {:?}", rule.target),
                    Reason::Tracks => "tracks".to_string(),
                };
                (
                    membership.library.name,
                    reason,
                    membership.tracks,
                    membership.total,
                )
            })
            .collect::<Vec<_>>()
    };
    let row = |name: &str, reason: &str, tracks, total| {
        (name.to_string(), reason.to_string(), tracks, total)
    };
    let through_artist = format!("through {:?}", artist(1));
    let through_jazz = format!("through {:?}", genre("Jazz"));

    assert_eq!(of(Subject::Track(1)), [row("Car", &through_artist, 1, 1)]);
    assert!(of(Subject::Track(2)).is_empty());
    // Its own rule first, though its release's tag brings it in too
    assert_eq!(of(Subject::Track(5)), [row("iPod", "own", 1, 1)]);
    assert_eq!(of(Subject::Track(6)), [row("iPod", &through_jazz, 1, 1)]);
    // A release: in by an artist, with one track left out
    assert_eq!(of(Subject::Release(1)), [row("Car", &through_artist, 1, 2)]);
    assert_eq!(of(Subject::Release(3)), [row("iPod", &through_jazz, 2, 2)]);
    // Only one of Sampler's tracks has the tag
    assert_eq!(of(Subject::Release(4)), [row("iPod", "tracks", 1, 2)]);
    // An artist is in those with a rule for it
    assert_eq!(of(Subject::Artist(1)), [row("Car", "own", 3, 4)]);
    assert!(of(Subject::Artist(2)).is_empty());
}

#[test]
fn contents_count_releases_tracks_and_what_they_take() {
    let f = Fixture::new();
    f.catalog();
    let car = f.folder_library("Car");
    f.rules(car.id, &[(false, release(4))]);
    // In AAC at 256 kbps, a 4-minute FLAC takes 7.68 MB, and a DSF is left
    // out
    let contents = f.external.contents(car.id).unwrap();
    assert_eq!(
        (
            contents.releases,
            contents.tracks,
            contents.size_bytes,
            contents.left_out
        ),
        (1, 2, 7_680_000, 1)
    );
    // Kept as it is, it takes its own size
    f.external
        .update(
            car.id,
            &ExternalLibraryInput {
                format: OutputFormat::Original,
                ..folder("Car", car.folder.as_deref().unwrap())
            },
        )
        .unwrap();
    assert_eq!(f.external.contents(car.id).unwrap().size_bytes, 10_000_000);

    // A sample track for the template: its first, or else the newest
    assert_eq!(f.external.sample_track(car.id).unwrap(), Some(7));
    let ipod = f.external.create(&ipod("iPod")).unwrap();
    assert_eq!(f.external.sample_track(ipod.id).unwrap(), Some(8));
}

#[test]
fn deleting_an_external_library_deletes_its_rules() {
    let f = Fixture::new();
    f.catalog();
    let car = f.folder_library("Car").id;
    f.rules(car, &[(false, artist(1))]);
    f.external.delete(car).unwrap();
    assert!(f.external.get(car).unwrap().is_none());
    let rules: i64 = f
        .store
        .conn()
        .query_row("select count(*) from external_library_rules", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rules, 0);
    // The catalog stays
    let releases: i64 = f
        .store
        .conn()
        .query_row("select count(*) from releases", [], |row| row.get(0))
        .unwrap();
    assert_eq!(releases, 4);
}

#[test]
fn the_database_checks_kinds_and_rules() {
    let f = Fixture::new();
    f.catalog();
    let uuid = |n: i64| format!("00000000-0000-4000-8000-{n:012}");
    let fails = |sql: &str| {
        assert!(f.store.conn().execute_batch(sql).is_err(), "{sql}");
    };
    let library = |n: i64, columns: &str| {
        format!(
            "insert into external_libraries (uuid, name, kind, folder, format, path_template, cd_resolution, embed_covers, cover_file)
            values ('{}', 'L{n}', {columns})",
            uuid(n)
        )
    };
    // A folder library has its folder and template; an iPod has neither, and
    // takes what it plays
    fails(&library(1, "'folder', null, 'flac', 'x', 0, 1, 0"));
    fails(&library(2, "'folder', '/card', 'flac', null, 0, 1, 0"));
    fails(&library(3, "'ipod', '/card', 'alac', null, 1, 0, 0"));
    fails(&library(4, "'ipod', null, 'original', null, 1, 0, 0"));
    fails(&library(5, "'ipod', null, 'alac', null, 0, 0, 0"));
    fails(&library(6, "'ipod', null, 'alac', null, 1, 1, 0"));
    fails(&library(7, "'folder', 'card', 'flac', 'x', 0, 1, 0"));
    fails(
        "insert into external_libraries (uuid, name, kind, format) values ('NOT-A-UUID', 'X', 'ipod', 'aac')",
    );
    f.sql(&library(8, "'folder', '/card', 'flac', 'x', 0, 1, 0"));
    f.sql(&library(9, "'ipod', null, 'aac', null, 1, 0, 0"));

    let rule = |columns: &str, values: &str| {
        format!(
            "insert into external_library_rules (external_library_id, {columns}) values (1, {values})"
        )
    };
    // A rule names exactly one thing, and only a release or a track is left
    // out
    fails(&rule("exclude", "0"));
    fails(&rule("release_id, artist_id", "1, 1"));
    fails(&rule("exclude, artist_id", "1, 1"));
    fails(&rule("exclude, text_tag_id, tag_equals", "1, 1, 'Jazz'"));
    // A tag value rule has its filter, and only it has one
    fails(&rule("text_tag_id", "1"));
    fails(&rule("text_tag_id, tag_equals, tag_min", "1, 'Jazz', 'a'"));
    fails(&rule("real_number_tag_id", "1"));
    fails(&rule(
        "real_number_tag_id, tag_equals, tag_min",
        "1, '8', '6'",
    ));
    fails(&rule("release_id, tag_equals", "1, 'x'"));
    // Each thing once
    f.sql(&rule("release_id", "1"));
    fails(&rule("exclude, release_id", "1, 1"));
    f.sql(&rule("text_tag_id, tag_equals", "1, 'Jazz'"));
    fails(&rule("text_tag_id, tag_equals", "1, 'JAZZ'"));
    f.sql(&rule("real_number_tag_id, tag_min", "1, '8'"));
    fails(&rule("real_number_tag_id, tag_min", "1, '8'"));
    f.sql(&rule("real_number_tag_id, tag_equals", "1, '8'"));
}
