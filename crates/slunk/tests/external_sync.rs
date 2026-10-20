//! Syncing a folder library, from start to end, in a temporary folder, with
//! the audio fixtures, as docs/development/backend.md lists it ("Syncing a
//! folder library"). They run `afconvert`, so only on macOS.
#![cfg(target_os = "macos")]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use lofty::file::TaggedFileExt;
use lofty::prelude::*;
use slunk::Error;
use slunk::external::plan::{Action, Subject};
use slunk::external::{
    External, ExternalKind, ExternalLibraryInput, OutputFormat, RuleTarget, RuleValues, SyncPlan,
};
use slunk::media::{Library, Media};
use slunk::repo::store::Store;
use slunk::repo::{connection, migrator};
use tempfile::TempDir;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/audio");

struct Fixture {
    dir: TempDir,
    store: Arc<Store>,
    external: External,
}

impl Fixture {
    /// A library with two releases: Ryo Fukui's Scenery, in FLAC, WAV and
    /// Apple Lossless, with a PNG cover, and Pink Floyd's Animals, in MP3
    /// and AAC, with no cover.
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let root = base.join("library");
        fs::create_dir_all(root.join("music")).unwrap();
        let db = root.join("slunk.db");
        migrator::migrate(&db, 10).unwrap();
        let store = Arc::new(Store::new(connection::open(&db).unwrap()));
        let media = Arc::new(Media::new(store.clone(), Library::new(&db)));
        let fixture = Fixture {
            external: External::new(store.clone(), media),
            store,
            dir,
        };
        let files = [
            (1, "flac.flac"),
            (2, "wav.wav"),
            (3, "alac.m4a"),
            (4, "cbr.mp3"),
            (5, "aac.m4a"),
        ];
        for (id, name) in files {
            let path = root.join("music").join(format!("{id}-{name}"));
            fs::copy(Path::new(FIXTURES).join(name), &path).unwrap();
            fixture.sql(&format!(
                "insert into files (id, path, type, size_bytes, mtime, sha256)
                values ({id}, 'music/{id}-{name}', 'music', {}, 0, '{}')",
                fs::metadata(&path).unwrap().len(),
                format!("{id}").repeat(64)
            ));
        }
        let cover = root.join("music").join("cover.png");
        image::RgbImage::from_pixel(800, 800, image::Rgb([30, 60, 90]))
            .save(&cover)
            .unwrap();
        fixture.sql(&format!(
            "insert into files (id, path, type, size_bytes, mtime, sha256)
            values (6, 'music/cover.png', 'image', {}, 0, '{}')",
            fs::metadata(&cover).unwrap().len(),
            "6".repeat(64)
        ));
        fixture.sql(
            "insert into artists (id, name) values (1, 'Ryo Fukui'), (2, 'Pink Floyd');
            insert into releases (id, title, various_artists, release_date, release_date_precision, cover_picture)
                values (1, 'Scenery', 0, 189302400, 'year', 6), (2, 'Animals', 0, null, null, null);
            insert into artists_releases (artist_id, release_id, position) values (1, 1, 0), (2, 2, 0);
            insert into tracks (id, title, track_number, cd_number, release_id, file_id, duration_ms) values
                (1, 'It Could Happen to You', 1, 1, 1, 1, 1001),
                (2, 'Early Summer', 2, 1, 1, 2, 1000),
                (3, 'Willow Weep for Me', 3, 1, 1, 3, 1000),
                (4, 'Pigs on the Wing', 1, 1, 2, 4, 1019),
                (5, 'Dogs', 2, 1, 2, 5, 1280);",
        );
        fixture
    }

    fn sql(&self, sql: &str) {
        self.store.conn().execute_batch(sql).unwrap();
    }

    fn car(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap().join("car")
    }

    /// Car, a folder library of both releases, in AAC, with covers
    /// embedded and in each release's folder.
    fn folder_library(&self) -> i64 {
        fs::create_dir_all(self.car()).unwrap();
        let library = self
            .external
            .create(&self.input(OutputFormat::Aac))
            .unwrap();
        let rules = [RuleTarget::Release(1), RuleTarget::Release(2)]
            .into_iter()
            .map(|target| RuleValues {
                exclude: false,
                target,
            })
            .collect::<Vec<_>>();
        self.external.set_rules(library.id, &rules).unwrap();
        library.id
    }

    fn input(&self, format: OutputFormat) -> ExternalLibraryInput {
        ExternalLibraryInput {
            name: "Car".into(),
            kind: ExternalKind::Folder,
            folder: Some(self.car().to_string_lossy().to_string()),
            format,
            aac_kbps: 128,
            cd_resolution: false,
            path_template: None,
            embed_covers: true,
            cover_px: 300,
            cover_file: true,
        }
    }

    fn plan(&self, id: i64) -> Arc<SyncPlan> {
        self.external.plan_sync(id).unwrap()
    }

    fn sync(&self, id: i64) -> slunk::external::sync::Outcome {
        let plan = self.plan(id);
        self.external.sync(id, plan.id, &|_| {}).unwrap()
    }

    /// Every file under Car, relative to it, sorted.
    fn files(&self) -> Vec<String> {
        let car = self.car();
        let mut files: Vec<String> = walkdir(&car)
            .into_iter()
            .map(|path| {
                path.strip_prefix(&car)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        files.sort();
        files
    }
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(walkdir(&path));
        } else {
            found.push(path);
        }
    }
    found
}

