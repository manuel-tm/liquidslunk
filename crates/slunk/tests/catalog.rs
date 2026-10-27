//! The catalog context, on a temporary library: its rules for artists,
//! labels and the lists the user manages, as docs/development/backend.md
//! lists them ("The catalog context").

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use slunk::Error;
use slunk::catalog::{
    AlternateName, AlternateNameInput, ArtistInput, Catalog, FileLinkInput, FileRef, LabelInput,
    ListKind,
};
use slunk::media::{FileLink, Library, Media};
use slunk::paging::ListArgs;
use slunk::repo::store::Store;
use slunk::repo::{connection, migrator};
use tempfile::TempDir;

/// A library in `<tmp>/library/`.
struct Fixture {
    dir: TempDir,
    store: Arc<Store>,
    media: Arc<Media>,
    catalog: Catalog,
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
        let catalog = Catalog::new(store.clone(), media.clone());
        Fixture {
            dir,
            store,
            media,
            catalog,
        }
    }

    fn library(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap().join("library")
    }

    /// Writes a file, relative to the library's folder.
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

    fn artist(&self, name: &str, names: &[(&str, bool)]) -> i64 {
        self.catalog
            .create_artist(&ArtistInput {
                name: name.into(),
                alternate_names: Some(
                    names
                        .iter()
                        .map(|&(name, real)| alternate(name, real))
                        .collect(),
                ),
                ..ArtistInput::default()
            })
            .unwrap()
            .id
    }

    fn names(&self, artist: i64) -> Vec<AlternateName> {
        self.catalog
            .artist_names_of(&[artist])
            .unwrap()
            .remove(&artist)
            .unwrap_or_default()
    }

    /// The artist's names, as `name` or `name (real)`.
    fn name_list(&self, artist: i64) -> Vec<String> {
        self.names(artist)
            .into_iter()
            .map(|name| {
                if name.real_name {
                    format!("{} (real)", name.name)
                } else {
                    name.name
                }
            })
            .collect()
    }

    fn name_id(&self, artist: i64, name: &str) -> i64 {
        self.names(artist)
            .into_iter()
            .find(|known| known.name == name)
            .unwrap_or_else(|| panic!("no name {name}"))
            .id
    }

    /// A release with the artist on it, credited with a name or its main
    /// name, and returns the release's id.
    fn release_by(&self, artist: i64, credited: Option<i64>) -> i64 {
        let credited = credited.map_or("null".into(), |id| id.to_string());
        self.sql(&format!(
            "insert into releases (title) values ('A release');
             insert into artists_releases (artist_id, release_id, position, credited_name_id)
             values ({artist}, last_insert_rowid(), 0, {credited});"
        ));
        self.number("select max(id) from releases")
    }

    /// The name each of the artist's release links shows: `None` for the main
    /// name.
    fn credited(&self, artist: i64) -> Vec<Option<String>> {
        let conn = self.store.conn();
        let mut statement = conn
            .prepare(
                "select n.name from artists_releases a
                 left join artist_alternate_names n on n.id = a.credited_name_id
                 where a.artist_id = ? order by a.release_id",
            )
            .unwrap();
        statement
            .query_map([artist], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn update(&self, id: i64, input: ArtistInput) -> slunk::Result<slunk::catalog::Artist> {
        self.catalog.update_artist(id, &input)
    }
}

fn alternate(name: &str, real_name: bool) -> AlternateNameInput {
    AlternateNameInput {
        name: name.into(),
        real_name,
        ..AlternateNameInput::default()
    }
}

/// The artist's names as the form sends them back: with their ids.
fn kept(names: &[AlternateName]) -> Vec<AlternateNameInput> {
    names
        .iter()
        .map(|name| AlternateNameInput {
            id: Some(name.id),
            name: name.name.clone(),
            language: name.language.clone(),
            real_name: name.real_name,
            note: name.note.clone(),
        })
        .collect()
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

fn list(sort: Option<&str>, q: Option<&str>) -> ListArgs {
    ListArgs {
        sort: sort.map(Into::into),
        q: q.map(Into::into),
        ..ListArgs::default()
    }
}

#[test]
fn lists_page_sort_by_name_ignoring_case_and_search() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let kind = ListKind::ReleaseFormats;
    catalog.create_list_item(kind, " box set ").unwrap();
    let names = |args| -> Vec<String> {
        catalog
            .list_items(kind, &args)
            .unwrap()
            .items
            .into_iter()
            .map(|item| item.name)
            .collect()
    };
    // The baseline's starter rows, and the new one, trimmed
    assert_eq!(
        names(list(Some("name"), None)),
        [
            "Album",
            "box set",
            "Compilation",
            "EP",
            "Live",
            "LP",
            "Mixtape",
            "Single",
            "Soundtrack"
        ]
    );
    assert_eq!(names(list(None, None))[0], "box set");
    assert_eq!(
        names(list(None, Some("L"))),
        ["Single", "LP", "Live", "Compilation", "Album"]
    );
    assert_eq!(
        catalog
            .list_items(kind, &list(None, Some("l")))
            .unwrap()
            .total,
        5
    );
    assert_eq!(
        invalid(catalog.list_items(kind, &list(Some("uses"), None))),
        "Lists can't be sorted by uses"
    );
    let media: Vec<String> = catalog
        .list_items(ListKind::ReleaseMedia, &list(Some("-name"), None))
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.name)
        .collect();
    assert_eq!(media, ["Vinyl", "Digital", "CD", "Cassette"]);
}

