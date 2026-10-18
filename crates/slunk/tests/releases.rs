//! The catalog context's releases, on a temporary library: their rules, as
//! docs/development/backend.md lists them ("Releases").

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use slunk::Error;
use slunk::catalog::{
    AlternateTitle, AlternateTitleInput, ArtistCredit, ArtistInput, ArtistLink, Catalog, Credit,
    CreditInput, CreditOwner, CreditedArtistInput, FileLinkInput, FileRef, LabelInput,
    ReleaseFilterArgs, ReleaseInput, ReleaseLink, TrackCreditsInput, released,
};
use slunk::media::{Library, Media};
use slunk::paging::ListArgs;
use slunk::repo::store::Store;
use slunk::repo::{connection, migrator};
use slunk::tagging::{TagFilterInput, TagInput, TagType, TagValuesInput, Tagging, tag_filters};
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
    store: Arc<Store>,
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
            catalog: Catalog::new(store.clone(), media),
            tagging: Tagging::new(store.clone()),
            store,
        }
    }

    fn library(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap().join("library")
    }

    fn write(&self, path: &str, content: &str) {
        let file = self.library().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content).unwrap();
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

    fn label(&self, name: &str) -> i64 {
        self.catalog
            .create_label(&LabelInput {
                name: name.into(),
                ..LabelInput::default()
            })
            .unwrap()
            .id
    }

    /// A format the migrations add, by name.
    fn format(&self, name: &str) -> i64 {
        self.number(&format!(
            "select id from release_formats where name = '{name}'"
        ))
    }

    fn release(&self, input: ReleaseInput) -> i64 {
        self.catalog.create_release(&input).unwrap().id
    }

    /// A track on a release, with a music file of its own, and its id.
    fn track(&self, release: i64, cd: i64, number: i64, title: &str) -> i64 {
        let conn = self.store.conn();
        let file: i64 = conn
            .query_row(
                "insert into files (path, type, size_bytes, mtime, sha256)
                 values ('music/' || ? || '.flac', 'music', 0, 0, printf('%064d', 0)) returning id",
                [format!("{release}-{cd}-{number}")],
                |row| row.get(0),
            )
            .unwrap();
        conn.query_row(
            "insert into tracks (title, cd_number, track_number, release_id, file_id, duration_ms)
             values (?, ?, ?, ?, ?, 1000) returning id",
            rusqlite::params![title, cd, number, release, file],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn names(&self, artist: i64) -> Vec<String> {
        self.catalog
            .artist_names_of(&[artist])
            .unwrap()
            .remove(&artist)
            .unwrap_or_default()
            .into_iter()
            .map(|name| format!("{} ({})", name.name, name.language.unwrap_or_default()))
            .collect()
    }

    /// The release's artists, as each is credited: its main name, or the
    /// name printed.
    fn credited(&self, release: i64) -> Vec<String> {
        let links = self
            .catalog
            .release_artists_of(&[release])
            .unwrap()
            .remove(&release)
            .unwrap_or_default();
        self.shown(&links)
    }

    fn shown(&self, links: &[ArtistLink]) -> Vec<String> {
        links
            .iter()
            .map(|link| match link.credited_name_id {
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
            })
            .collect()
    }

    fn titles(&self, release: i64) -> Vec<AlternateTitle> {
        self.catalog
            .release_titles_of(&[release])
            .unwrap()
            .remove(&release)
            .unwrap_or_default()
    }

    fn list(&self, args: ListArgs, filters: ReleaseFilterArgs) -> Vec<String> {
        self.catalog
            .list_releases(&args, &filters)
            .unwrap()
            .items
            .into_iter()
            .map(|release| release.title)
            .collect()
    }
}

fn credited(artist_id: i64, credited_as: Option<&str>) -> CreditedArtistInput {
    CreditedArtistInput {
        artist_id,
        credited_as: credited_as.map(Into::into),
    }
}

fn titled(title: &str) -> ReleaseInput {
    ReleaseInput {
        title: title.into(),
        ..ReleaseInput::default()
    }
}

fn title(title: &str, language: Option<&str>, printed: Option<bool>) -> AlternateTitleInput {
    AlternateTitleInput {
        title: title.into(),
        language: language.map(Into::into),
        printed,
    }
}

fn line(role: &str, artists: Vec<CreditedArtistInput>) -> CreditInput {
    CreditInput {
        role: role.into(),
        artists,
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

fn sorted(sort: &str) -> ListArgs {
    ListArgs {
        sort: Some(sort.into()),
        ..ListArgs::default()
    }
}

#[test]
fn details_are_cleaned_and_dates_keep_their_precision() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let release = catalog
        .create_release(&ReleaseInput {
            title: " Cafe\u{301} ".into(),
            title_language: Some(Some("JA-latn".into())),
            released: Some(" 2006-01 ".into()),
            catalog_number: Some(" B0019141-02 ".into()),
            various_artists: true,
            ..ReleaseInput::default()
        })
        .unwrap();
    assert_eq!(release.title, "Caf\u{e9}");
    assert_eq!(release.title_language.as_deref(), Some("ja-Latn"));
    assert_eq!(release.catalog_number.as_deref(), Some("B0019141-02"));
    assert!(release.various_artists);
    assert_eq!(released(&release).as_deref(), Some("2006-01"));
    assert_eq!(release.release_date, Some(1_136_073_600));

    // Left out, the language stays; null clears it; a blank date is none
    let kept = catalog
        .update_release(
            release.id,
            &ReleaseInput {
                title: "Cafe".into(),
                released: Some(" ".into()),
                ..ReleaseInput::default()
            },
        )
        .unwrap();
    assert_eq!(kept.title_language.as_deref(), Some("ja-Latn"));
    assert_eq!((kept.release_date, released(&kept)), (None, None));
    let cleared = catalog
        .update_release(
            release.id,
            &ReleaseInput {
                title: "Cafe".into(),
                title_language: Some(None),
                ..ReleaseInput::default()
            },
        )
        .unwrap();
    assert_eq!(cleared.title_language, None);

    assert_eq!(
        invalid(catalog.create_release(&ReleaseInput {
            title: "Bad".into(),
            released: Some(" 2006-02-30 ".into()),
            ..ReleaseInput::default()
        })),
        "There's no such date: 2006-02-30"
    );
    assert_eq!(
        invalid(catalog.create_release(&ReleaseInput {
            title: "Bad".into(),
            title_language: Some(Some("und".into())),
            released: Some("06".into()),
            ..ReleaseInput::default()
        })),
        "Leave the language blank when it isn't known"
    );
    assert_eq!(
        not_found(catalog.update_release(99, &titled("Nobody"))),
        "There's no release 99"
    );
    // Nothing was saved by the saves that failed
    assert_eq!(fixture.number("select count(*) from releases"), 1);
}

#[test]
fn artists_are_credited_with_the_names_printed() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let alika = fixture.artist("Alika", &["ALIKA"]);
    let ryo = fixture.artist("Ryo Fukui", &[]);
    let release = fixture.release(ReleaseInput {
        title: "virmalised".into(),
        artists: vec![
            credited(alika, Some("ALIKA")),
            credited(ryo, Some(" ふくい りょう ")),
            // An artist given twice is kept once, as first given
            credited(alika, None),
        ],
        ..ReleaseInput::default()
    });
    assert_eq!(fixture.credited(release), ["ALIKA", "ふくい りょう"]);
    // A new name is added to the artist's, in the language its script gives
    assert_eq!(fixture.names(ryo), ["ふくい りょう (ja)"]);

    // Blank, or the main name, is the main name
    catalog
        .update_release(
            release,
            &ReleaseInput {
                title: "virmalised".into(),
                artists: vec![
                    credited(alika, Some("  ")),
                    credited(ryo, Some("Ryo Fukui")),
                ],
                ..ReleaseInput::default()
            },
        )
        .unwrap();
    assert_eq!(fixture.credited(release), ["Alika", "Ryo Fukui"]);
    assert_eq!(
        not_found(catalog.update_release(
            release,
            &ReleaseInput {
                title: "virmalised".into(),
                artists: vec![credited(alika, None), credited(99, None)],
                ..ReleaseInput::default()
            }
        )),
        "There's no artist 99"
    );
}

#[test]
fn labels_keep_their_order_and_formats_go_by_id_once_each() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let (trio, nadja) = (fixture.label("Trio"), fixture.label("Nadja"));
    let (lp, album) = (fixture.format("LP"), fixture.format("Album"));
    let release = fixture.release(ReleaseInput {
        title: "Scenery".into(),
        label_ids: vec![nadja, trio, nadja],
        format_ids: vec![lp, album, lp],
        ..ReleaseInput::default()
    });
    let links = |link| {
        catalog
            .release_links_of(link, &[release])
            .unwrap()
            .remove(&release)
    };
    assert_eq!(links(ReleaseLink::Labels), Some(vec![nadja, trio]));
    // Formats and media have no position: they're listed by id
    assert!(album < lp);
    assert_eq!(links(ReleaseLink::Formats), Some(vec![album, lp]));
    assert_eq!(links(ReleaseLink::Media), None);
}

