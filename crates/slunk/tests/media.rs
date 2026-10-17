//! The media context, on a temporary library: its rules, as slunk's backend
//! doc lists them (docs/development/backend.md, "The media context").

use std::fs::{self, File};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use slunk::Error;
use slunk::media::{
    FileLink, FileListArgs, FileType, FileUse, FileUseKind, Library, LinkOwner, Media,
};
use slunk::paging::ListArgs;
use slunk::repo::store::Store;
use slunk::repo::{connection, file_links, files, migrator};
use tempfile::TempDir;

/// A library in `<tmp>/top/library/`, with `<tmp>/top/outside/` next to it.
/// Temporary folders' names start with a dot, which browsing leaves out, so
/// the library is one folder down.
struct Fixture {
    dir: TempDir,
    store: Arc<Store>,
    media: Media,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        // Resolved, as the app's library paths are
        let root = dir.path().canonicalize().unwrap().join("top");
        fs::create_dir_all(root.join("library")).unwrap();
        fs::create_dir_all(root.join("outside")).unwrap();
        let db = root.join("library/slunk.db");
        migrator::migrate(&db, 10).unwrap();
        let store = Arc::new(Store::new(connection::open(&db).unwrap()));
        let media = Media::new(store.clone(), Library::new(&db));
        Fixture { dir, store, media }
    }

    /// `<tmp>/top/library/`.
    fn library(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap().join("top/library")
    }

    /// Writes a file, relative to the library's folder.
    fn write(&self, path: &str, content: &str) -> PathBuf {
        let file = self.library().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, content).unwrap();
        file
    }

    fn register(&self, paths: &[&str]) -> slunk::Result<Vec<slunk::media::FileRecord>> {
        let paths: Vec<String> = paths.iter().map(|path| path.to_string()).collect();
        self.media.register_files(&paths)
    }

    fn sql(&self, sql: &str) {
        self.store.conn().execute_batch(sql).unwrap();
    }
}

fn invalid(result: slunk::Result<impl std::fmt::Debug>) -> String {
    match result {
        Err(Error::Invalid(message)) => message,
        other => panic!("expected an invalid error, got {other:?}"),
    }
}

