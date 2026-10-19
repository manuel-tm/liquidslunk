//! The library check, on a temporary library: its rules, as
//! docs/development/backend.md lists them ("The library check").

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use slunk::Error;
use slunk::catalog::{Catalog, CreditedArtistInput, FileRef, ReleaseInput, ReleaseTrackInput};
use slunk::library_check::LibraryCheck;
use slunk::media::{FileMove, Library, Media};
use slunk::repo::store::Store;
use slunk::repo::{connection, migrator};
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
    store: Arc<Store>,
    media: Arc<Media>,
    catalog: Arc<Catalog>,
    check: LibraryCheck,
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
        let catalog = Arc::new(Catalog::new(store.clone(), media.clone()));
        Fixture {
            check: LibraryCheck::new(media.clone(), catalog.clone()),
            dir,
            store,
            media,
            catalog,
        }
    }

    fn library(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap().join("library")
    }

    fn put(&self, path: &str, content: &[u8]) {
        let file = self.library().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content).unwrap();
    }

    fn register(&self, path: &str) -> i64 {
        self.media.register_files(&[path.into()]).unwrap()[0].id
    }

    /// Moves a file on disk, as the user might.
    fn rename(&self, from: &str, to: &str) {
        let to = self.library().join(to);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::rename(self.library().join(from), to).unwrap();
    }

    fn number(&self, sql: &str) -> i64 {
        self.store
            .conn()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    }
}

fn fixture_audio(name: &str) -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/audio")
            .join(name),
    )
    .unwrap()
}

#[test]
fn missing_files_are_found_by_their_content() {
    let fixture = Fixture::new();
    fixture.put("music/A/01 One.flac", b"one");
    fixture.put("music/A/02 Two.flac", b"two");
    fixture.put("music/A/03 Three.flac", b"333");
    fixture.put("music/A/cover.jpg", b"jpeg");
    fixture.put("../outside/x.txt", b"x");
    for path in [
        "music/A/01 One.flac",
        "music/A/02 Two.flac",
        "music/A/03 Three.flac",
        "../outside/x.txt",
    ] {
        fixture.register(path);
    }

    // One moved to another folder; one copied twice, under its name and
    // another; one gone; a file of the same size but other content
    fixture.rename("music/A/01 One.flac", "music/B/01 One.flac");
    fixture.put("music/C/02 Two.flac", b"two");
    fixture.put("music/D/other.flac", b"two");
    fs::remove_file(fixture.library().join("music/A/02 Two.flac")).unwrap();
    fs::remove_file(fixture.library().join("music/A/03 Three.flac")).unwrap();
    fixture.put("music/A/04 Four.flac", b"444");
    fixture.put("../outside/y.txt", b"y");

    let report = fixture.check.run().unwrap();
    assert_eq!(report.files.file_count, 4);
    let missing: Vec<(String, Vec<String>, Option<String>)> = report
        .files
        .missing
        .into_iter()
        .map(|missing| (missing.file.path, missing.moved_to, missing.likely_move))
        .collect();
    assert_eq!(
        missing,
        [
            (
                "music/A/01 One.flac".to_string(),
                vec!["music/B/01 One.flac".to_string()],
                Some("music/B/01 One.flac".to_string())
            ),
            (
                "music/A/02 Two.flac".to_string(),
                vec![
                    "music/C/02 Two.flac".to_string(),
                    "music/D/other.flac".to_string()
                ],
                Some("music/C/02 Two.flac".to_string())
            ),
            ("music/A/03 Three.flac".to_string(), vec![], None),
        ]
    );
    // Unregistered files in the folders that hold registered ones, by
    // folder, except files of a missing file's size, which were candidates
    let unregistered: Vec<(String, Vec<String>)> = report
        .files
        .unregistered
        .into_iter()
        .map(|folder| {
            (
                folder.path,
                folder.files.into_iter().map(|file| file.name).collect(),
            )
        })
        .collect();
    assert_eq!(
        unregistered,
        [
            ("../outside".to_string(), vec!["y.txt".to_string()]),
            ("music/A".to_string(), vec!["cover.jpg".to_string()]),
        ]
    );

    // Relinking reads the file again, one move at a time
    let one = fixture.number("select id from files where path = 'music/A/01 One.flac'");
    let relinked = fixture
        .check
        .relink_files(&[FileMove {
            file_id: one,
            path: "music/B/01 One.flac".into(),
        }])
        .unwrap();
    assert_eq!(relinked[0].path, "music/B/01 One.flac");
    match fixture.check.relink_files(&[FileMove {
        file_id: one,
        path: "music/gone.flac".into(),
    }]) {
        Err(Error::Invalid(message)) => assert!(message.contains("music/gone.flac"), "{message}"),
        other => panic!("expected an invalid error, got {other:?}"),
    }
    assert_eq!(fixture.check.run().unwrap().files.missing.len(), 2);
}