#[test]
fn alternate_titles_are_saved_in_order_and_checked() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let release = fixture.release(ReleaseInput {
        title: "Scenery".into(),
        alternate_titles: Some(vec![
            title(" シーナリィ ", Some("JA"), None),
            title("", None, None),
            title("Paysage", Some("fr"), Some(false)),
        ]),
        ..ReleaseInput::default()
    });
    assert_eq!(
        fixture.titles(release),
        [
            AlternateTitle {
                title: "シーナリィ".into(),
                language: Some("ja".into()),
                printed: true
            },
            AlternateTitle {
                title: "Paysage".into(),
                language: Some("fr".into()),
                printed: false
            },
        ]
    );

    let save = |input: ReleaseInput| catalog.update_release(release, &input);
    assert_eq!(
        invalid(save(ReleaseInput {
            alternate_titles: Some(vec![title("A", None, None), title(" A", None, None)]),
            ..titled("Scenery")
        })),
        "\"A\" is listed twice on the release"
    );
    assert_eq!(
        invalid(save(ReleaseInput {
            alternate_titles: Some(vec![title("Scenery", None, None)]),
            ..titled("Scenery")
        })),
        "\"Scenery\" is already the main title of the release"
    );
    // Left out, they stay, except one that's now the main title
    save(titled("Paysage")).unwrap();
    assert_eq!(
        fixture
            .titles(release)
            .into_iter()
            .map(|title| title.title)
            .collect::<Vec<_>>(),
        ["シーナリィ"]
    );
}