#[test]
fn list_names_are_unique_ignoring_case() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let error = catalog
        .create_list_item(ListKind::ReleaseFormats, "album")
        .unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("UNIQUE constraint failed: release_formats.name"),
        "{error}"
    );
    // The other list has its own names
    catalog
        .create_list_item(ListKind::ReleaseMedia, "Album")
        .unwrap();
    let renamed = catalog
        .rename_list_item(ListKind::ReleaseFormats, 1, "ALBUM")
        .unwrap();
    assert_eq!(renamed.name, "ALBUM");
    assert_eq!(
        not_found(catalog.rename_list_item(ListKind::ReleaseFormats, 99, "Box")),
        "There's no format 99"
    );
    assert_eq!(
        not_found(catalog.rename_list_item(ListKind::ReleaseMedia, 99, "Box")),
        "There's no medium 99"
    );
}

#[test]
fn a_format_or_medium_in_use_is_not_deleted() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    fixture.sql(
        "insert into releases (title) values ('Scenery'), ('Alika');
         insert into release_formats_releases (format_id, release_id) values (1, 1), (1, 2);
         insert into release_media_releases (medium_id, release_id) values (2, 1);",
    );
    let formats = ListKind::ReleaseFormats;
    assert_eq!(
        catalog.list_item_blockers(formats, 1).unwrap(),
        ["Used by 2 releases"]
    );
    assert_eq!(
        invalid(catalog.delete_list_item(formats, 1)),
        "The format is used by 2 releases"
    );
    assert_eq!(
        invalid(catalog.delete_list_item(ListKind::ReleaseMedia, 2)),
        "The medium is used by 1 release"
    );
    assert_eq!(
        catalog.list_item_uses(formats, &[1, 2]).unwrap().get(&1),
        Some(&2)
    );
    assert!(catalog.list_item_blockers(formats, 2).unwrap().is_empty());
    catalog.delete_list_item(formats, 2).unwrap();
    assert_eq!(catalog.get_list_item(formats, 2).unwrap(), None);
    // An unknown id deletes nothing, as in slunk
    catalog.delete_list_item(formats, 99).unwrap();
}

#[test]
fn artists_and_labels_are_found_by_any_of_their_names() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let ryo = fixture.artist("Ryo Fukui", &[("福居良", false), ("R. Fukui", false)]);
    let alika = fixture.artist("alika", &[("ALIKA", false)]);
    fixture.artist("Drake", &[]);
    let found = |q| -> Vec<i64> {
        catalog
            .list_artists(&list(Some("name"), Some(q)))
            .unwrap()
            .items
            .into_iter()
            .map(|artist| artist.id)
            .collect()
    };
    assert_eq!(found("福居"), [ryo]);
    assert_eq!(found("r. f"), [ryo]);
    assert_eq!(found("ali"), [alika]);
    // Sorted by name ignoring case, then by id
    let names: Vec<String> = catalog
        .list_artists(&list(Some("name"), None))
        .unwrap()
        .items
        .into_iter()
        .map(|artist| artist.name)
        .collect();
    assert_eq!(names, ["alika", "Drake", "Ryo Fukui"]);
    assert_eq!(
        invalid(catalog.list_artists(&list(Some("picture"), None))),
        "Artists can't be sorted by picture"
    );

    let label = catalog
        .create_label(&LabelInput {
            name: "Trio Records".into(),
            alternate_names: Some(vec![alternate("トリオレコード", false)]),
            ..LabelInput::default()
        })
        .unwrap();
    let labels = catalog.list_labels(&list(None, Some("トリオ"))).unwrap();
    assert_eq!(labels.items[0].id, label.id);
    assert_eq!(
        invalid(catalog.list_labels(&list(Some("logo"), None))),
        "Labels can't be sorted by logo"
    );
}