#[test]
fn changed_files_are_read_again_with_their_tracks_durations() {
    let fixture = Fixture::new();
    fixture.put("music/1.flac", &fixture_audio("untagged.flac"));
    fixture.put("music/notes.txt", b"notes");
    fixture.put("music/same.txt", b"same");
    let release = fixture
        .catalog
        .create_release(&ReleaseInput {
            title: "A".into(),
            artists: vec![],
            ..ReleaseInput::default()
        })
        .unwrap()
        .id;
    fixture
        .catalog
        .set_release_tracks(
            release,
            &[ReleaseTrackInput {
                file: FileRef {
                    file_id: None,
                    path: Some("music/1.flac".into()),
                },
                cd_number: 1,
                track_number: 1,
                title: "One".into(),
                ..ReleaseTrackInput::default()
            }],
            None,
        )
        .unwrap();
    let audio = fixture.number("select id from files where path = 'music/1.flac'");
    let notes = fixture.register("music/notes.txt");
    fixture.register("music/same.txt");
    assert!(fixture.check.run().unwrap().files.changed.is_empty());

    // New content, of another length, and a new mtime only
    fixture.put("music/1.flac", &fixture_audio("flac.flac"));
    fixture.put("music/notes.txt", b"more notes");
    let touched = fs::File::options()
        .write(true)
        .open(fixture.library().join("music/same.txt"))
        .unwrap();
    touched
        .set_modified(UNIX_EPOCH + Duration::from_secs(1_000_000_000))
        .unwrap();
    let changed: Vec<(String, i64, i64)> = fixture
        .check
        .run()
        .unwrap()
        .files
        .changed
        .into_iter()
        .map(|changed| {
            (
                changed.file.path,
                changed.file.size_bytes,
                changed.size_bytes,
            )
        })
        .collect();
    assert_eq!(changed.len(), 3);
    assert_eq!(changed[1], ("music/notes.txt".to_string(), 5, 10));
    assert_eq!(changed[2].1, changed[2].2);

    // Rehashing saves what's on disk, and the track's new duration
    let rehashed = fixture.check.rehash_files(&[audio, notes, 999]).unwrap();
    assert_eq!(rehashed.len(), 2);
    assert_eq!(rehashed[1].size_bytes, 10);
    assert_eq!(fixture.number("select duration_ms from tracks"), 1001);
    assert_eq!(fixture.check.run().unwrap().files.changed.len(), 1);

    // A file that's gone fails it before anything is saved
    fixture.put("music/notes.txt", b"other notes");
    fs::remove_file(fixture.library().join("music/same.txt")).unwrap();
    let same = fixture.number("select id from files where path = 'music/same.txt'");
    match fixture.check.rehash_files(&[notes, same]) {
        Err(Error::Invalid(message)) => assert_eq!(message, "There's no file at music/same.txt"),
        other => panic!("expected an invalid error, got {other:?}"),
    }
    assert_eq!(
        fixture.number(&format!("select size_bytes from files where id = {notes}")),
        10
    );
}

#[test]
fn the_catalog_s_gaps_are_releases_with_no_artist_and_tracks_nobody_performs() {
    let fixture = Fixture::new();
    fixture.put("music/1.flac", &fixture_audio("untagged.flac"));
    fixture.put("music/2.flac", &fixture_audio("untagged.flac"));
    let artist = fixture
        .catalog
        .create_artist(&slunk::catalog::ArtistInput {
            name: "Alika".into(),
            ..Default::default()
        })
        .unwrap()
        .id;
    let release = |title: &str, artists: Vec<CreditedArtistInput>, various: bool| {
        fixture
            .catalog
            .create_release(&ReleaseInput {
                title: title.into(),
                artists,
                various_artists: various,
                ..ReleaseInput::default()
            })
            .unwrap()
            .id
    };
    let credited = || {
        vec![CreditedArtistInput {
            artist_id: artist,
            credited_as: None,
        }]
    };
    release("b", vec![], false);
    release("A", vec![], false);
    let various = release("Various", credited(), true);
    release("Regular", credited(), false);
    let row = |path: &str, number: i64, title: &str, features: Vec<CreditedArtistInput>| {
        ReleaseTrackInput {
            file: FileRef {
                file_id: None,
                path: Some(path.into()),
            },
            cd_number: 1,
            track_number: number,
            title: title.into(),
            features,
            ..ReleaseTrackInput::default()
        }
    };
    fixture
        .catalog
        .set_release_tracks(
            various,
            &[
                row("music/1.flac", 1, "Featured only", credited()),
                ReleaseTrackInput {
                    artists: credited(),
                    ..row("music/2.flac", 2, "Performed", vec![])
                },
            ],
            None,
        )
        .unwrap();

    let report = fixture.check.run().unwrap();
    let titles: Vec<&str> = report
        .releases_without_artists
        .iter()
        .map(|release| release.title.as_str())
        .collect();
    assert_eq!(titles, ["A", "b"]);
    let tracks: Vec<&str> = report
        .tracks_without_performers
        .iter()
        .map(|track| track.title.as_str())
        .collect();
    assert_eq!(tracks, ["Featured only"]);
}