#[test]
fn lists_sort_by_title_or_date_search_titles_and_filter() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let alika = fixture.artist("Alika", &[]);
    let trio = fixture.label("Trio");
    let lp = fixture.format("LP");
    let cd = fixture.number("select id from release_media where name = 'CD'");
    let date = |text: &str| Some(text.to_string());
    let scenery = fixture.release(ReleaseInput {
        released: date("1976"),
        alternate_titles: Some(vec![title("シーナリィ", None, None)]),
        label_ids: vec![trio],
        format_ids: vec![lp],
        ..titled("Scenery")
    });
    let alika_release = fixture.release(ReleaseInput {
        released: date("2023-11-24"),
        artists: vec![credited(alika, None)],
        format_ids: vec![lp],
        medium_ids: vec![cd],
        ..titled("alika")
    });
    let undated = fixture.release(titled("Blank"));
    let all = ReleaseFilterArgs::default;

    assert_eq!(
        fixture.list(ListArgs::default(), all()),
        ["Blank", "alika", "Scenery"]
    );
    assert_eq!(
        fixture.list(sorted("title"), all()),
        ["alika", "Blank", "Scenery"]
    );
    // Undated releases go last, either way
    assert_eq!(
        fixture.list(sorted("released"), all()),
        ["Scenery", "alika", "Blank"]
    );
    assert_eq!(
        fixture.list(sorted("-released"), all()),
        ["alika", "Scenery", "Blank"]
    );
    assert_eq!(
        invalid(catalog.list_releases(&sorted("label"), &all())),
        "Releases can't be sorted by label"
    );
    // The search matches an alternate title too
    let searched = |q: &str| {
        fixture.list(
            ListArgs {
                q: Some(q.into()),
                ..ListArgs::default()
            },
            all(),
        )
    };
    assert_eq!(searched("シーナ"), ["Scenery"]);
    assert_eq!(searched("A"), ["Blank", "alika"]);

    let filtered = |filters: ReleaseFilterArgs| fixture.list(ListArgs::default(), filters);
    assert_eq!(
        filtered(ReleaseFilterArgs {
            format: Some(lp),
            medium: Some(cd),
            ..all()
        }),
        ["alika"]
    );
    assert_eq!(
        filtered(ReleaseFilterArgs {
            label: Some(trio),
            ..all()
        }),
        ["Scenery"]
    );
    assert_eq!(
        filtered(ReleaseFilterArgs {
            artist: Some(alika),
            ..all()
        }),
        ["alika"]
    );

    // Tag filters, from the tagging context
    let score = fixture
        .tagging
        .create_tag(
            TagType::Number,
            &TagInput {
                name: "Score".into(),
                multiple: false,
            },
        )
        .unwrap();
    for (release, value) in [(scenery, "9"), (alika_release, "7")] {
        fixture
            .tagging
            .set_tags(
                slunk::tagging::TagOwner::Release,
                release,
                &[TagValuesInput {
                    kind: TagType::Number,
                    tag_id: score.id,
                    values: vec![value.into()],
                }],
            )
            .unwrap();
    }
    let tags = tag_filters(&[TagFilterInput {
        kind: TagType::Number,
        tag_id: score.id,
        equals: None,
        contains: None,
        min: Some("8".into()),
        max: None,
    }])
    .unwrap();
    assert_eq!(
        filtered(ReleaseFilterArgs {
            tags: tags.clone(),
            ..all()
        }),
        ["Scenery"]
    );
    // Selecting them all: every match's id, newest first
    assert_eq!(
        catalog.release_ids(None, &all()).unwrap(),
        [undated, alika_release, scenery]
    );
    assert_eq!(
        catalog
            .release_ids(
                Some("e"),
                &ReleaseFilterArgs {
                    format: Some(lp),
                    ..all()
                }
            )
            .unwrap(),
        [scenery]
    );
}