#[test]
fn alternate_names_are_saved_in_order_and_cleaned() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let id = fixture.artist(
        " Ryo  Fukui ",
        &[("福居良", false), ("  ", false), ("R. Fukui ", false)],
    );
    let artist = catalog.get_artist(id).unwrap().unwrap();
    assert_eq!(artist.name, "Ryo  Fukui");
    assert_eq!(fixture.name_list(id), ["福居良", "R. Fukui"]);

    // An id keeps its row, a name without one is added, and one left out goes
    let names = fixture.names(id);
    let mut input = kept(&names[1..]);
    input.push(AlternateNameInput {
        name: "Cafe\u{301}".into(),
        language: Some("FR".into()),
        note: Some(" a note ".into()),
        ..AlternateNameInput::default()
    });
    fixture
        .update(
            id,
            ArtistInput {
                name: "Ryo Fukui".into(),
                alternate_names: Some(input),
                ..ArtistInput::default()
            },
        )
        .unwrap();
    let saved = fixture.names(id);
    assert_eq!(saved[0].id, names[1].id);
    assert_eq!(saved[1].name, "Caf\u{e9}");
    assert_eq!(saved[1].language.as_deref(), Some("fr"));
    assert_eq!(saved[1].note.as_deref(), Some("a note"));

    // Left out entirely, they stay as they are
    fixture
        .update(
            id,
            ArtistInput {
                name: "Ryo Fukui".into(),
                ..ArtistInput::default()
            },
        )
        .unwrap();
    assert_eq!(fixture.names(id), saved);

    // They compare exactly, case included
    let alika = fixture.artist("Alika", &[("ALIKA", false), ("alika", false)]);
    assert_eq!(fixture.name_list(alika), ["ALIKA", "alika"]);
}

#[test]
fn a_name_twice_or_the_main_name_is_refused() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let save = |name: &str, names: &[(&str, bool)]| {
        catalog.create_artist(&ArtistInput {
            name: name.into(),
            alternate_names: Some(
                names
                    .iter()
                    .map(|&(name, real)| alternate(name, real))
                    .collect(),
            ),
            ..ArtistInput::default()
        })
    };
    assert_eq!(
        invalid(save("Alika", &[("ALIKA", false), (" ALIKA", false)])),
        "\"ALIKA\" is listed twice"
    );
    assert_eq!(
        invalid(save("Alika", &[("Alika", false)])),
        "\"Alika\" is already the artist's main name"
    );
    // Nothing was saved
    assert_eq!(catalog.list_artists(&ListArgs::default()).unwrap().total, 0);
    // A real name can be the main name too
    let id = save("Alika Milova", &[("Alika Milova", true)]).unwrap().id;
    assert_eq!(fixture.name_list(id), ["Alika Milova (real)"]);

    // Labels have no real names
    let label = catalog.create_label(&LabelInput {
        name: "Nadja".into(),
        alternate_names: Some(vec![alternate("Nadja", true)]),
        ..LabelInput::default()
    });
    assert_eq!(invalid(label), "\"Nadja\" is already the label's main name");

    // A name that isn't the artist's own
    let other = fixture.artist("Drake", &[("Drizzy", false)]);
    let drizzy = fixture.name_id(other, "Drizzy");
    let error = fixture.update(
        id,
        ArtistInput {
            name: "Alika Milova".into(),
            alternate_names: Some(vec![AlternateNameInput {
                id: Some(drizzy),
                name: "Drizzy".into(),
                ..AlternateNameInput::default()
            }]),
            ..ArtistInput::default()
        },
    );
    assert_eq!(
        invalid(error),
        format!("Name {drizzy} isn't one of its alternate names")
    );
}

#[test]
fn languages_are_saved_canonical_or_refused() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let artist = catalog
        .create_artist(&ArtistInput {
            name: "Ryo Fukui".into(),
            name_language: Some("JA-latn".into()),
            alternate_names: Some(vec![AlternateNameInput {
                name: "福居良".into(),
                language: Some("jpn".into()),
                ..AlternateNameInput::default()
            }]),
            ..ArtistInput::default()
        })
        .unwrap();
    assert_eq!(artist.name_language.as_deref(), Some("ja-Latn"));
    assert_eq!(fixture.names(artist.id)[0].language.as_deref(), Some("ja"));
    let refused = catalog.create_label(&LabelInput {
        name: "Nadja".into(),
        name_language: Some("ja_JP".into()),
        ..LabelInput::default()
    });
    assert_eq!(
        invalid(refused),
        "\"ja_JP\" isn't a language tag, like en, ja or ja-Latn"
    );
    let refused = catalog.create_artist(&ArtistInput {
        name: "Teruo Isono".into(),
        alternate_names: Some(vec![AlternateNameInput {
            name: "いソノてルヲ".into(),
            language: Some("und".into()),
            ..AlternateNameInput::default()
        }]),
        ..ArtistInput::default()
    });
    assert_eq!(
        invalid(refused),
        "Leave the language blank when it isn't known"
    );
    // Neither was saved
    assert_eq!(catalog.list_labels(&ListArgs::default()).unwrap().total, 0);
    assert_eq!(catalog.list_artists(&ListArgs::default()).unwrap().total, 1);

    // In use, the most used first
    catalog
        .create_label(&LabelInput {
            name: "Trio Records".into(),
            name_language: Some("en".into()),
            alternate_names: Some(vec![AlternateNameInput {
                name: "トリオレコード".into(),
                language: Some("ja".into()),
                ..AlternateNameInput::default()
            }]),
            ..LabelInput::default()
        })
        .unwrap();
    assert_eq!(catalog.languages().unwrap(), ["ja", "en", "ja-Latn"]);
}