/// Each step's action, by what it's about.
fn actions(plan: &SyncPlan) -> HashMap<Subject, Action> {
    plan.plan
        .steps
        .iter()
        .map(|step| (step.subject, step.action))
        .collect()
}

fn tag(path: &Path, key: ItemKey) -> Option<String> {
    let file = lofty::read_from_path(path).unwrap();
    let tag = file.primary_tag()?;
    tag.get_string(key).map(str::to_string)
}

const SCENERY: &str = "Ryo Fukui/1976 - Scenery";
const ANIMALS: &str = "Pink Floyd/Animals";

#[test]
fn a_folder_library_is_synced_and_follows_the_catalog() {
    let f = Fixture::new();
    let car = f.folder_library();

    // The first sync adds everything, converting lossless sources to AAC
    // and copying the lossy ones, with a cover.jpg for the release that has
    // a cover
    let plan = f.plan(car);
    assert_eq!(plan.plan.steps.len(), 6);
    assert!(
        plan.plan
            .steps
            .iter()
            .all(|step| step.action == Action::Add)
    );
    assert!(plan.fits());
    let outcome = f.external.sync(car, plan.id, &|_| {}).unwrap();
    assert_eq!(outcome.done[&Action::Add], 6, "{outcome:?}");
    assert!(outcome.failed.is_empty());
    assert_eq!(
        f.files(),
        vec![
            ".liquidslunk/manifest.json".to_string(),
            format!("{ANIMALS}/01 Pigs on the Wing.mp3"),
            format!("{ANIMALS}/02 Dogs.m4a"),
            format!("{SCENERY}/01 It Could Happen to You.m4a"),
            format!("{SCENERY}/02 Early Summer.m4a"),
            format!("{SCENERY}/03 Willow Weep for Me.m4a"),
            format!("{SCENERY}/cover.jpg"),
        ]
    );
    let first = f.car().join(SCENERY).join("01 It Could Happen to You.m4a");
    assert_eq!(
        tag(&first, ItemKey::TrackTitle).as_deref(),
        Some("It Could Happen to You")
    );
    assert_eq!(
        tag(&first, ItemKey::TrackArtist).as_deref(),
        Some("Ryo Fukui")
    );
    assert_eq!(tag(&first, ItemKey::AlbumTitle).as_deref(), Some("Scenery"));
    let tagged = lofty::read_from_path(&first).unwrap();
    let cover = &tagged.primary_tag().unwrap().pictures()[0];
    let image = image::load_from_memory(cover.data()).unwrap();
    assert_eq!((image.width(), image.height()), (300, 300));
    let mp3 = f.car().join(ANIMALS).join("01 Pigs on the Wing.mp3");
    assert_eq!(
        tag(&mp3, ItemKey::TrackTitle).as_deref(),
        Some("Pigs on the Wing")
    );
    assert!(
        lofty::read_from_path(&mp3)
            .unwrap()
            .primary_tag()
            .unwrap()
            .pictures()
            .is_empty()
    );
    let library = f.external.get(car).unwrap().unwrap();
    assert!(library.last_synced_at.is_some());

    // A second sync has nothing to do
    let plan = f.plan(car);
    assert!(plan.plan.steps.is_empty(), "{:?}", plan.plan.steps);
    assert_eq!(plan.plan.unchanged, 6);

    // Renaming an artist moves and retags their files, and the folders left
    // empty go
    f.sql("update artists set name = 'Ryo Fukui Trio' where id = 1");
    let plan = f.plan(car);
    let steps: Vec<_> = plan
        .plan
        .steps
        .iter()
        .filter(|step| step.action == Action::Move)
        .collect();
    assert_eq!(steps.len(), 3);
    assert_eq!(
        steps[0].from.as_deref(),
        Some(format!("{SCENERY}/01 It Could Happen to You.m4a").as_str())
    );
    let changed: Vec<&str> = steps[0]
        .changes
        .iter()
        .map(|change| change.field.as_str())
        .collect();
    assert_eq!(changed, vec!["Artist", "Artists", "Album artist"]);
    // Its cover.jpg goes with the folder
    let covers: Vec<(Action, &str)> = plan
        .plan
        .steps
        .iter()
        .filter(|step| step.subject == Subject::Release(1))
        .map(|step| (step.action, step.path.as_str()))
        .collect();
    assert_eq!(
        covers,
        vec![
            (Action::Add, "Ryo Fukui Trio/1976 - Scenery/cover.jpg"),
            (Action::Remove, "Ryo Fukui/1976 - Scenery/cover.jpg"),
        ]
    );
    f.external.sync(car, plan.id, &|_| {}).unwrap();
    let moved = f
        .car()
        .join("Ryo Fukui Trio/1976 - Scenery/01 It Could Happen to You.m4a");
    assert_eq!(
        tag(&moved, ItemKey::TrackArtist).as_deref(),
        Some("Ryo Fukui Trio")
    );
    assert!(!f.car().join("Ryo Fukui").exists());
    assert!(
        f.car()
            .join("Ryo Fukui Trio/1976 - Scenery/cover.jpg")
            .exists()
    );
    assert!(f.plan(car).plan.steps.is_empty());

    // Changing the format converts the lossless sources again, and the
    // copies stay
    f.external
        .update(car, &f.input(OutputFormat::Alac))
        .unwrap();
    let plan = f.plan(car);
    let converted: Vec<Subject> = plan
        .plan
        .steps
        .iter()
        .filter(|step| step.action == Action::Convert)
        .map(|step| step.subject)
        .collect();
    assert_eq!(
        converted,
        vec![Subject::Track(1), Subject::Track(2), Subject::Track(3)]
    );
    let outcome = f.external.sync(car, plan.id, &|_| {}).unwrap();
    assert_eq!(outcome.done[&Action::Convert], 3);
    let alac = lofty::read_from_path(&moved).unwrap();
    assert_eq!(alac.properties().bit_depth(), Some(16));

    // Removing a release removes its files, and its folders when they're
    // empty. Someone else's file stays, and so does its folder
    fs::write(f.car().join("Pink Floyd/notes.txt"), b"mine").unwrap();
    f.external.remove(car, &RuleTarget::Release(2)).unwrap();
    let plan = f.plan(car);
    assert_eq!(actions(&plan)[&Subject::Track(4)], Action::Remove);
    f.external.sync(car, plan.id, &|_| {}).unwrap();
    assert!(!f.car().join(ANIMALS).exists());
    assert!(f.car().join("Pink Floyd/notes.txt").exists());

    // A file liquidslunk didn't write, where a track would go, is never
    // replaced: the track is left out
    f.external
        .add(
            car,
            &slunk::external::Additions {
                releases: vec![2],
                ..Default::default()
            },
        )
        .unwrap();
    fs::create_dir_all(f.car().join(ANIMALS)).unwrap();
    fs::write(f.car().join(ANIMALS).join("02 Dogs.m4a"), b"theirs").unwrap();
    let plan = f.plan(car);
    assert!(
        plan.plan
            .left_out
            .iter()
            .any(|left| left.subject == Subject::Track(5)
                && left.reason
                    == format!("A file liquidslunk didn't write is at {ANIMALS}/02 Dogs.m4a"))
    );
    f.external.sync(car, plan.id, &|_| {}).unwrap();
    assert_eq!(
        fs::read(f.car().join(ANIMALS).join("02 Dogs.m4a")).unwrap(),
        b"theirs"
    );

    // Deleting it with its files removes what liquidslunk wrote, and only
    // that
    f.external.delete(car, true).unwrap();
    assert_eq!(
        f.files(),
        vec![
            format!("{ANIMALS}/02 Dogs.m4a"),
            "Pink Floyd/notes.txt".to_string()
        ]
    );
}