#[test]
fn a_release_with_tracks_is_not_deleted() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let alika = fixture.artist("Alika", &[]);
    let release = fixture.release(ReleaseInput {
        artists: vec![credited(alika, None)],
        alternate_titles: Some(vec![title("Virmalised", None, None)]),
        ..titled("virmalised")
    });
    fixture.track(release, 1, 1, "virmalised");
    assert_eq!(catalog.release_blockers(release).unwrap(), ["Has 1 track"]);
    assert_eq!(
        invalid(catalog.delete_release(release)),
        "The release still has 1 track. Delete them first."
    );
    fixture.sql("delete from tracks");
    assert!(catalog.release_blockers(release).unwrap().is_empty());
    catalog.delete_release(release).unwrap();
    assert_eq!(catalog.get_release(release).unwrap(), None);
    // Its links and titles went with it, and its artist stays
    assert_eq!(fixture.number("select count(*) from artists_releases"), 0);
    assert_eq!(
        fixture.number("select count(*) from release_alternate_titles"),
        0
    );
    assert!(catalog.get_artist(alika).unwrap().is_some());
    // An unknown id deletes nothing
    catalog.delete_release(99).unwrap();
}

#[test]
fn credit_lines_are_checked_and_saved_with_the_tracks_lines() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let ryo = fixture.artist("Ryo Fukui", &["R. Fukui"]);
    let ann = fixture.artist("Ann Ronell", &[]);
    let release = fixture.release(titled("Scenery"));
    let other = fixture.release(titled("Mellow Dream"));
    let track = fixture.track(release, 1, 1, "It Could Happen to You");
    let elsewhere = fixture.track(other, 1, 1, "Mellow Dream");
    let save = |credits: Vec<CreditInput>, tracks: Vec<TrackCreditsInput>| {
        catalog.set_release_credits(release, &credits, &tracks)
    };

    save(
        vec![
            line(
                " Piano ",
                vec![credited(ryo, Some("R. Fukui")), credited(ryo, None)],
            ),
            line(
                "Composed By",
                vec![credited(ann, Some("A. Ronnell")), credited(ryo, None)],
            ),
        ],
        vec![TrackCreditsInput {
            track_id: track,
            credits: vec![line("Written-By", vec![credited(ann, Some("A. Ronnell"))])],
        }],
    )
    .unwrap();
    let lines = |owner, id: i64| -> Vec<(String, Vec<String>)> {
        catalog
            .credits_of(owner, &[id])
            .unwrap()
            .remove(&id)
            .unwrap_or_default()
            .into_iter()
            .map(|Credit { role, artists }| (role, fixture.shown(&artists)))
            .collect()
    };
    assert_eq!(
        lines(CreditOwner::Release, release),
        [
            ("Piano".to_string(), vec!["R. Fukui".to_string()]),
            (
                "Composed By".to_string(),
                vec!["A. Ronnell".to_string(), "Ryo Fukui".to_string()]
            ),
        ]
    );
    assert_eq!(
        lines(CreditOwner::Track, track),
        [("Written-By".to_string(), vec!["A. Ronnell".to_string()])]
    );
    // "A. Ronnell" was added once, and both lines are credited with it
    assert_eq!(fixture.names(ann), ["A. Ronnell ()"]);

    assert_eq!(
        invalid(save(vec![line(" ", vec![credited(ryo, None)])], vec![])),
        "A credit needs a role"
    );
    assert_eq!(
        invalid(save(vec![line("Piano", vec![])], vec![])),
        "The Piano credit needs an artist"
    );
    assert_eq!(
        invalid(save(
            vec![
                line("Mixed By", vec![credited(ryo, None)]),
                line("mixed by", vec![credited(ann, None)])
            ],
            vec![]
        )),
        "Two credits have the role mixed by, ignoring case. Put their artists on one line."
    );
    assert_eq!(
        invalid(save(
            vec![],
            vec![TrackCreditsInput {
                track_id: elsewhere,
                credits: vec![]
            }]
        )),
        format!("Track {elsewhere} isn't on this release")
    );
    assert_eq!(
        not_found(catalog.set_release_credits(99, &[], &[])),
        "There's no release 99"
    );
    // The saves that failed changed nothing
    assert_eq!(lines(CreditOwner::Release, release).len(), 2);

    // Roles in use: each spelling once, the most used first
    catalog
        .set_release_credits(
            other,
            &[
                line("Piano", vec![credited(ryo, None)]),
                line("composed by", vec![credited(ann, None)]),
            ],
            &[],
        )
        .unwrap();
    assert_eq!(
        catalog.credit_roles(None).unwrap(),
        ["Piano", "Composed By", "composed by", "Written-By"]
    );
    assert_eq!(
        catalog.credit_roles(Some(" BY ")).unwrap(),
        ["Composed By", "composed by", "Written-By"]
    );
}