#[test]
fn a_new_name_the_artist_has_already_keeps_its_row_and_links() {
    let fixture = Fixture::new();
    let alika = fixture.artist("Alika", &[("ALIKA", false)]);
    let name = fixture.name_id(alika, "ALIKA");
    fixture.release_by(alika, Some(name));
    // Sent without its id, as a form that lost it would
    fixture
        .update(
            alika,
            ArtistInput {
                name: "Alika".into(),
                alternate_names: Some(vec![
                    alternate("Alika Milova", true),
                    alternate("ALIKA", false),
                ]),
                ..ArtistInput::default()
            },
        )
        .unwrap();
    assert_eq!(fixture.name_id(alika, "ALIKA"), name);
    assert_eq!(fixture.credited(alika), [Some("ALIKA".into())]);
    assert_eq!(fixture.name_list(alika), ["Alika Milova (real)", "ALIKA"]);
}

#[test]
fn names_can_trade_places_in_one_save() {
    let fixture = Fixture::new();
    let ryo = fixture.artist("Ryo Fukui", &[("福居良", false), ("R. Fukui", false)]);
    let names = fixture.names(ryo);
    let mut swapped = kept(&names);
    swapped[0].name = "R. Fukui".into();
    swapped[1].name = "福居良".into();
    fixture
        .update(
            ryo,
            ArtistInput {
                name: "Ryo Fukui".into(),
                alternate_names: Some(swapped),
                ..ArtistInput::default()
            },
        )
        .unwrap();
    let saved = fixture.names(ryo);
    assert_eq!(
        (saved[0].id, saved[0].name.as_str()),
        (names[0].id, "R. Fukui")
    );
    assert_eq!(
        (saved[1].id, saved[1].name.as_str()),
        (names[1].id, "福居良")
    );
}

#[test]
fn removing_a_name_credits_its_links_with_the_main_name() {
    let fixture = Fixture::new();
    let alika = fixture.artist("Alika", &[("ALIKA", false), ("Alika M.", false)]);
    let shouted = fixture.name_id(alika, "ALIKA");
    fixture.release_by(alika, Some(shouted));
    fixture.release_by(alika, None);
    assert_eq!(
        fixture.catalog.artist_name_uses(&[shouted]).unwrap()[&shouted],
        1
    );
    assert_eq!(fixture.catalog.main_name_uses(alika).unwrap(), 1);
    let names = fixture.names(alika);
    fixture
        .update(
            alika,
            ArtistInput {
                name: "Alika".into(),
                alternate_names: Some(kept(&names[1..])),
                ..ArtistInput::default()
            },
        )
        .unwrap();
    assert_eq!(fixture.credited(alika), [None, None]);
    assert_eq!(fixture.name_list(alika), ["Alika M."]);
    assert_eq!(fixture.catalog.main_name_uses(alika).unwrap(), 2);
}

#[test]
fn renaming_an_alternate_name_renames_it_on_its_links() {
    let fixture = Fixture::new();
    let ann = fixture.artist("Ann Ronell", &[("A. Ronnell", false)]);
    let misprint = fixture.name_id(ann, "A. Ronnell");
    fixture.release_by(ann, Some(misprint));
    let mut names = kept(&fixture.names(ann));
    names[0].name = "A. Ronell".into();
    fixture
        .update(
            ann,
            ArtistInput {
                name: "Ann Ronell".into(),
                alternate_names: Some(names),
                ..ArtistInput::default()
            },
        )
        .unwrap();
    assert_eq!(fixture.credited(ann), [Some("A. Ronell".into())]);
    assert_eq!(fixture.name_id(ann, "A. Ronell"), misprint);
}

#[test]
fn a_rename_can_keep_the_old_name_on_the_links_that_showed_it() {
    let fixture = Fixture::new();
    let ryo = fixture.artist("Ryo Fukui", &[("R. Fukui", false)]);
    let initials = fixture.name_id(ryo, "R. Fukui");
    fixture.release_by(ryo, None);
    fixture.release_by(ryo, Some(initials));
    let rename = |name: &str, keep_old_name| {
        fixture.update(
            ryo,
            ArtistInput {
                name: name.into(),
                name_language: Some("en".into()),
                keep_old_name,
                ..ArtistInput::default()
            },
        )
    };

    // Without keeping it, the links that showed the main name show the new
    rename("Ryo Fukui Trio", false).unwrap();
    assert_eq!(fixture.credited(ryo), [None, Some("R. Fukui".into())]);
    assert_eq!(fixture.name_list(ryo), ["R. Fukui"]);

    // Kept, the old name is added after the others, with its language
    rename("福居良", true).unwrap();
    assert_eq!(
        fixture.credited(ryo),
        [Some("Ryo Fukui Trio".into()), Some("R. Fukui".into())]
    );
    assert_eq!(fixture.name_list(ryo), ["R. Fukui", "Ryo Fukui Trio"]);
    assert_eq!(fixture.names(ryo)[1].language.as_deref(), Some("en"));

    // With no link showing the main name, there's nothing to keep it on
    rename("Ryo", true).unwrap();
    assert_eq!(fixture.name_list(ryo), ["R. Fukui", "Ryo Fukui Trio"]);
}