#[test]
fn a_cancelled_sync_carries_on_at_the_next_one() {
    let f = Fixture::new();
    let car = f.folder_library();
    let plan = f.plan(car);
    let written = AtomicUsize::new(0);
    let outcome = f
        .external
        .sync(car, plan.id, &|progress| {
            if progress.done >= 2 && written.fetch_add(1, Ordering::Relaxed) == 0 {
                f.external.cancel_sync();
            }
        })
        .unwrap();
    assert!(outcome.cancelled);
    let done: usize = outcome.done.values().sum();
    assert!((2..6).contains(&done), "{outcome:?}");
    // It wasn't a whole sync
    assert!(
        f.external
            .get(car)
            .unwrap()
            .unwrap()
            .last_synced_at
            .is_none()
    );
    let plan = f.plan(car);
    assert_eq!(plan.plan.steps.len(), 6 - done);
    assert_eq!(plan.plan.unchanged, done);
    f.external.sync(car, plan.id, &|_| {}).unwrap();
    assert!(f.plan(car).plan.steps.is_empty());
}

#[test]
fn what_isnt_as_written_is_restored() {
    let f = Fixture::new();
    let car = f.folder_library();
    f.sync(car);
    let dogs = f.car().join(ANIMALS).join("02 Dogs.m4a");
    fs::remove_file(&dogs).unwrap();
    fs::write(
        f.car().join(SCENERY).join("02 Early Summer.m4a"),
        b"changed",
    )
    .unwrap();
    let plan = f.plan(car);
    let actions = actions(&plan);
    assert_eq!(actions[&Subject::Track(5)], Action::Restore);
    assert_eq!(actions[&Subject::Track(2)], Action::Restore);
    f.external.sync(car, plan.id, &|_| {}).unwrap();
    assert!(dogs.exists());
    assert!(f.plan(car).plan.steps.is_empty());
}