#[test]
fn covers_and_file_links_register_paths_in_the_same_transaction() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let release = fixture.release(titled("Scenery"));
    fixture.write("music/cover.jpg", "jpeg");
    fixture.write("music/rip.log", "log");
    let by_path = |path: &str| FileRef {
        file_id: None,
        path: Some(path.into()),
    };

    let saved = catalog
        .set_release_files(
            release,
            Some(&by_path("music/cover.jpg")),
            &[FileLinkInput {
                file: by_path("music/rip.log"),
                note: Some(" Rip log ".into()),
            }],
        )
        .unwrap();
    let cover = saved.cover_picture.unwrap();
    let conn = fixture.store.conn();
    let note: String = conn
        .query_row("select note from files_releases", [], |row| row.get(0))
        .unwrap();
    drop(conn);
    assert_eq!(note, "Rip log");

    // The cover alone, keeping the links; then none, keeping them too
    let rip = fixture.number("select file_id from files_releases");
    let swapped = catalog
        .set_release_cover(
            release,
            &FileRef {
                file_id: Some(cover),
                path: None,
            },
        )
        .unwrap();
    assert_eq!(swapped.cover_picture, Some(cover));
    let cleared = catalog
        .set_release_files(
            release,
            None,
            &[FileLinkInput {
                file: FileRef {
                    file_id: Some(rip),
                    path: None,
                },
                note: None,
            }],
        )
        .unwrap();
    assert_eq!(cleared.cover_picture, None);

    // A file the database refuses, or a release that isn't there, leaves
    // nothing registered
    let registered = fixture.number("select count(*) from files");
    fixture.write("music/notes.txt", "notes");
    assert!(
        catalog
            .set_release_cover(release, &by_path("music/notes.txt"))
            .is_err()
    );
    assert_eq!(
        not_found(catalog.set_release_cover(99, &by_path("music/notes.txt"))),
        "There's no release 99"
    );
    assert_eq!(fixture.number("select count(*) from files"), registered);
    assert_eq!(
        invalid(catalog.set_release_cover(release, &FileRef::default())),
        "A file needs an id or a path"
    );
}