#[test]
fn keeping_the_old_name_comes_before_the_names_that_go() {
    let fixture = Fixture::new();
    let alika = fixture.artist("Alika", &[("ALIKA", false)]);
    let shouted = fixture.name_id(alika, "ALIKA");
    fixture.release_by(alika, None);
    fixture.release_by(alika, Some(shouted));
    // Renamed, keeping "Alika", while "ALIKA" goes
    fixture
        .update(
            alika,
            ArtistInput {
                name: "Alika Milova".into(),
                alternate_names: Some(Vec::new()),
                keep_old_name: true,
                ..ArtistInput::default()
            },
        )
        .unwrap();
    assert_eq!(fixture.credited(alika), [Some("Alika".into()), None]);
    assert_eq!(fixture.name_list(alika), ["Alika"]);
}

#[test]
fn renaming_to_an_alternate_name_promotes_it() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let alika = fixture.artist("Alika", &[("ALIKA", false), ("Alika Milova", true)]);
    let shouted = fixture.name_id(alika, "ALIKA");
    let real = fixture.name_id(alika, "Alika Milova");
    fixture.release_by(alika, Some(shouted));
    fixture.release_by(alika, Some(real));
    let rename = |name: &str| {
        catalog.update_artist(
            alika,
            &ArtistInput {
                name: name.into(),
                ..ArtistInput::default()
            },
        )
    };

    // With the list left out, the name goes and its links show the main name
    rename("ALIKA").unwrap();
    assert_eq!(fixture.credited(alika), [None, Some("Alika Milova".into())]);
    assert_eq!(fixture.name_list(alika), ["Alika Milova (real)"]);

    // A real name stays one, and its links show the main name, the same text
    rename("Alika Milova").unwrap();
    assert_eq!(fixture.credited(alika), [None, None]);
    assert_eq!(fixture.name_list(alika), ["Alika Milova (real)"]);
}

#[test]
fn make_main_swaps_the_names_and_every_link_shows_what_it_did() {
    let fixture = Fixture::new();
    let ryo = fixture.artist("Ryo Fukui", &[("福居良", false), ("R. Fukui", false)]);
    let kanji = fixture.name_id(ryo, "福居良");
    let initials = fixture.name_id(ryo, "R. Fukui");
    fixture.release_by(ryo, None);
    fixture.release_by(ryo, Some(kanji));
    fixture.release_by(ryo, Some(initials));
    // What the form sends: the promoted name dropped, the old main name as a
    // new row in its place, and keepOldName
    let names = fixture.names(ryo);
    let mut input = vec![alternate("Ryo Fukui", false)];
    input.extend(kept(&names[1..]));
    fixture
        .update(
            ryo,
            ArtistInput {
                name: "福居良".into(),
                name_language: Some("ja".into()),
                alternate_names: Some(input),
                keep_old_name: true,
                ..ArtistInput::default()
            },
        )
        .unwrap();
    assert_eq!(fixture.name_list(ryo), ["Ryo Fukui", "R. Fukui"]);
    assert_eq!(
        fixture.credited(ryo),
        [Some("Ryo Fukui".into()), None, Some("R. Fukui".into())]
    );
}

#[test]
fn a_labels_names_are_replaced_and_kept_when_left_out() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let label = catalog
        .create_label(&LabelInput {
            name: "Trio Records".into(),
            alternate_names: Some(vec![
                alternate("トリオレコード", false),
                alternate("Trio", false),
            ]),
            ..LabelInput::default()
        })
        .unwrap();
    let names = |id| -> Vec<String> {
        catalog
            .label_names_of(&[id])
            .unwrap()
            .remove(&id)
            .unwrap_or_default()
            .into_iter()
            .map(|name| name.name)
            .collect()
    };
    assert_eq!(names(label.id), ["トリオレコード", "Trio"]);
    let rename = |name: &str| {
        catalog.update_label(
            label.id,
            &LabelInput {
                name: name.into(),
                ..LabelInput::default()
            },
        )
    };
    rename("Trio Records Inc.").unwrap();
    assert_eq!(names(label.id), ["トリオレコード", "Trio"]);
    // Renamed to one of them, it goes
    rename("Trio").unwrap();
    assert_eq!(names(label.id), ["トリオレコード"]);
    assert_eq!(not_found(rename_label(catalog, 99)), "There's no label 99");
}

fn rename_label(catalog: &Catalog, id: i64) -> slunk::Result<slunk::catalog::Label> {
    catalog.update_label(
        id,
        &LabelInput {
            name: "Nadja".into(),
            ..LabelInput::default()
        },
    )
}

