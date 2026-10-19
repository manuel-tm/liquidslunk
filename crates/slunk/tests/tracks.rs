//! The catalog context's tracks and the import's suggestions, on a temporary
//! library: their rules, as docs/development/backend.md lists them
//! ("Tracks", "Import suggestions").

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use slunk::Error;
use slunk::catalog::{
    AlternateTitleInput, ArtistInput, ArtistLink, Catalog, CreditInput, CreditOwner,
    CreditedArtistInput, DiscTitleInput, FileLinkInput, FileRef, ReleaseInput, ReleaseTrackInput,
    TrackArtist, TrackFilterArgs, TrackInput, performers,
};
use slunk::media::{Library, LinkOwner, Media};
use slunk::paging::ListArgs;
use slunk::repo::store::Store;
use slunk::repo::{connection, migrator};
use slunk::tagging::{
    TagFilterInput, TagInput, TagOwner, TagType, TagValuesInput, Tagging, tag_filters,
};
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
    store: Arc<Store>,
    media: Arc<Media>,
    catalog: Catalog,
    tagging: Tagging,
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
            catalog: Catalog::new(store.clone(), media.clone()),
            tagging: Tagging::new(store.clone()),
            media,
            store,
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

    /// One of the committed audio fixtures, at a path in the library.
    fn audio(&self, path: &str, fixture: &str) {
        self.put(path, &fs::read(fixtures().join(fixture)).unwrap());
    }

    /// The registered file's id, registering it.
    fn register(&self, path: &str) -> i64 {
        self.media.register_files(&[path.into()]).unwrap()[0].id
    }

    fn sql(&self, sql: &str) {
        self.store.conn().execute_batch(sql).unwrap();
    }

    fn number(&self, sql: &str) -> i64 {
        self.store
            .conn()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    }

    fn artist(&self, name: &str, names: &[&str]) -> i64 {
        self.catalog
            .create_artist(&ArtistInput {
                name: name.into(),
                alternate_names: Some(
                    names
                        .iter()
                        .map(|&name| slunk::catalog::AlternateNameInput {
                            name: name.into(),
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..ArtistInput::default()
            })
            .unwrap()
            .id
    }

    fn release(&self, title: &str, artists: &[i64]) -> i64 {
        self.catalog
            .create_release(&ReleaseInput {
                title: title.into(),
                artists: artists.iter().map(|&id| credited(id, None)).collect(),
                ..ReleaseInput::default()
            })
            .unwrap()
            .id
    }

    /// The release's tracks, as "disc-number title (duration)".
    fn tracks(&self, release: i64) -> Vec<String> {
        self.catalog
            .tracks_of_releases(&[release])
            .unwrap()
            .remove(&release)
            .unwrap_or_default()
            .into_iter()
            .map(|track| {
                format!(
                    "{}-{} {} ({})",
                    track.cd_number,
                    track.track_number,
                    track.title,
                    track.duration_ms.map_or("?".into(), |ms| ms.to_string())
                )
            })
            .collect()
    }

    fn track_ids(&self, release: i64) -> Vec<i64> {
        self.catalog
            .tracks_of_releases(&[release])
            .unwrap()
            .remove(&release)
            .unwrap_or_default()
            .into_iter()
            .map(|track| track.id)
            .collect()
    }

    /// The release's disc titles, as "disc: title [alternate titles]".
    fn disc_titles(&self, release: i64) -> Vec<String> {
        self.catalog
            .disc_titles_of(&[release])
            .unwrap()
            .remove(&release)
            .unwrap_or_default()
            .into_iter()
            .map(|disc| {
                let alternates: Vec<String> =
                    disc.alternate_titles.into_iter().map(|t| t.title).collect();
                format!("{}: {} {alternates:?}", disc.cd_number, disc.title)
            })
            .collect()
    }

    /// A track's artists, as each is credited, features marked.
    fn artists(&self, track: i64) -> Vec<String> {
        self.catalog
            .track_artists_of(&[track])
            .unwrap()
            .remove(&track)
            .unwrap_or_default()
            .into_iter()
            .map(|artist| {
                let shown = self.shown(&ArtistLink {
                    artist_id: artist.artist_id,
                    credited_name_id: artist.credited_name_id,
                });
                if artist.featured {
                    format!("feat. {shown}")
                } else {
                    shown
                }
            })
            .collect()
    }

    fn shown(&self, link: &ArtistLink) -> String {
        match link.credited_name_id {
            None => {
                self.catalog
                    .get_artist(link.artist_id)
                    .unwrap()
                    .unwrap()
                    .name
            }
            Some(id) => self.catalog.artist_names_by_ids(&[id]).unwrap()[0]
                .name
                .clone(),
        }
    }

    fn list(&self, args: ListArgs, filters: TrackFilterArgs) -> Vec<String> {
        self.catalog
            .list_tracks(&args, &filters)
            .unwrap()
            .items
            .into_iter()
            .map(|track| track.title)
            .collect()
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio")
}

/// untagged.flac with these Vorbis comments, in a block after STREAMINFO,
/// as `fixtures/audio/generate.ts` writes them.
fn tagged_flac(comments: &[(&str, &str)]) -> Vec<u8> {
    let file = fs::read(fixtures().join("untagged.flac")).unwrap();
    assert_eq!(&file[..4], b"fLaC");
    let mut blocks: Vec<(u8, Vec<u8>)> = Vec::new();
    let mut offset = 4;
    loop {
        let last = file[offset] & 0x80 != 0;
        let length =
            u32::from_be_bytes([0, file[offset + 1], file[offset + 2], file[offset + 3]]) as usize;
        blocks.push((
            file[offset] & 0x7f,
            file[offset + 4..offset + 4 + length].to_vec(),
        ));
        offset += 4 + length;
        if last {
            break;
        }
    }
    let vendor = b"liquidslunk tests";
    let mut body = (vendor.len() as u32).to_le_bytes().to_vec();
    body.extend(vendor);
    body.extend((comments.len() as u32).to_le_bytes());
    for (key, value) in comments {
        let comment = format!("{key}={value}");
        body.extend((comment.len() as u32).to_le_bytes());
        body.extend(comment.as_bytes());
    }
    blocks.retain(|(kind, _)| *kind != 4);
    blocks.insert(1, (4, body));
    let mut out = b"fLaC".to_vec();
    let count = blocks.len();
    for (i, (kind, body)) in blocks.into_iter().enumerate() {
        out.push(if i == count - 1 { 0x80 } else { 0 } | kind);
        out.extend(&(body.len() as u32).to_be_bytes()[1..]);
        out.extend(body);
    }
    out.extend(&file[offset..]);
    out
}

fn credited(artist_id: i64, credited_as: Option<&str>) -> CreditedArtistInput {
    CreditedArtistInput {
        artist_id,
        credited_as: credited_as.map(Into::into),
    }
}

/// A new track's row, from a path.
fn row(path: &str, cd: i64, number: i64, title: &str) -> ReleaseTrackInput {
    ReleaseTrackInput {
        file: FileRef {
            file_id: None,
            path: Some(path.into()),
        },
        cd_number: cd,
        track_number: number,
        title: title.into(),
        ..ReleaseTrackInput::default()
    }
}

/// A row for a track the release has.
fn kept(id: i64, cd: i64, number: i64, title: &str) -> ReleaseTrackInput {
    ReleaseTrackInput {
        id: Some(id),
        cd_number: cd,
        track_number: number,
        title: title.into(),
        ..ReleaseTrackInput::default()
    }
}

fn disc(cd: i64, title: &str, alternates: &[&str]) -> DiscTitleInput {
    DiscTitleInput {
        cd_number: cd,
        title: title.into(),
        language: None,
        alternate_titles: alternates
            .iter()
            .map(|&title| AlternateTitleInput {
                title: title.into(),
                ..AlternateTitleInput::default()
            })
            .collect(),
    }
}

fn track_input(release: i64, file: i64, number: i64, title: &str) -> TrackInput {
    TrackInput {
        release_id: release,
        file_id: file,
        cd_number: 1,
        track_number: number,
        title: title.into(),
        ..TrackInput::default()
    }
}

fn invalid(result: slunk::Result<impl std::fmt::Debug>) -> String {
    match result {
        Err(Error::Invalid(message)) => message,
        other => panic!("expected an invalid error, got {other:?}"),
    }
}

fn not_found(result: slunk::Result<impl std::fmt::Debug>) -> String {
    match result {
        Err(Error::NotFound(message)) => message,
        other => panic!("expected a not found error, got {other:?}"),
    }
}

/// A constraint's or trigger's message, as SQLite gives it.
fn refused(result: slunk::Result<impl std::fmt::Debug>) -> String {
    match result {
        Err(Error::Sqlite(error)) => error.to_string(),
        other => panic!("expected SQLite to refuse it, got {other:?}"),
    }
}

#[test]
fn a_release_s_tracks_are_saved_as_a_whole_list() {
    let fixture = Fixture::new();
    fixture.audio("music/A/1 One.flac", "untagged.flac");
    fixture.audio("music/A/2 Two.flac", "flac.flac");
    fixture.audio("music/A/3 Three.flac", "untagged.flac");
    let release = fixture.release("A", &[]);
    let other = fixture.release("B", &[]);
    let registered = fixture.register("music/A/3 Three.flac");

    // New tracks from paths, registered in the same save, and from a
    // registered file, each with its duration
    fixture
        .catalog
        .set_release_tracks(
            release,
            &[
                row("music/A/1 One.flac", 1, 1, " One "),
                row("./music/A//2 Two.flac", 1, 2, "Two"),
                ReleaseTrackInput {
                    file: FileRef {
                        file_id: Some(registered),
                        path: None,
                    },
                    ..row("", 2, 1, "Three")
                },
            ],
            None,
        )
        .unwrap();
    assert_eq!(
        fixture.tracks(release),
        ["1-1 One (1000)", "1-2 Two (1001)", "2-1 Three (1000)"]
    );
    let [one, two, three] = fixture.track_ids(release)[..] else {
        panic!("three tracks")
    };

    // Swapping numbers doesn't trip the unique (release, disc, number) index,
    // and a track left out is deleted, but its file stays registered
    fixture
        .catalog
        .set_release_tracks(
            release,
            &[kept(two, 1, 1, "Two"), kept(one, 1, 2, "One")],
            None,
        )
        .unwrap();
    assert_eq!(
        fixture.tracks(release),
        ["1-1 Two (1001)", "1-2 One (1000)"]
    );
    assert_eq!(
        fixture.number(&format!(
            "select count(*) from files where id = {registered}"
        )),
        1
    );
    assert!(fixture.catalog.get_track(three).unwrap().is_none());

    // A track of another release, or one that isn't there, is refused, and
    // nothing is saved
    fixture
        .catalog
        .set_release_tracks(other, &[row("music/A/3 Three.flac", 1, 1, "Three")], None)
        .unwrap();
    let theirs = fixture.track_ids(other)[0];
    assert_eq!(
        invalid(
            fixture
                .catalog
                .set_release_tracks(release, &[kept(theirs, 1, 3, "Three")], None)
        ),
        format!("Track {theirs} isn't on this release")
    );
    assert_eq!(
        fixture.tracks(release),
        ["1-1 Two (1001)", "1-2 One (1000)"]
    );
    assert_eq!(
        not_found(fixture.catalog.set_release_tracks(999, &[], None)),
        "There's no release 999"
    );

    // A file that's another track's audio, one that isn't music, a missing
    // path, and a row with no file at all
    assert_eq!(
        refused(fixture.catalog.set_release_tracks(
            release,
            &[
                kept(two, 1, 1, "Two"),
                ReleaseTrackInput {
                    file: FileRef {
                        file_id: Some(registered),
                        path: None,
                    },
                    ..row("", 1, 3, "Again")
                },
            ],
            None
        )),
        "UNIQUE constraint failed: tracks.file_id"
    );
    fixture.put("music/A/notes.txt", b"notes");
    assert_eq!(
        refused(fixture.catalog.set_release_tracks(
            release,
            &[row("music/A/notes.txt", 1, 3, "Notes")],
            None
        )),
        "a track's file must be music"
    );
    // The file the save would have registered isn't left registered
    assert_eq!(
        fixture.number("select count(*) from files where path = 'music/A/notes.txt'"),
        0
    );
    assert!(
        invalid(fixture.catalog.set_release_tracks(
            release,
            &[row("music/A/gone.flac", 1, 3, "Gone")],
            None
        ))
        .contains("music/A/gone.flac")
    );
    assert_eq!(
        invalid(
            fixture
                .catalog
                .set_release_tracks(release, &[row("", 1, 3, "Nothing")], None)
        ),
        "A file needs an id or a path"
    );
    assert_eq!(
        refused(fixture.catalog.set_release_tracks(
            release,
            &[kept(two, 1, 1, "Two"), kept(one, 1, 1, "One")],
            None
        )),
        "UNIQUE constraint failed: tracks.release_id, tracks.cd_number, tracks.track_number"
    );
    assert_eq!(
        fixture.tracks(release),
        ["1-1 Two (1001)", "1-2 One (1000)"]
    );
}

#[test]
fn a_track_s_title_language_and_alternate_titles_stay_unless_given() {
    let fixture = Fixture::new();
    fixture.audio("music/1.flac", "untagged.flac");
    let release = fixture.release("A", &[]);
    fixture
        .catalog
        .set_release_tracks(
            release,
            &[ReleaseTrackInput {
                title_language: Some(Some("JA".into())),
                alternate_titles: Some(vec![
                    AlternateTitleInput {
                        title: "Early Summer".into(),
                        language: Some("en".into()),
                        printed: Some(false),
                    },
                    AlternateTitleInput {
                        title: "初夏 ".into(),
                        ..AlternateTitleInput::default()
                    },
                ]),
                ..row("music/1.flac", 1, 1, "Shoka")
            }],
            None,
        )
        .unwrap();
    let id = fixture.track_ids(release)[0];
    let titles = |fixture: &Fixture| -> Vec<String> {
        fixture
            .catalog
            .track_titles_of(&[id])
            .unwrap()
            .remove(&id)
            .unwrap_or_default()
            .into_iter()
            .map(|t| format!("{} {:?} {}", t.title, t.language, t.printed))
            .collect()
    };
    assert_eq!(
        titles(&fixture),
        ["Early Summer Some(\"en\") false", "初夏 None true"]
    );

    // Left out, they stay, except one that's now the main title
    fixture
        .catalog
        .set_release_tracks(release, &[kept(id, 1, 1, "初夏")], None)
        .unwrap();
    let track = fixture.catalog.get_track(id).unwrap().unwrap();
    assert_eq!(
        (track.title.as_str(), track.title_language.as_deref()),
        ("初夏", Some("ja"))
    );
    assert_eq!(titles(&fixture), ["Early Summer Some(\"en\") false"]);

    // Given as null, the language is the release's
    fixture
        .catalog
        .set_release_tracks(
            release,
            &[ReleaseTrackInput {
                title_language: Some(None),
                ..kept(id, 1, 1, "初夏")
            }],
            None,
        )
        .unwrap();
    assert_eq!(
        fixture
            .catalog
            .get_track(id)
            .unwrap()
            .unwrap()
            .title_language,
        None
    );
    assert_eq!(
        invalid(fixture.catalog.set_release_tracks(
            release,
            &[ReleaseTrackInput {
                alternate_titles: Some(vec![AlternateTitleInput {
                    title: "初夏".into(),
                    ..AlternateTitleInput::default()
                }]),
                ..kept(id, 1, 4, "初夏")
            }],
            None
        )),
        "\"初夏\" is already the main title of track 4"
    );
}

#[test]
fn disc_titles_are_saved_after_the_tracks() {
    let fixture = Fixture::new();
    fixture.audio("music/1.flac", "untagged.flac");
    fixture.audio("music/2.flac", "untagged.flac");
    let release = fixture.release("A", &[]);
    fixture
        .catalog
        .set_release_tracks(
            release,
            &[
                row("music/1.flac", 1, 1, "One"),
                row("music/2.flac", 2, 1, "Two"),
            ],
            Some(&[
                disc(1, " Side A ", &["A面", " "]),
                // A blank title is left out
                disc(2, " ", &[]),
            ]),
        )
        .unwrap();
    assert_eq!(fixture.disc_titles(release), ["1: Side A [\"A面\"]"]);
    let [one, two] = fixture.track_ids(release)[..] else {
        panic!("two tracks")
    };

    // Left out, they stay as they are
    fixture
        .catalog
        .set_release_tracks(
            release,
            &[kept(one, 1, 1, "One"), kept(two, 2, 1, "Two")],
            None,
        )
        .unwrap();
    assert_eq!(fixture.disc_titles(release), ["1: Side A [\"A面\"]"]);

    assert_eq!(
        invalid(fixture.catalog.set_release_tracks(
            release,
            &[],
            Some(&[disc(1, "A", &[]), disc(1, "B", &[])])
        )),
        "Disc 1 has two titles"
    );
    assert_eq!(
        invalid(fixture.catalog.set_release_tracks(
            release,
            &[],
            Some(&[disc(2, "", &["Side B"])])
        )),
        "Disc 2 has alternate titles but no title"
    );
    assert_eq!(
        invalid(
            fixture
                .catalog
                .set_release_tracks(release, &[], Some(&[disc(2, "B", &["B"])]))
        ),
        "\"B\" is already the main title of disc 2"
    );
    // A title needs a track on its disc
    assert_eq!(
        refused(fixture.catalog.set_release_tracks(
            release,
            &[kept(one, 1, 1, "One"), kept(two, 2, 1, "Two")],
            Some(&[disc(3, "Bonus", &[])])
        )),
        "a disc title needs a track on its disc"
    );

    // A disc left without tracks loses its title, even when titles are left
    // out
    fixture
        .catalog
        .set_release_tracks(release, &[kept(two, 2, 1, "Two")], None)
        .unwrap();
    assert!(fixture.disc_titles(release).is_empty());
    assert_eq!(
        fixture.number("select count(*) from release_disc_alternate_titles"),
        0
    );
}

#[test]
fn track_artists_are_credited_and_features_kept_apart() {
    let fixture = Fixture::new();
    fixture.audio("music/1.flac", "untagged.flac");
    let alika = fixture.artist("Alika", &["ALIKA"]);
    let nublu = fixture.artist("Nublu", &[]);
    let ito = fixture.artist("Masataka Ito", &[]);
    let release = fixture.release("A", &[alika]);
    fixture
        .catalog
        .set_release_tracks(
            release,
            &[ReleaseTrackInput {
                artists: vec![
                    credited(alika, Some("ALIKA")),
                    credited(ito, Some("いとう・まさたか")),
                    credited(alika, Some("Alika")),
                ],
                features: vec![credited(nublu, Some(" "))],
                ..row("music/1.flac", 1, 1, "One")
            }],
            None,
        )
        .unwrap();
    let id = fixture.track_ids(release)[0];
    assert_eq!(
        fixture.artists(id),
        ["ALIKA", "いとう・まさたか", "feat. Nublu"]
    );
    // A name given that the artist doesn't have yet is added, in the
    // language its script gives away
    let added = fixture.catalog.artist_names_of(&[ito]).unwrap()[&ito][0].clone();
    assert_eq!(
        (added.name.as_str(), added.language.as_deref()),
        ("いとう・まさたか", Some("ja"))
    );

    assert_eq!(
        invalid(fixture.catalog.set_release_tracks(
            release,
            &[ReleaseTrackInput {
                artists: vec![credited(nublu, None)],
                features: vec![credited(nublu, None)],
                ..kept(id, 1, 1, "One")
            }],
            None
        )),
        "An artist can't both perform a track and be featured on it"
    );
    assert_eq!(
        not_found(fixture.catalog.set_release_tracks(
            release,
            &[ReleaseTrackInput {
                artists: vec![credited(999, None)],
                ..kept(id, 1, 1, "One")
            }],
            None
        )),
        "There's no artist 999"
    );
}

#[test]
fn performers_fall_back_to_the_release_s_artists_on_a_regular_release() {
    let fixture = Fixture::new();
    let release = fixture
        .catalog
        .get_release(fixture.release("A", &[]))
        .unwrap()
        .unwrap();
    let link = |artist_id: i64, credited_name_id: Option<i64>| ArtistLink {
        artist_id,
        credited_name_id,
    };
    let own = |artist_id: i64, featured: bool| TrackArtist {
        artist_id,
        credited_name_id: None,
        featured,
    };
    let release_artists = [link(1, Some(7))];
    // Its own artists, without features
    assert_eq!(
        performers(&release, &[own(2, false), own(3, true)], &release_artists),
        [link(2, None)]
    );
    // None of its own: the release's, as credited there
    assert_eq!(
        performers(&release, &[own(3, true)], &release_artists),
        release_artists
    );
    // On a various-artists release, nobody
    let various = slunk::catalog::Release {
        various_artists: true,
        ..release
    };
    assert!(performers(&various, &[own(3, true)], &release_artists).is_empty());
}

#[test]
fn a_track_is_saved_with_its_credits_files_and_tags() {
    let fixture = Fixture::new();
    fixture.audio("music/1.flac", "untagged.flac");
    fixture.audio("music/2.flac", "flac.flac");
    fixture.put("music/cover.jpg", b"jpeg");
    fixture.put("music/notes.txt", b"notes");
    let one = fixture.register("music/1.flac");
    let two = fixture.register("music/2.flac");
    let notes = fixture.register("music/notes.txt");
    let kosma = fixture.artist("Joseph Kosma", &[]);
    let release = fixture.release("A", &[]);
    let genre = fixture
        .tagging
        .create_tag(
            TagType::Text,
            &TagInput {
                name: "Genre".into(),
                multiple: true,
            },
        )
        .unwrap()
        .id;
    let tags = |values: &[&str]| {
        Some(vec![TagValuesInput {
            kind: TagType::Text,
            tag_id: genre,
            values: values.iter().map(|&value| value.into()).collect(),
        }])
    };

    let track = fixture
        .catalog
        .create_track(&TrackInput {
            title_language: Some(Some("en".into())),
            credits: vec![CreditInput {
                role: " Bass ".into(),
                artists: vec![credited(kosma, Some("J. Kosma"))],
            }],
            files: vec![FileLinkInput {
                file: FileRef {
                    file_id: None,
                    path: Some("music/cover.jpg".into()),
                },
                note: Some(" Booklet ".into()),
            }],
            tags: tags(&["Jazz", "jazz", "Bebop"]),
            ..track_input(release, one, 1, " Autumn Leaves ")
        })
        .unwrap();
    assert_eq!(
        (
            track.title.as_str(),
            track.title_language.as_deref(),
            track.duration_ms
        ),
        ("Autumn Leaves", Some("en"), Some(1000))
    );
    let credits = fixture
        .catalog
        .credits_of(CreditOwner::Track, &[track.id])
        .unwrap()
        .remove(&track.id)
        .unwrap();
    assert_eq!(credits[0].role, "Bass");
    assert_eq!(fixture.shown(&credits[0].artists[0]), "J. Kosma");
    let links = fixture
        .media
        .file_links_of(LinkOwner::Track, &[track.id])
        .unwrap()
        .remove(&track.id)
        .unwrap();
    assert_eq!(links[0].note.as_deref(), Some("Booklet"));
    let values = |fixture: &Fixture| -> Vec<String> {
        fixture
            .tagging
            .tags_of(TagOwner::Track, &[track.id])
            .unwrap()
            .remove(&track.id)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|tag| tag.values)
            .map(|value| slunk::tagging::format_value(&value))
            .collect()
    };
    assert_eq!(values(&fixture), ["Jazz", "Bebop"]);

    // The same file keeps the duration the track has, and tags left out
    // stay as they are
    fixture.sql(&format!(
        "update tracks set duration_ms = 5 where id = {}",
        track.id
    ));
    let updated = fixture
        .catalog
        .update_track(track.id, &track_input(release, one, 2, "Autumn Leaves"))
        .unwrap();
    assert_eq!(
        (updated.duration_ms, updated.title_language.as_deref()),
        (Some(5), Some("en"))
    );
    assert_eq!(values(&fixture), ["Jazz", "Bebop"]);
    // Its credits and file links are replaced
    assert!(
        fixture
            .catalog
            .credits_of(CreditOwner::Track, &[track.id])
            .unwrap()
            .is_empty()
    );
    // A new file is read again, and tags given replace the old ones
    let updated = fixture
        .catalog
        .update_track(
            track.id,
            &TrackInput {
                title_language: Some(None),
                tags: tags(&[]),
                ..track_input(release, two, 2, "Autumn Leaves")
            },
        )
        .unwrap();
    assert_eq!(
        (updated.duration_ms, updated.title_language),
        (Some(1001), None)
    );
    assert!(values(&fixture).is_empty());

    // The audio has to be a registered music file
    assert_eq!(
        not_found(
            fixture
                .catalog
                .create_track(&track_input(release, 999, 3, "X"))
        ),
        "There's no file 999"
    );
    assert_eq!(
        invalid(
            fixture
                .catalog
                .create_track(&track_input(release, notes, 3, "X"))
        ),
        "music/notes.txt isn't a music file"
    );
    assert_eq!(
        refused(
            fixture
                .catalog
                .create_track(&track_input(release, two, 3, "X"))
        ),
        "UNIQUE constraint failed: tracks.file_id"
    );
    assert_eq!(
        not_found(
            fixture
                .catalog
                .update_track(999, &track_input(release, two, 3, "X"))
        ),
        "There's no track 999"
    );

    // Deleting keeps its file registered, and an unknown id deletes nothing
    fixture.catalog.delete_track(track.id).unwrap();
    fixture.catalog.delete_track(999).unwrap();
    assert!(fixture.catalog.get_track(track.id).unwrap().is_none());
    assert_eq!(fixture.number("select count(*) from files"), 4);
}

#[test]
fn the_track_list_searches_titles_and_filters_by_release_and_tags() {
    let fixture = Fixture::new();
    for name in ["1", "2", "3"] {
        fixture.audio(&format!("music/{name}.flac"), "untagged.flac");
    }
    let a = fixture.release("A", &[]);
    let b = fixture.release("B", &[]);
    fixture
        .catalog
        .set_release_tracks(
            a,
            &[
                ReleaseTrackInput {
                    alternate_titles: Some(vec![AlternateTitleInput {
                        title: "初夏".into(),
                        ..AlternateTitleInput::default()
                    }]),
                    ..row("music/1.flac", 1, 1, "early summer")
                },
                row("music/2.flac", 1, 2, "Mellow Dream"),
            ],
            None,
        )
        .unwrap();
    fixture
        .catalog
        .set_release_tracks(b, &[row("music/3.flac", 1, 1, "Baroque")], None)
        .unwrap();
    let sorted = |sort: &str| ListArgs {
        sort: Some(sort.into()),
        ..ListArgs::default()
    };
    let none = TrackFilterArgs::default;
    assert_eq!(
        fixture.list(ListArgs::default(), none()),
        ["Baroque", "Mellow Dream", "early summer"]
    );
    assert_eq!(
        fixture.list(sorted("title"), none()),
        ["Baroque", "early summer", "Mellow Dream"]
    );
    assert_eq!(
        fixture.list(
            ListArgs {
                q: Some("初".into()),
                ..ListArgs::default()
            },
            none()
        ),
        ["early summer"]
    );
    assert_eq!(
        fixture.list(
            sorted("title"),
            TrackFilterArgs {
                release: Some(a),
                ..none()
            }
        ),
        ["early summer", "Mellow Dream"]
    );
    let score = fixture
        .tagging
        .create_tag(
            TagType::Number,
            &TagInput {
                name: "Score".into(),
                multiple: false,
            },
        )
        .unwrap()
        .id;
    let baroque = fixture.track_ids(b)[0];
    fixture
        .tagging
        .set_tags(
            TagOwner::Track,
            baroque,
            &[TagValuesInput {
                kind: TagType::Number,
                tag_id: score,
                values: vec!["8".into()],
            }],
        )
        .unwrap();
    let filters = tag_filters(&[TagFilterInput {
        kind: TagType::Number,
        tag_id: score,
        equals: None,
        contains: None,
        min: Some("8".into()),
        max: None,
    }])
    .unwrap();
    assert_eq!(
        fixture.list(
            ListArgs::default(),
            TrackFilterArgs {
                tags: filters,
                ..none()
            }
        ),
        ["Baroque"]
    );
    assert_eq!(
        invalid(
            fixture
                .catalog
                .list_tracks(&sorted("duration"), &TrackFilterArgs::default())
        ),
        "Tracks can't be sorted by duration"
    );
}

#[test]
fn a_folder_suggests_a_release_from_its_tags_or_its_name() {
    let fixture = Fixture::new();
    let fukui = fixture.artist("Ryo Fukui", &["福居良"]);
    fixture.release("Scenery", &[fukui]);
    let tagged = |title: &str, date: &str| {
        tagged_flac(&[
            ("TITLE", title),
            ("ALBUM", "Scenery = シーナリィ"),
            ("ALBUMARTIST", "Ryo Fukui = 福居良 = Ryo"),
            ("ARTIST", "R. Fukui"),
            ("DATE", date),
        ])
    };
    fixture.put(
        "music/Scenery/1.flac",
        &tagged("It Could Happen to You", "1976"),
    );
    fixture.put("music/Scenery/2.flac", &tagged("Early Summer", "1976"));
    fixture.put(
        "music/Scenery/3.flac",
        &tagged("Mellow Dream", "1977-02-30"),
    );
    fixture.put("music/Scenery/cover.jpg", b"jpeg");
    fixture.put("music/Scenery/rip.log", b"log");

    let suggestion = fixture.catalog.suggest_release("music/Scenery/").unwrap();
    // The title is kept whole, and finds the release with one of its parts
    assert_eq!(suggestion.title, "Scenery = シーナリィ");
    assert_eq!(suggestion.released.as_deref(), Some("1976"));
    assert_eq!(
        suggestion
            .same_title
            .iter()
            .map(|release| release.title.as_str())
            .collect::<Vec<_>>(),
        ["Scenery"]
    );
    assert_eq!(
        (suggestion.music, suggestion.images, suggestion.others),
        (3, 1, 1)
    );
    // The album artist tag, matched by its first part, with its other parts
    // the artist doesn't have yet to offer
    let [artist] = &suggestion.artists[..] else {
        panic!("one artist")
    };
    assert_eq!(artist.name, "Ryo Fukui");
    assert_eq!(artist.artist.as_ref().map(|a| a.id), Some(fukui));
    assert_eq!(artist.credited_as, None);
    assert_eq!(artist.other_names, ["Ryo"]);

    // Without tags, the folder's name, and a date that isn't one is left out
    fixture.audio("music/Mellow Dream/1.flac", "untagged.flac");
    let suggestion = fixture
        .catalog
        .suggest_release("music/Mellow Dream")
        .unwrap();
    assert_eq!(suggestion.title, "Mellow Dream");
    assert!(suggestion.artists.is_empty() && suggestion.same_title.is_empty());
    fixture.put(
        "music/Bad/1.flac",
        &tagged_flac(&[("ALBUM", "Bad"), ("DATE", "2013-02-30")]),
    );
    assert_eq!(
        fixture
            .catalog
            .suggest_release("music/Bad")
            .unwrap()
            .released,
        None
    );
    assert!(invalid(fixture.catalog.suggest_release("music/nowhere")).contains("music/nowhere"));
}

#[test]
fn a_folder_suggests_a_row_for_each_audio_file() {
    let fixture = Fixture::new();
    fixture.put(
        "music/A/CD1/1.flac",
        &tagged_flac(&[
            ("TITLE", "Tagged"),
            ("TRACKNUMBER", "7"),
            ("DISCNUMBER", "1"),
            ("DISCSUBTITLE", "Live at Slowboat"),
            ("ARTIST", "Alika"),
            ("ARTIST", "Nublu"),
        ]),
    );
    fixture.audio("music/A/CD2/03 - From the Name.flac", "untagged.flac");
    fixture.audio("music/A/CD2/Unnumbered.flac", "untagged.flac");
    fixture.audio("music/A/CD2/Another.flac", "untagged.flac");
    fixture.put("music/A/cover.jpg", b"jpeg");
    let alika = fixture.artist("Alika", &[]);
    let release = fixture.release("Elsewhere", &[]);
    fixture
        .catalog
        .set_release_tracks(
            release,
            &[row("music/A/CD2/Another.flac", 1, 1, "Another")],
            None,
        )
        .unwrap();

    let rows = fixture.catalog.suggest_tracks("music/A").unwrap();
    let shown: Vec<String> = rows
        .iter()
        .map(|row| {
            format!(
                "{} {}-{} {} {:?} {:?}",
                row.path,
                row.cd_number,
                row.track_number,
                row.title,
                row.disc_title,
                row.duration_ms
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            "music/A/CD1/1.flac 1-7 Tagged Some(\"Live at Slowboat\") Some(1000)",
            "music/A/CD2/03 - From the Name.flac 2-3 From the Name None Some(1000)",
            // Numbered by its place on its disc, after the others
            "music/A/CD2/Another.flac 2-2 Another None Some(1000)",
            "music/A/CD2/Unnumbered.flac 2-3 Unnumbered None Some(1000)",
        ]
    );
    // A file already a track's audio has its registration and its track
    assert_eq!(
        rows[2].track.as_ref().map(|track| track.release_id),
        Some(release)
    );
    assert!(rows[2].file.is_some() && rows[0].file.is_none());
    // Each row has its own artist tags, matched
    let artists: Vec<(String, Option<i64>)> = rows[0]
        .artists
        .iter()
        .map(|a| (a.name.clone(), a.artist.as_ref().map(|a| a.id)))
        .collect();
    assert_eq!(
        artists,
        [("Alika".into(), Some(alika)), ("Nublu".into(), None)]
    );
    assert!(rows[1].artists.is_empty());
}

#[test]
fn artist_tags_are_matched_by_any_name_in_three_steps() {
    let fixture = Fixture::new();
    let alika = fixture.artist("Alika", &["ALIKA"]);
    let fukui = fixture.artist("Ryo Fukui", &["R. Fukui", "福居良"]);
    let trio = fixture.artist("Ryo Fukui Trio", &[]);
    fixture.artist("Twin", &[]);
    fixture.artist("TWIN", &[]);
    let artists: Vec<String> = [
        "Alika",
        "ALIKA",
        "alika",
        "R. Fukui",
        "r. fukui = 福居良 = Fukui Ryo",
        "ryo fukui trio",
        "Twin",
        "twin",
        "Nobody",
        " = ",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let mut comments: Vec<(&str, &str)> = artists.iter().map(|a| ("ARTIST", a.as_str())).collect();
    comments.push(("TITLE", "Tagged"));
    fixture.put("music/1.flac", &tagged_flac(&comments));

    let rows = fixture.catalog.suggest_tracks("music").unwrap();
    let suggested: Vec<String> = rows[0]
        .artists
        .iter()
        .map(|a| {
            format!(
                "{} → {:?} as {:?}, {} matches, offers {:?}",
                a.name,
                a.artist.as_ref().map(|a| a.id),
                a.credited_as,
                a.matches,
                a.other_names
            )
        })
        .collect();
    assert_eq!(
        suggested,
        [
            // The main name exactly
            format!("Alika → Some({alika}) as None, 1 matches, offers []"),
            // An alternate name exactly, credited with it
            format!("ALIKA → Some({alika}) as Some(\"ALIKA\"), 1 matches, offers []"),
            // Either ignoring case, credited as tagged
            format!("alika → Some({alika}) as Some(\"alika\"), 1 matches, offers []"),
            format!("R. Fukui → Some({fukui}) as Some(\"R. Fukui\"), 1 matches, offers []"),
            // Matched by its first part, offering the parts it doesn't have
            format!(
                "r. fukui → Some({fukui}) as Some(\"r. fukui\"), 1 matches, offers [\"Fukui Ryo\"]"
            ),
            format!(
                "ryo fukui trio → Some({trio}) as Some(\"ryo fukui trio\"), 1 matches, offers []"
            ),
            // Twin matches only "Twin" exactly; "twin" matches two artists
            "Twin → Some(4) as None, 1 matches, offers []".to_string(),
            "twin → None as None, 2 matches, offers []".to_string(),
            "Nobody → None as None, 0 matches, offers []".to_string(),
            "= → None as None, 0 matches, offers []".to_string(),
        ]
    );
}

#[test]
fn an_alternate_name_is_added_once() {
    let fixture = Fixture::new();
    let fukui = fixture.artist("Ryo Fukui", &["R. Fukui"]);
    fixture
        .catalog
        .add_artist_name(fukui, " 福居良 ", Some("JA"))
        .unwrap();
    let names: Vec<(String, Option<String>)> = fixture
        .catalog
        .artist_names_of(&[fukui])
        .unwrap()
        .remove(&fukui)
        .unwrap()
        .into_iter()
        .map(|name| (name.name, name.language))
        .collect();
    assert_eq!(
        names,
        [
            ("R. Fukui".into(), None),
            ("福居良".into(), Some("ja".into()))
        ]
    );
    assert_eq!(
        invalid(fixture.catalog.add_artist_name(fukui, "福居良", None)),
        "Ryo Fukui already has the name \"福居良\""
    );
    assert_eq!(
        invalid(fixture.catalog.add_artist_name(fukui, "Ryo Fukui", None)),
        "\"Ryo Fukui\" is already the artist's main name"
    );
    assert_eq!(
        invalid(fixture.catalog.add_artist_name(fukui, " ", None)),
        "The name is blank"
    );
    assert_eq!(
        not_found(fixture.catalog.add_artist_name(999, "X", None)),
        "There's no artist 999"
    );
    assert!(invalid(fixture.catalog.add_artist_name(fukui, "X", Some("und"))).contains("blank"));
}