#[test]
fn the_artist_page_counts_appearances_credits_and_formats() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let alika = fixture.artist("Alika", &["ALIKA"]);
    let guest = fixture.artist("Guest", &[]);
    let lp = fixture.format("LP");
    let own = fixture.release(ReleaseInput {
        artists: vec![credited(alika, None)],
        format_ids: vec![lp],
        ..titled("virmalised")
    });
    let other = fixture.release(ReleaseInput {
        artists: vec![credited(guest, None)],
        ..titled("Elsewhere")
    });
    let on_own = fixture.track(own, 1, 1, "One");
    let (first, second) = (
        fixture.track(other, 1, 2, "Two"),
        fixture.track(other, 1, 1, "One"),
    );
    fixture.sql(&format!(
        "insert into artists_tracks (track_id, artist_id, position) values ({on_own}, {alika}, 0);
         insert into artists_tracks (track_id, artist_id, featured, position) values ({first}, {alika}, 1, 0);
         insert into artists_tracks (track_id, artist_id, position) values ({second}, {alika}, 0);"
    ));
    // On tracks of a release it isn't a release artist on, in order
    let appearances = catalog.artist_appearances(alika).unwrap();
    assert_eq!(appearances.len(), 1);
    assert_eq!(
        (appearances[0].release_id, appearances[0].track_ids.clone()),
        (other, vec![second, first])
    );
    assert_eq!(catalog.artist_format_counts(alika).unwrap()[0].count, 1);

    catalog
        .set_release_credits(
            other,
            &[line("Producer", vec![credited(alika, Some("ALIKA"))])],
            &[TrackCreditsInput {
                track_id: first,
                credits: vec![line("Vocals", vec![credited(alika, None)])],
            }],
        )
        .unwrap();
    let credits: Vec<ArtistCredit> = catalog.artist_credits(alika).unwrap();
    assert_eq!(
        credits
            .iter()
            .map(|credit| (
                credit.role.as_str(),
                credit.track_id,
                credit.credited_name_id.is_some()
            ))
            .collect::<Vec<_>>(),
        [("Producer", None, true), ("Vocals", Some(first), false)]
    );

    let totals = catalog.totals().unwrap();
    assert_eq!(
        (
            totals.releases,
            totals.artists,
            totals.tracks,
            totals.duration_ms
        ),
        (2, 2, 3, 3000)
    );
}