#[test]
fn pictures_and_logos_register_a_path_in_the_same_transaction() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    fixture.write("Drake/drake.jpg", "jpeg");
    fixture.write("Drake/notes.txt", "notes");
    let drake = fixture.artist("Drake", &[]);
    let by_path = |path: &str| FileRef {
        path: Some(path.into()),
        ..FileRef::default()
    };
    let artist = catalog
        .set_artist_picture(drake, &by_path("./Drake//drake.jpg"))
        .unwrap();
    let picture = artist.picture.unwrap();
    assert_eq!(
        fixture.media.get_file(picture).unwrap().unwrap().path,
        "Drake/drake.jpg"
    );

    // A file the database refuses isn't left registered
    let error = catalog
        .set_artist_picture(drake, &by_path("Drake/notes.txt"))
        .unwrap_err();
    assert_eq!(error.to_string(), "an artist picture must be an image");
    // Nor is one for an artist that isn't there
    let label = catalog.set_label_picture(99, &by_path("Drake/notes.txt"));
    assert_eq!(not_found(label), "There's no label 99");
    assert_eq!(fixture.number("select count(*) from files"), 1);

    // By id, a file already registered
    let label = catalog
        .create_label(&LabelInput {
            name: "OVO".into(),
            ..LabelInput::default()
        })
        .unwrap();
    let logo = FileRef {
        file_id: Some(picture),
        ..FileRef::default()
    };
    assert_eq!(
        catalog.set_label_picture(label.id, &logo).unwrap().picture,
        Some(picture)
    );
    assert_eq!(
        invalid(catalog.set_label_picture(label.id, &FileRef::default())),
        "A file needs an id or a path"
    );
    assert_eq!(
        invalid(catalog.set_label_picture(label.id, &by_path("Drake/gone.jpg"))),
        "There's no file at Drake/gone.jpg"
    );
}

#[test]
fn an_artists_file_links_register_paths_with_the_artist() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    fixture.write("Ryo Fukui/scan.png", "png");
    fixture.write("Ryo Fukui/interview.txt", "text");
    let link = |path: &str, note: Option<&str>| FileLinkInput {
        file: FileRef {
            path: Some(path.into()),
            ..FileRef::default()
        },
        note: note.map(Into::into),
    };
    let artist = catalog
        .create_artist(&ArtistInput {
            name: "Ryo Fukui".into(),
            files: vec![
                link("Ryo Fukui/interview.txt", Some(" Swing Journal ")),
                link("Ryo Fukui/scan.png", Some("  ")),
            ],
            ..ArtistInput::default()
        })
        .unwrap();
    let links = catalog.artist_files(artist.id).unwrap();
    assert_eq!(
        links,
        [
            FileLink {
                file_id: 1,
                note: Some("Swing Journal".into())
            },
            FileLink {
                file_id: 2,
                note: None
            }
        ]
    );

    // A path that isn't there registers nothing and saves nothing
    let error = fixture.update(
        artist.id,
        ArtistInput {
            name: "Ryo".into(),
            files: vec![
                link("Ryo Fukui/scan.png", None),
                link("Ryo Fukui/gone.png", None),
            ],
            ..ArtistInput::default()
        },
    );
    assert_eq!(invalid(error), "There's no file at Ryo Fukui/gone.png");
    assert_eq!(
        catalog.get_artist(artist.id).unwrap().unwrap().name,
        "Ryo Fukui"
    );
    assert_eq!(catalog.artist_files(artist.id).unwrap().len(), 2);

    // An unknown artist registers nothing either
    fixture.write("Ryo Fukui/new.png", "png");
    let error = fixture.update(
        99,
        ArtistInput {
            name: "Ryo".into(),
            files: vec![link("Ryo Fukui/new.png", None)],
            ..ArtistInput::default()
        },
    );
    assert_eq!(not_found(error), "There's no artist 99");
    assert_eq!(fixture.number("select count(*) from files"), 2);
}

#[test]
fn deletes_say_what_blocks_them() {
    let fixture = Fixture::new();
    let catalog = &fixture.catalog;
    let drake = fixture.artist("Drake", &[]);
    let release = fixture.release_by(drake, None);
    fixture.sql(&format!(
        "begin;
         insert into release_credits (release_id, role, position) values ({release}, 'Producer', 0);
         insert into artists_release_credits (credit_id, artist_id, position) values (last_insert_rowid(), {drake}, 0);
         insert into release_credits (release_id, role, position) values ({release}, 'Mixed By', 1);
         insert into artists_release_credits (credit_id, artist_id, position) values (last_insert_rowid(), {drake}, 0);
         commit;"
    ));
    assert_eq!(
        catalog.artist_blockers(drake).unwrap(),
        ["Release artist on 1 release", "Credited 2 times"]
    );
    assert_eq!(
        invalid(catalog.delete_artist(drake)),
        "The artist is still used: release artist on 1 release, credited 2 times"
    );

    let label = catalog
        .create_label(&LabelInput {
            name: "OVO".into(),
            ..LabelInput::default()
        })
        .unwrap();
    fixture.sql(&format!(
        "insert into labels_releases (label_id, release_id, position) values ({}, {release}, 0)",
        label.id
    ));
    assert_eq!(catalog.label_blockers(label.id).unwrap(), ["On 1 release"]);
    assert_eq!(
        invalid(catalog.delete_label(label.id)),
        "The label is on 1 release"
    );

    // Unused, they go, with their names and file links, and their files stay
    // registered
    fixture.write("Alika/press.txt", "press");
    let alika = catalog
        .create_artist(&ArtistInput {
            name: "Alika".into(),
            alternate_names: Some(vec![alternate("ALIKA", false)]),
            files: vec![FileLinkInput {
                file: FileRef {
                    path: Some("Alika/press.txt".into()),
                    ..FileRef::default()
                },
                note: None,
            }],
            ..ArtistInput::default()
        })
        .unwrap()
        .id;
    assert!(catalog.artist_blockers(alika).unwrap().is_empty());
    catalog.delete_artist(alika).unwrap();
    assert_eq!(catalog.get_artist(alika).unwrap(), None);
    assert_eq!(
        fixture.number("select count(*) from artist_alternate_names"),
        0
    );
    assert_eq!(fixture.number("select count(*) from artists_files"), 0);
    assert_eq!(fixture.number("select count(*) from files"), 1);
}