#[test]
fn registering_reads_size_mtime_and_hash() {
    let fixture = Fixture::new();
    let file = fixture.write("Ryo Fukui/01 Early Summer.flac", "hello");
    // A few nanoseconds before a second: Node's mtimeMs rounds up to it
    let mtime = SystemTime::UNIX_EPOCH + Duration::new(1_700_000_000, 999_999_950);
    File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(mtime)
        .unwrap();

    let [registered] = &fixture
        .register(&["Ryo Fukui/01 Early Summer.flac"])
        .unwrap()[..]
    else {
        panic!("one file");
    };
    assert_eq!(registered.path, "Ryo Fukui/01 Early Summer.flac");
    assert_eq!(registered.file_type, FileType::Music);
    assert_eq!(registered.size_bytes, 5);
    assert_eq!(registered.mtime, 1_700_000_001);
    assert_eq!(
        registered.sha256,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    // Only read
    assert_eq!(fs::read_to_string(&file).unwrap(), "hello");
}

#[test]
fn paths_are_normalized_and_registered_ones_returned_as_they_are() {
    let fixture = Fixture::new();
    fixture.write("music/a.flac", "a");
    fixture.write("music/cover.JPG", "b");
    fs::write(fixture.library().join("../outside/notes.txt"), "c").unwrap();
    let first = fixture.register(&["music/a.flac"]).unwrap();

    let outside = fixture.library().join("../outside/notes.txt");
    let registered = fixture
        .register(&[
            "./music//cover.JPG",
            "music/../music/a.flac",
            "music/a.flac",
            outside.to_str().unwrap(),
        ])
        .unwrap();
    let paths: Vec<&str> = registered.iter().map(|file| file.path.as_str()).collect();
    // In the order given, each once, and above the library with ../
    assert_eq!(
        paths,
        ["music/cover.JPG", "music/a.flac", "../outside/notes.txt"]
    );
    assert_eq!(registered[1], first[0]);
    assert_eq!(registered[0].file_type, FileType::Image);
    assert_eq!(registered[2].file_type, FileType::Other);
}

#[test]
fn registering_a_folder_or_a_missing_file_registers_nothing() {
    let fixture = Fixture::new();
    fixture.write("music/a.flac", "a");
    assert_eq!(
        invalid(fixture.register(&["music/a.flac", "."])),
        "That's a folder, not a file"
    );
    assert_eq!(
        invalid(fixture.register(&["music/a.flac", "music/gone.flac"])),
        "There's no file at music/gone.flac"
    );
    assert_eq!(
        invalid(fixture.register(&["music"])),
        "There's no file at music"
    );
    assert_eq!(fixture.media.file_counts().unwrap().total, 0);
}

#[test]
fn browsing_follows_symlinks_and_leaves_out_hidden_and_own_files() {
    let fixture = Fixture::new();
    let library = fixture.library();
    fixture.write("music/10 Song.flac", "10");
    fixture.write("music/2 Song.flac", "2");
    fixture.write("music/.DS_Store", "");
    fixture.write("music/CD1/1 Intro.flac", "1");
    fixture.write("music/.hidden/x.flac", "x");
    fs::create_dir_all(library.join("slunk-backups")).unwrap();
    fixture.write("slunk-backups/slunk-20261017000000.db", "");
    symlink(library.join("../outside"), library.join("music/Elsewhere")).unwrap();
    symlink(
        library.join("music/2 Song.flac"),
        library.join("music/Alias.flac"),
    )
    .unwrap();
    symlink(
        library.join("music/gone.flac"),
        library.join("music/Broken.flac"),
    )
    .unwrap();
    fixture
        .register(&["music/CD1/1 Intro.flac", "music/2 Song.flac"])
        .unwrap();

    let folder = fixture.media.browse_folder("music").unwrap();
    assert_eq!(folder.path, "music");
    assert_eq!(
        folder.absolute_path,
        library.join("music").to_str().unwrap()
    );
    assert_eq!(folder.parent.as_deref(), Some(""));
    let folders: Vec<(&str, &str, bool)> = folder
        .folders
        .iter()
        .map(|sub| {
            (
                sub.name.as_str(),
                sub.path.as_str(),
                sub.has_registered_files,
            )
        })
        .collect();
    assert_eq!(
        folders,
        [
            ("CD1", "music/CD1", true),
            ("Elsewhere", "music/Elsewhere", false)
        ]
    );
    let files: Vec<(&str, bool)> = folder
        .files
        .iter()
        .map(|file| (file.disk.name.as_str(), file.file.is_some()))
        .collect();
    assert_eq!(
        files,
        [
            ("2 Song.flac", true),
            ("10 Song.flac", false),
            ("Alias.flac", false)
        ]
    );

    // The database, its -wal and -shm, and slunk-backups/ aren't listed
    let top = fixture.media.browse_folder("").unwrap();
    assert_eq!(top.parent.as_deref(), Some(".."));
    assert!(top.files.is_empty(), "{:?}", top.files);
    let names: Vec<&str> = top.folders.iter().map(|sub| sub.name.as_str()).collect();
    assert_eq!(names, ["music"]);

    // At the top of the disk, there's no parent
    let root = fixture.media.browse_folder("/").unwrap();
    assert_eq!(root.parent, None);
    assert_eq!(root.absolute_path, "/");
    assert!(root.path.starts_with(".."), "{}", root.path);

    assert_eq!(
        invalid(fixture.media.browse_folder("nowhere")),
        "Can't read the folder nowhere"
    );
    assert_eq!(
        invalid(fixture.media.browse_folder("music/2 Song.flac")),
        "Can't read the folder music/2 Song.flac"
    );
}

#[test]
fn folders_above_the_library_know_what_they_hold() {
    let fixture = Fixture::new();
    fs::write(fixture.library().join("../outside/a.flac"), "a").unwrap();
    fs::create_dir_all(fixture.library().join("../empty")).unwrap();
    fixture.write("..odd/b.flac", "b");

    let above = fixture.media.browse_folder("..").unwrap();
    let holds = |above: &slunk::media::BrowsedFolder| -> Vec<(String, bool)> {
        above
            .folders
            .iter()
            .map(|sub| (sub.path.clone(), sub.has_registered_files))
            .collect()
    };
    assert_eq!(
        holds(&above),
        [
            ("../empty".to_string(), false),
            ("".to_string(), false),
            ("../outside".to_string(), false)
        ]
    );

    // A folder named "..odd" is in the library, not above it
    fixture.register(&["..odd/b.flac"]).unwrap();
    let above = fixture.media.browse_folder("..").unwrap();
    assert_eq!(holds(&above)[1], ("".to_string(), true));

    fixture.register(&["../outside/a.flac"]).unwrap();
    let above = fixture.media.browse_folder("..").unwrap();
    assert_eq!(holds(&above)[2], ("../outside".to_string(), true));
    // The folder above that holds everything
    let top = fixture.media.browse_folder("../..").unwrap();
    assert_eq!(top.folders.len(), 1);
    assert_eq!(top.folders[0].path, "..");
    assert!(top.folders[0].has_registered_files);
}

#[test]
fn scanning_lists_every_file_below_without_following_symlinked_folders() {
    let fixture = Fixture::new();
    let library = fixture.library();
    fixture.write("music/B/10 Song.flac", "10");
    fixture.write("music/B/2 Song.flac", "2");
    fixture.write("music/a.jpg", "a");
    fixture.write("music/.DS_Store", "");
    fixture.write("music/.hidden/x.flac", "x");
    fs::write(library.join("../outside/far.flac"), "far").unwrap();
    symlink(library.join("../outside"), library.join("music/Elsewhere")).unwrap();
    symlink(library.join("music/a.jpg"), library.join("music/Alias.jpg")).unwrap();
    symlink(
        library.join("music/gone"),
        library.join("music/Broken.flac"),
    )
    .unwrap();
    fixture.register(&["music/a.jpg"]).unwrap();

    let found = fixture.media.scan_folder("music").unwrap();
    let paths: Vec<(&str, bool)> = found
        .iter()
        .map(|file| (file.disk.path.as_str(), file.file.is_some()))
        .collect();
    assert_eq!(
        paths,
        [
            ("music/a.jpg", true),
            ("music/Alias.jpg", false),
            ("music/B/2 Song.flac", false),
            ("music/B/10 Song.flac", false),
        ]
    );
    assert_eq!(found[2].disk.name, "2 Song.flac");
    assert_eq!(found[2].disk.size_bytes, 1);

    // The library's own files aren't listed
    let everything = fixture.media.scan_folder("").unwrap();
    assert!(
        everything
            .iter()
            .all(|file| file.disk.path.starts_with("music/"))
    );

    assert_eq!(
        invalid(fixture.media.scan_folder("music/a.jpg")),
        "Can't read the folder music/a.jpg"
    );
}

#[test]
fn updating_changes_the_type_or_relinks_a_file_that_moved() {
    let fixture = Fixture::new();
    fixture.write("old/a.flac", "old");
    fixture.write("new/a.flac", "moved");
    fixture.write("b.flac", "b");
    let [a, b] = &fixture.register(&["old/a.flac", "b.flac"]).unwrap()[..] else {
        panic!("two files");
    };
    fixture.sql(&format!(
        "update files set updated_at = 0 where id = {}",
        a.id
    ));

    let typed = fixture
        .media
        .update_file(a.id, None, Some(FileType::Other))
        .unwrap();
    assert_eq!(typed.file_type, FileType::Other);
    assert_eq!(typed.sha256, a.sha256);
    assert!(typed.updated_at > 0);

    let relinked = fixture
        .media
        .update_file(a.id, Some("./new//a.flac"), Some(FileType::Music))
        .unwrap();
    assert_eq!(relinked.path, "new/a.flac");
    assert_eq!(relinked.size_bytes, 5);
    assert_ne!(relinked.sha256, a.sha256);

    // The same path isn't read again, so a file that's gone keeps its facts
    fs::remove_file(fixture.library().join("new/a.flac")).unwrap();
    let same = fixture
        .media
        .update_file(a.id, Some("new/a.flac"), None)
        .unwrap();
    assert_eq!(same.sha256, relinked.sha256);

    assert_eq!(
        invalid(fixture.media.update_file(a.id, Some("b.flac"), None)),
        format!("File {} is already registered at b.flac", b.id)
    );
    assert_eq!(
        invalid(fixture.media.update_file(a.id, Some("gone.flac"), None)),
        "There's no file at gone.flac"
    );
    match fixture.media.update_file(99, None, Some(FileType::Music)) {
        Err(Error::NotFound(message)) => assert_eq!(message, "There's no file 99"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_tracks_audio_stays_music_and_a_cover_an_image() {
    let fixture = Fixture::new();
    fixture.write("a.flac", "a");
    fixture.write("cover.jpg", "c");
    let [audio, cover] = &fixture.register(&["a.flac", "cover.jpg"]).unwrap()[..] else {
        panic!("two files");
    };
    fixture.sql(&format!(
        "insert into releases (id, title, cover_picture) values (1, 'Scenery', {});
         insert into tracks (title, track_number, release_id, file_id) values ('Early Summer', 1, 1, {});",
        cover.id, audio.id
    ));
    let error = fixture
        .media
        .update_file(audio.id, None, Some(FileType::Image));
    assert!(
        matches!(&error, Err(Error::Sqlite(error)) if error.to_string().contains("a track's file must stay music")),
        "{error:?}"
    );
    let error = fixture
        .media
        .update_file(cover.id, None, Some(FileType::Other));
    assert!(
        matches!(&error, Err(Error::Sqlite(error)) if error.to_string().contains("a cover or picture must stay an image")),
        "{error:?}"
    );
}

#[test]
fn deleting_removes_only_the_record_and_not_while_anything_uses_it() {
    let fixture = Fixture::new();
    let disk = fixture.write("a.jpg", "a");
    fixture.write("b.jpg", "b");
    let [a, b] = &fixture.register(&["a.jpg", "b.jpg"]).unwrap()[..] else {
        panic!("two files");
    };
    fixture.sql(&format!(
        "insert into artists (name, picture) values ('Alika', {});
         insert into labels (name, picture) values ('Spinnin', {});",
        b.id, b.id
    ));

    fixture.media.delete_file(a.id).unwrap();
    assert_eq!(fixture.media.get_file(a.id).unwrap(), None);
    assert!(disk.exists());
    assert_eq!(
        invalid(fixture.media.delete_file(b.id)),
        r#"The file is still used: the picture of "Alika", the logo of "Spinnin""#
    );
    // An unknown id deletes nothing, as in slunk
    fixture.media.delete_file(99).unwrap();
}

#[test]
fn uses_name_everything_that_uses_a_file() {
    let fixture = Fixture::new();
    fixture.write("a.flac", "a");
    fixture.write("a.jpg", "b");
    let [audio, image] = &fixture.register(&["a.flac", "a.jpg"]).unwrap()[..] else {
        panic!("two files");
    };
    let (audio, image) = (audio.id, image.id);
    fixture.sql(&format!(
        "insert into releases (id, title, cover_picture) values (1, 'Scenery', {image});
         insert into tracks (id, title, track_number, release_id, file_id) values (1, 'Early Summer', 1, 1, {audio});
         insert into artists (id, name, picture) values (1, 'Ryo Fukui', {image});
         insert into labels (id, name, picture) values (1, 'Trio', {image});
         insert into files_releases (release_id, file_id, note) values (1, {image}, 'Back');
         insert into artists_files (artist_id, file_id) values (1, {image});
         insert into files_tracks (track_id, file_id, note) values (1, {image}, 'Lyrics');"
    ));
    let uses = fixture.media.file_uses(image).unwrap();
    let use_of = |kind, entity_id, name: &str, note: Option<&str>| FileUse {
        kind,
        entity_id,
        name: name.into(),
        note: note.map(Into::into),
    };
    assert_eq!(
        uses,
        [
            use_of(FileUseKind::ReleaseCover, 1, "Scenery", None),
            use_of(FileUseKind::ArtistPicture, 1, "Ryo Fukui", None),
            use_of(FileUseKind::LabelPicture, 1, "Trio", None),
            use_of(FileUseKind::ReleaseLink, 1, "Scenery", Some("Back")),
            use_of(FileUseKind::ArtistLink, 1, "Ryo Fukui", None),
            use_of(FileUseKind::TrackLink, 1, "Early Summer", Some("Lyrics")),
        ]
    );
    assert_eq!(
        fixture.media.file_uses(audio).unwrap(),
        [use_of(FileUseKind::TrackAudio, 1, "Early Summer", None)]
    );
    let tracks = fixture.media.tracks_of_files(&[audio, image]).unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[&audio].title, "Early Summer");
    let counts = fixture.media.file_counts().unwrap();
    assert_eq!((counts.total, counts.unused), (2, 0));
}

#[test]
fn lists_page_sort_search_and_filter() {
    let fixture = Fixture::new();
    for path in [
        "b/2 Song.flac",
        "a/50%_off.flac",
        "a/50 off.flac",
        "c/Cover.JPG",
    ] {
        fixture.write(path, path);
    }
    let registered = fixture
        .register(&[
            "b/2 Song.flac",
            "a/50%_off.flac",
            "a/50 off.flac",
            "c/Cover.JPG",
        ])
        .unwrap();
    fixture.sql(&format!(
        "insert into releases (id, title) values (1, 'Scenery');
         insert into tracks (title, track_number, release_id, file_id) values ('Song', 1, 1, {});",
        registered[0].id
    ));
    let list = |args: FileListArgs| -> Vec<String> {
        let page = fixture.media.list_files(&args).unwrap();
        page.items.into_iter().map(|file| file.path).collect()
    };
    let args = |sort: Option<&str>, q: Option<&str>| FileListArgs {
        list: ListArgs {
            sort: sort.map(Into::into),
            q: q.map(Into::into),
            ..ListArgs::default()
        },
        ..FileListArgs::default()
    };

    // Newest first by default
    assert_eq!(
        list(args(None, None)),
        [
            "c/Cover.JPG",
            "a/50 off.flac",
            "a/50%_off.flac",
            "b/2 Song.flac"
        ]
    );
    assert_eq!(
        list(args(Some("path"), None)),
        [
            "a/50 off.flac",
            "a/50%_off.flac",
            "b/2 Song.flac",
            "c/Cover.JPG"
        ]
    );
    assert_eq!(list(args(Some("-path"), None))[0], "c/Cover.JPG");
    // % and _ are only themselves, and case is ignored
    assert_eq!(list(args(None, Some("50%_"))), ["a/50%_off.flac"]);
    assert_eq!(list(args(None, Some(" cover "))), ["c/Cover.JPG"]);
    assert_eq!(
        list(FileListArgs {
            file_type: Some(FileType::Image),
            ..FileListArgs::default()
        }),
        ["c/Cover.JPG"]
    );
    assert_eq!(
        list(FileListArgs {
            without_track: true,
            list: ListArgs {
                sort: Some("path".into()),
                ..ListArgs::default()
            },
            ..FileListArgs::default()
        }),
        ["a/50 off.flac", "a/50%_off.flac", "c/Cover.JPG"]
    );
    assert_eq!(
        list(FileListArgs {
            unused: true,
            file_type: Some(FileType::Music),
            ..FileListArgs::default()
        }),
        ["a/50 off.flac", "a/50%_off.flac"]
    );

    let page = fixture
        .media
        .list_files(&FileListArgs {
            list: ListArgs {
                page: Some(2),
                per_page: Some(3),
                sort: Some("path".into()),
                ..ListArgs::default()
            },
            ..FileListArgs::default()
        })
        .unwrap();
    assert_eq!(page.total, 4);
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        invalid(fixture.media.list_files(&args(Some("size"), None))),
        "Files can't be sorted by size"
    );
}

#[test]
fn file_links_keep_their_order_and_saving_replaces_them() {
    let fixture = Fixture::new();
    for path in ["a.jpg", "b.jpg", "c.log"] {
        fixture.write(path, path);
    }
    let ids: Vec<i64> = fixture
        .register(&["a.jpg", "b.jpg", "c.log"])
        .unwrap()
        .iter()
        .map(|file| file.id)
        .collect();
    fixture.sql("insert into releases (id, title) values (1, 'Scenery'), (2, 'Hotel California');");
    let link = |file_id, note: Option<&str>| FileLink {
        file_id,
        note: note.map(Into::into),
    };
    fixture
        .store
        .transaction(|tx| {
            file_links::set(
                tx,
                LinkOwner::Release,
                1,
                &[link(ids[2], Some("Rip log")), link(ids[0], None)],
            )?;
            file_links::set(tx, LinkOwner::Release, 2, &[link(ids[1], None)])
        })
        .unwrap();
    let links = fixture
        .media
        .file_links_of(LinkOwner::Release, &[1, 2, 3])
        .unwrap();
    assert_eq!(
        links[&1],
        [link(ids[2], Some("Rip log")), link(ids[0], None)]
    );
    assert_eq!(links[&2], [link(ids[1], None)]);
    assert!(!links.contains_key(&3));

    fixture
        .store
        .transaction(|tx| file_links::set(tx, LinkOwner::Release, 1, &[link(ids[1], None)]))
        .unwrap();
    let links = fixture
        .media
        .file_links_of(LinkOwner::Release, &[1])
        .unwrap();
    assert_eq!(links[&1], [link(ids[1], None)]);
}

#[test]
fn many_ids_are_read_in_chunks() {
    let fixture = Fixture::new();
    let paths: Vec<String> = (0..1200).map(|n| format!("many/{n}.txt")).collect();
    for path in &paths {
        fixture.write(path, path);
    }
    let registered = fixture.media.register_files(&paths).unwrap();
    assert_eq!(registered.len(), 1200);
    let ids: Vec<i64> = registered.iter().map(|file| file.id).collect();
    assert_eq!(fixture.media.files_by_ids(&ids).unwrap().len(), 1200);
    assert_eq!(
        files::find_by_paths(&fixture.store.conn(), &paths)
            .unwrap()
            .len(),
        1200
    );
    // Ids follow the order the paths were given in
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn a_registered_files_path_resolves_lexically() {
    let fixture = Fixture::new();
    fs::write(fixture.library().join("../outside/a.flac"), "a").unwrap();
    let [file] = &fixture.register(&["../outside/a.flac"]).unwrap()[..] else {
        panic!("one file");
    };
    let expected: &Path = &fixture.library().join("..").join("outside/a.flac");
    let resolved = fixture.media.file_path(file.id).unwrap().unwrap();
    assert_eq!(resolved, fs::canonicalize(expected).unwrap());
    assert!(fixture.media.file_on_disk(file));
    assert_eq!(fixture.media.file_path(99).unwrap(), None);
}