#[test]
fn a_missing_source_leaves_its_copy_where_it_is() {
    let f = Fixture::new();
    let car = f.folder_library();
    f.sync(car);
    let root = f.dir.path().canonicalize().unwrap().join("library");
    fs::remove_file(root.join("music/4-cbr.mp3")).unwrap();
    let plan = f.plan(car);
    assert!(plan.plan.steps.is_empty(), "{:?}", plan.plan.steps);
    assert_eq!(
        plan.plan.left_out[0].reason,
        "Its file is missing from the library: music/4-cbr.mp3"
    );
    assert!(
        f.car()
            .join(ANIMALS)
            .join("01 Pigs on the Wing.mp3")
            .exists()
    );
}

#[test]
fn a_plan_is_run_as_it_was_shown() {
    let f = Fixture::new();
    let car = f.folder_library();
    let plan = f.plan(car);
    let again = f.plan(car);
    let stale = f.external.sync(car, plan.id, &|_| {});
    assert!(
        matches!(stale, Err(Error::Invalid(message)) if message == "The plan is out of date. Look at it again.")
    );
    f.external.sync(car, again.id, &|_| {}).unwrap();

    // A folder that belongs to another external library is refused
    let other = f
        .external
        .create(&ExternalLibraryInput {
            name: "Other".into(),
            folder: Some(
                f.dir
                    .path()
                    .canonicalize()
                    .unwrap()
                    .join("other")
                    .to_string_lossy()
                    .to_string(),
            ),
            ..f.input(OutputFormat::Aac)
        })
        .unwrap();
    let refused = f.external.locate(other.id, &f.car().to_string_lossy());
    assert!(
        matches!(refused, Err(Error::Invalid(message)) if message == "This folder belongs to another external library, Car.")
    );
}

#[test]
fn files_that_trade_paths_are_moved_aside_first() {
    let f = Fixture::new();
    let car = f.folder_library();
    f.external
        .update(
            car,
            &ExternalLibraryInput {
                path_template: Some("${album}/${track_number}.${format}".into()),
                ..f.input(OutputFormat::Aac)
            },
        )
        .unwrap();
    f.sync(car);
    let before = fs::read(f.car().join("Scenery/1.m4a")).unwrap();
    f.sql(
        "update tracks set track_number = 99 where id = 1;
        update tracks set track_number = 1 where id = 2;
        update tracks set track_number = 2 where id = 1;",
    );
    let plan = f.plan(car);
    let moves = plan
        .plan
        .steps
        .iter()
        .filter(|step| step.action == Action::Move)
        .count();
    assert_eq!(moves, 2);
    f.external.sync(car, plan.id, &|_| {}).unwrap();
    // Track 1's file is now 2.m4a, retagged with its new number
    let moved = f.car().join("Scenery/2.m4a");
    assert_eq!(
        tag(&moved, ItemKey::TrackTitle).as_deref(),
        Some("It Could Happen to You")
    );
    assert_ne!(fs::read(&moved).unwrap(), before);
    assert_eq!(
        tag(&f.car().join("Scenery/1.m4a"), ItemKey::TrackTitle).as_deref(),
        Some("Early Summer")
    );
    assert!(f.plan(car).plan.steps.is_empty());
    assert!(!f.files().iter().any(|file| file.contains(".moving")));
}