impl Fixture {
    /// Saves only the artist's aliases, as the form does.
    fn set_aliases(&self, id: i64, aliases: &[i64]) -> slunk::Result<slunk::catalog::Artist> {
        let artist = self.catalog.get_artist(id).unwrap().unwrap();
        self.update(
            id,
            ArtistInput {
                name: artist.name,
                aliases: Some(aliases.to_vec()),
                ..ArtistInput::default()
            },
        )
    }

    /// The names of the artist's aliases, in order.
    fn aliases(&self, id: i64) -> Vec<String> {
        self.catalog
            .artist_aliases(id)
            .unwrap()
            .into_iter()
            .map(|artist| artist.name)
            .collect()
    }
}

#[test]
fn every_artist_in_a_group_is_an_alias_of_the_others() {
    let fixture = Fixture::new();
    let joel = fixture.artist("Joel Thomas Zimmerman", &[]);
    let deadmau5 = fixture.artist("deadmau5", &[]);
    let testpilot = fixture.artist("testpilot", &[]);
    let other = fixture.artist("Ryo Fukui", &[]);
    fixture.set_aliases(joel, &[testpilot, deadmau5]).unwrap();

    // By name ignoring case, then by id
    assert_eq!(fixture.aliases(joel), ["deadmau5", "testpilot"]);
    assert_eq!(
        fixture.aliases(deadmau5),
        ["Joel Thomas Zimmerman", "testpilot"]
    );
    assert_eq!(
        fixture.aliases(testpilot),
        ["deadmau5", "Joel Thomas Zimmerman"]
    );
    assert!(fixture.aliases(other).is_empty());
    assert!(fixture.aliases(999).is_empty());
    assert_eq!(
        fixture.number("select count(distinct alias_group) from artist_aliases"),
        1
    );

    // A new artist joins with the whole group, as the form sends it
    let halcyon = fixture
        .catalog
        .create_artist(&ArtistInput {
            name: "halcyon".into(),
            aliases: Some(vec![deadmau5, joel, testpilot]),
            ..ArtistInput::default()
        })
        .unwrap()
        .id;
    assert_eq!(
        fixture.aliases(testpilot),
        ["deadmau5", "halcyon", "Joel Thomas Zimmerman"]
    );
    assert_eq!(
        fixture.aliases(halcyon),
        ["deadmau5", "Joel Thomas Zimmerman", "testpilot"]
    );
}

#[test]
fn linking_artists_with_aliases_joins_their_groups() {
    let fixture = Fixture::new();
    let [a, b, c, d] = ["A", "B", "C", "D"].map(|name| fixture.artist(name, &[]));
    fixture.set_aliases(a, &[b]).unwrap();
    fixture.set_aliases(c, &[d]).unwrap();
    assert_eq!(fixture.aliases(a), ["B"]);
    assert_eq!(fixture.aliases(d), ["C"]);

    // Picking C brings its alias D, so the form sends both
    fixture.set_aliases(a, &[b, c, d]).unwrap();
    assert_eq!(fixture.aliases(d), ["A", "B", "C"]);
    assert_eq!(fixture.aliases(b), ["A", "C", "D"]);
    assert_eq!(
        fixture.number("select count(distinct alias_group) from artist_aliases"),
        1
    );

    // Saving the same group again changes nothing but its number
    fixture.set_aliases(b, &[a, c, d]).unwrap();
    assert_eq!(fixture.aliases(a), ["B", "C", "D"]);
}

#[test]
fn leaving_a_group_leaves_the_others_together() {
    let fixture = Fixture::new();
    let [a, b, c] = ["A", "B", "C"].map(|name| fixture.artist(name, &[]));
    fixture.set_aliases(a, &[b, c]).unwrap();

    fixture.set_aliases(a, &[]).unwrap();
    assert!(fixture.aliases(a).is_empty());
    assert_eq!(fixture.aliases(b), ["C"]);
    assert_eq!(fixture.aliases(c), ["B"]);

    // A group of two loses both
    fixture.set_aliases(b, &[]).unwrap();
    assert!(fixture.aliases(c).is_empty());
    assert_eq!(fixture.number("select count(*) from artist_aliases"), 0);
}

#[test]
fn an_artist_given_without_its_own_aliases_leaves_them_behind() {
    let fixture = Fixture::new();
    let [a, b, c, d] = ["A", "B", "C", "D"].map(|name| fixture.artist(name, &[]));
    fixture.set_aliases(a, &[b]).unwrap();
    fixture.set_aliases(c, &[d]).unwrap();

    // C comes without D, which is left alone, so it loses its group
    fixture.set_aliases(a, &[b, c]).unwrap();
    assert_eq!(fixture.aliases(a), ["B", "C"]);
    assert!(fixture.aliases(d).is_empty());
    assert_eq!(fixture.number("select count(*) from artist_aliases"), 3);

    // Removing two of A's aliases leaves them aliases of each other
    fixture.set_aliases(a, &[b, c, d]).unwrap();
    fixture.set_aliases(a, &[d]).unwrap();
    assert_eq!(fixture.aliases(a), ["D"]);
    assert_eq!(fixture.aliases(b), ["C"]);
    assert_eq!(fixture.aliases(c), ["B"]);
}

#[test]
fn aliases_left_out_stay_and_wrong_ones_are_refused() {
    let fixture = Fixture::new();
    let [a, b, c] = ["A", "B", "C"].map(|name| fixture.artist(name, &[]));
    fixture.set_aliases(a, &[b]).unwrap();

    // Left out, they stay as they are
    fixture
        .update(
            a,
            ArtistInput {
                name: "A2".into(),
                ..ArtistInput::default()
            },
        )
        .unwrap();
    assert_eq!(fixture.aliases(b), ["A2"]);

    // Given twice, an alias is kept once
    fixture.set_aliases(a, &[b, c, b]).unwrap();
    assert_eq!(fixture.aliases(a), ["B", "C"]);

    assert_eq!(
        invalid(fixture.set_aliases(a, &[b, a])),
        "An artist can't be its own alias"
    );
    assert_eq!(
        not_found(fixture.set_aliases(a, &[b, 999])),
        "There's no artist 999"
    );

    // A refused save changes nothing, its rename included
    let refused = fixture.update(
        a,
        ArtistInput {
            name: "A3".into(),
            aliases: Some(vec![999]),
            ..ArtistInput::default()
        },
    );
    assert_eq!(not_found(refused), "There's no artist 999");
    assert_eq!(fixture.catalog.get_artist(a).unwrap().unwrap().name, "A2");
    assert_eq!(fixture.aliases(a), ["B", "C"]);
}

#[test]
fn deleting_an_artist_takes_it_out_of_its_group() {
    let fixture = Fixture::new();
    let [a, b, c] = ["A", "B", "C"].map(|name| fixture.artist(name, &[]));
    fixture.set_aliases(a, &[b, c]).unwrap();
    assert!(fixture.catalog.artist_blockers(a).unwrap().is_empty());

    fixture.catalog.delete_artist(a).unwrap();
    assert_eq!(fixture.aliases(b), ["C"]);

    // The last one goes with the one before it
    fixture.catalog.delete_artist(b).unwrap();
    assert!(fixture.aliases(c).is_empty());
    assert_eq!(fixture.number("select count(*) from artist_aliases"), 0);
}

#[test]
fn the_table_keeps_an_artist_in_one_group_of_two_or_more() {
    let fixture = Fixture::new();
    let [a, b, c] = ["A", "B", "C"].map(|name| fixture.artist(name, &[]));
    fixture.sql(&format!(
        "insert into artist_aliases (artist_id, alias_group) values ({a}, 1), ({b}, 1)"
    ));
    let refuse = |sql: String| {
        fixture
            .store
            .conn()
            .execute(&sql, [])
            .unwrap_err()
            .to_string()
    };
    assert_eq!(
        refuse(format!(
            "insert into artist_aliases (artist_id, alias_group) values ({a}, 2)"
        )),
        "UNIQUE constraint failed: artist_aliases.artist_id"
    );
    assert_eq!(
        refuse(format!(
            "insert into artist_aliases (artist_id, alias_group) values ({c}, 0)"
        )),
        "CHECK constraint failed: alias_group"
    );
    assert_eq!(
        refuse("insert into artist_aliases (artist_id, alias_group) values (999, 1)".into()),
        "FOREIGN KEY constraint failed"
    );

    // Deleting a row that leaves one deletes that one too
    fixture.sql(&format!("delete from artist_aliases where artist_id = {a}"));
    assert_eq!(fixture.number("select count(*) from artist_aliases"), 0);

    // And so does deleting its artist
    fixture.sql(&format!(
        "insert into artist_aliases (artist_id, alias_group) values ({b}, 3), ({c}, 3)"
    ));
    fixture.sql(&format!("delete from artists where id = {c}"));
    assert_eq!(fixture.number("select count(*) from artist_aliases"), 0);
}
