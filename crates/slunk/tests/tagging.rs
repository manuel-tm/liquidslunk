//! The tagging context, on a temporary library: its rules for tag
//! definitions and their values, as docs/development/backend.md lists them
//! ("The tagging context").

use std::sync::Arc;

use rusqlite::params_from_iter;
use sea_query::{Query, SqliteQueryBuilder};
use slunk::Error;
use slunk::paging::ListArgs;
use slunk::repo::helpers::bind;
use slunk::repo::store::Store;
use slunk::repo::tags::tag_matches;
use slunk::repo::{connection, migrator};
use slunk::tagging::{
    OwnerTag, Tag, TagFilter, TagFilterInput, TagInput, TagOwner, TagType, TagUsage, TagValue,
    TagValuesInput, Tagging, ValueCount, format_value, tag_filters,
};
use tempfile::TempDir;

struct Fixture {
    _dir: TempDir,
    store: Arc<Store>,
    tagging: Tagging,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        migrator::migrate(&db, 10).unwrap();
        let store = Arc::new(Store::new(connection::open(&db).unwrap()));
        let tagging = Tagging::new(store.clone());
        Fixture {
            _dir: dir,
            store,
            tagging,
        }
    }

    fn sql(&self, sql: &str) {
        self.store.conn().execute_batch(sql).unwrap();
    }

    fn tag(&self, kind: TagType, name: &str, multiple: bool) -> Tag {
        self.tagging
            .create_tag(
                kind,
                &TagInput {
                    name: name.into(),
                    multiple,
                },
            )
            .unwrap()
    }

    /// A release, and its id.
    fn release(&self, title: &str) -> i64 {
        self.store
            .conn()
            .query_row(
                "insert into releases (title) values (?) returning id",
                [title],
                |row| row.get(0),
            )
            .unwrap()
    }

    /// A track on a release, with a file of its own, and its id.
    fn track(&self, release: i64, title: &str) -> i64 {
        let conn = self.store.conn();
        let file: i64 = conn
            .query_row(
                "insert into files (path, type, size_bytes, mtime, sha256)
                 values (? || '.flac', 'music', 0, 0, printf('%064d', 0)) returning id",
                [title],
                |row| row.get(0),
            )
            .unwrap();
        conn.query_row(
            "insert into tracks (title, track_number, release_id, file_id)
             values (?, (select count(*) + 1 from tracks), ?, ?) returning id",
            rusqlite::params![title, release, file],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn set(&self, owner: TagOwner, id: i64, tags: &[(&Tag, &[&str])]) -> slunk::Result<()> {
        self.tagging.set_tags(owner, id, &input(tags))
    }

    /// An owner's tags, as `Name: value, value`.
    fn shown(&self, owner: TagOwner, id: i64) -> Vec<String> {
        let names: Vec<Tag> = self.tagging.list_tags(&per_page(200), None).unwrap().items;
        self.tagging
            .tags_of(owner, &[id])
            .unwrap()
            .remove(&id)
            .unwrap_or_default()
            .into_iter()
            .map(
                |OwnerTag {
                     kind,
                     tag_id,
                     values,
                 }| {
                    let tag = names
                        .iter()
                        .find(|tag| tag.kind == kind && tag.id == tag_id)
                        .unwrap();
                    let values: Vec<String> = values.iter().map(format_value).collect();
                    format!("{}: {}", tag.name, values.join(", "))
                },
            )
            .collect()
    }

    /// The releases a list's tag filters keep, by id.
    fn filtered(&self, filters: &[TagFilter]) -> Vec<i64> {
        let mut select = Query::select();
        select.column(("releases", "id")).from("releases");
        for filter in filters {
            select.and_where(tag_matches(TagOwner::Release, filter));
        }
        let (sql, values) = select
            .order_by(("releases", "id"), sea_query::Order::Asc)
            .build(SqliteQueryBuilder);
        let conn = self.store.conn();
        let mut statement = conn.prepare(&sql).unwrap();
        statement
            .query_map(params_from_iter(bind(values)), |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }
}

fn input(tags: &[(&Tag, &[&str])]) -> Vec<TagValuesInput> {
    tags.iter()
        .map(|(tag, values)| TagValuesInput {
            kind: tag.kind,
            tag_id: tag.id,
            values: values.iter().map(|&value| value.into()).collect(),
        })
        .collect()
}

fn per_page(per_page: i64) -> ListArgs {
    ListArgs {
        per_page: Some(per_page),
        ..ListArgs::default()
    }
}

fn names(tags: &[Tag]) -> Vec<&str> {
    tags.iter().map(|tag| tag.name.as_str()).collect()
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

fn sqlite(result: slunk::Result<impl std::fmt::Debug>) -> String {
    match result {
        Err(Error::Sqlite(error)) => error.to_string(),
        other => panic!("expected an SQLite error, got {other:?}"),
    }
}

fn filter(kind: TagType, tag_id: i64, parts: &[(&str, &str)]) -> TagFilterInput {
    let part = |name: &str| {
        parts
            .iter()
            .find(|(part, _)| *part == name)
            .map(|(_, value)| value.to_string())
    };
    TagFilterInput {
        kind,
        tag_id,
        equals: part("equals"),
        contains: part("contains"),
        min: part("min"),
        max: part("max"),
    }
}

#[test]
fn tags_of_every_type_list_together_sort_by_name_and_search() {
    let fixture = Fixture::new();
    let tagging = &fixture.tagging;
    fixture.tag(TagType::Text, "Genre", true);
    fixture.tag(TagType::Number, "Score", false);
    fixture.tag(TagType::Date, "Bought", false);
    fixture.tag(TagType::Text, "mood", true);
    let list = |sort: Option<&str>, q: Option<&str>, kind: Option<TagType>| {
        let args = ListArgs {
            sort: sort.map(Into::into),
            q: q.map(Into::into),
            ..ListArgs::default()
        };
        tagging.list_tags(&args, kind).unwrap()
    };

    // Newest first by default, and ids that tie go text, number, date
    let all = list(None, None, None);
    assert_eq!(all.total, 4);
    assert_eq!(names(&all.items), ["mood", "Genre", "Score", "Bought"]);
    assert_eq!(
        names(&list(Some("id"), None, None).items),
        ["Genre", "Score", "Bought", "mood"]
    );
    // By name ignoring case
    assert_eq!(
        names(&list(Some("name"), None, None).items),
        ["Bought", "Genre", "mood", "Score"]
    );
    assert_eq!(
        names(&list(Some("-name"), Some("O"), None).items),
        ["Score", "mood", "Bought"]
    );
    let texts = list(Some("name"), None, Some(TagType::Text));
    assert_eq!(
        (names(&texts.items), texts.total),
        (vec!["Genre", "mood"], 2)
    );
    let paged = tagging
        .list_tags(
            &ListArgs {
                page: Some(2),
                per_page: Some(3),
                sort: Some("name".into()),
                ..ListArgs::default()
            },
            None,
        )
        .unwrap();
    assert_eq!((names(&paged.items), paged.total), (vec!["Score"], 4));
    assert_eq!(
        invalid(tagging.list_tags(
            &ListArgs {
                sort: Some("multiple".into()),
                ..ListArgs::default()
            },
            None
        )),
        "Tags can't be sorted by multiple"
    );
}

#[test]
fn names_are_unique_across_types_ignoring_case() {
    let fixture = Fixture::new();
    let tagging = &fixture.tagging;
    let score = fixture.tag(TagType::Number, " Score ", false);
    assert_eq!(score.name, "Score");
    let create = |kind, name: &str| {
        tagging.create_tag(
            kind,
            &TagInput {
                name: name.into(),
                multiple: false,
            },
        )
    };
    assert_eq!(
        invalid(create(TagType::Text, "score")),
        "There's already a number tag named Score"
    );
    // The database refuses the same name in one type
    assert_eq!(
        sqlite(create(TagType::Number, "SCORE")),
        "UNIQUE constraint failed: real_number_tags.name"
    );
    assert!(sqlite(create(TagType::Date, " ")).starts_with("CHECK constraint failed"));

    let genre = fixture.tag(TagType::Text, "Genre", true);
    let rename = |tag: &Tag, name: &str| {
        tagging.update_tag(
            tag.kind,
            tag.id,
            &TagInput {
                name: name.into(),
                multiple: tag.multiple,
            },
        )
    };
    assert_eq!(
        invalid(rename(&genre, "SCORE")),
        "There's already a number tag named Score"
    );
    // Its own name in another case is fine
    assert_eq!(rename(&score, "score").unwrap().name, "score");
    assert_eq!(
        not_found(tagging.update_tag(
            TagType::Date,
            99,
            &TagInput {
                name: "Bought".into(),
                multiple: false
            }
        )),
        "There's no date tag 99"
    );
}

#[test]
fn switching_to_single_fails_while_an_owner_has_several_values() {
    let fixture = Fixture::new();
    let tagging = &fixture.tagging;
    let genre = fixture.tag(TagType::Text, "Genre", true);
    let (first, second) = (fixture.release("First"), fixture.release("Second"));
    let track = fixture.track(first, "Song");
    fixture
        .set(TagOwner::Release, first, &[(&genre, &["Jazz", "Rock"])])
        .unwrap();
    fixture
        .set(TagOwner::Release, second, &[(&genre, &["Pop", "Rock"])])
        .unwrap();
    fixture
        .set(TagOwner::Track, track, &[(&genre, &["Bebop", "Jazz"])])
        .unwrap();
    let single = |name: &str| {
        tagging.update_tag(
            TagType::Text,
            genre.id,
            &TagInput {
                name: name.into(),
                multiple: false,
            },
        )
    };
    assert_eq!(
        invalid(single("Genre")),
        "2 releases and 1 track have several values for this tag. Leave one value on each before making it single-value."
    );
    let owners = |owner| {
        let page = tagging
            .multi_value_owners(owner, TagType::Text, genre.id, &ListArgs::default())
            .unwrap();
        (page.items, page.total)
    };
    assert_eq!(owners(TagOwner::Release), (vec![first, second], 2));
    assert_eq!(owners(TagOwner::Track), (vec![track], 1));

    fixture
        .set(TagOwner::Release, first, &[(&genre, &["Jazz"])])
        .unwrap();
    fixture
        .set(TagOwner::Release, second, &[(&genre, &["Pop"])])
        .unwrap();
    assert_eq!(
        invalid(single("Genre")),
        "1 track has several values for this tag. Leave one value on each before making it single-value."
    );
    // The database refuses it too
    let refused = fixture.store.conn().execute_batch(&format!(
        "update text_tags set multiple = 0 where id = {}",
        genre.id
    ));
    assert!(
        refused
            .unwrap_err()
            .to_string()
            .contains("some releases or tracks have several values for this tag")
    );

    fixture.sql(&format!(
        "delete from track_text_tags where track_id = {track} and value = 'Bebop'"
    ));
    let renamed = single("Style").unwrap();
    assert_eq!((renamed.name.as_str(), renamed.multiple), ("Style", false));
}

#[test]
fn a_tag_in_use_is_not_deleted() {
    let fixture = Fixture::new();
    let tagging = &fixture.tagging;
    let genre = fixture.tag(TagType::Text, "Genre", true);
    let release = fixture.release("Release");
    let track = fixture.track(release, "Song");
    fixture
        .set(TagOwner::Release, release, &[(&genre, &["Jazz", "Rock"])])
        .unwrap();
    fixture
        .set(TagOwner::Track, track, &[(&genre, &["Jazz"])])
        .unwrap();
    assert_eq!(
        tagging.tag_usage(TagType::Text, genre.id).unwrap(),
        TagUsage {
            releases: 1,
            tracks: 1
        }
    );
    assert_eq!(
        tagging.tag_blockers(TagType::Text, genre.id).unwrap(),
        ["Set on 1 release and 1 track"]
    );
    assert_eq!(
        invalid(tagging.delete_tag(TagType::Text, genre.id)),
        "The tag is set on 1 release and 1 track"
    );

    fixture.set(TagOwner::Track, track, &[]).unwrap();
    assert_eq!(
        invalid(tagging.delete_tag(TagType::Text, genre.id)),
        "The tag is set on 1 release"
    );
    fixture.set(TagOwner::Release, release, &[]).unwrap();
    assert!(
        tagging
            .tag_blockers(TagType::Text, genre.id)
            .unwrap()
            .is_empty()
    );
    tagging.delete_tag(TagType::Text, genre.id).unwrap();
    assert_eq!(tagging.get_tag(TagType::Text, genre.id).unwrap(), None);
    // An unknown id deletes nothing
    tagging.delete_tag(TagType::Date, 99).unwrap();
}

#[test]
fn values_are_typed_as_text_and_stored_by_type() {
    let fixture = Fixture::new();
    let genre = fixture.tag(TagType::Text, "Genre", true);
    let score = fixture.tag(TagType::Number, "Score", true);
    let bought = fixture.tag(TagType::Date, "Bought", true);
    let release = fixture.release("Release");

    fixture
        .set(
            TagOwner::Release,
            release,
            &[
                (&genre, &[" Rock ", "Cafe\u{301}"]),
                (&score, &["8.50", "1e3", "-.5", "1e21"]),
                (&bought, &["2006", "2006-02", " 2006-03-10 "]),
            ],
        )
        .unwrap();
    assert_eq!(
        fixture.shown(TagOwner::Release, release),
        [
            "Bought: 2006-01-01, 2006-02-01, 2006-03-10",
            "Genre: Rock, Caf\u{e9}",
            "Score: 8.5, 1000, -0.5, 1e+21",
        ]
    );
    // Dates are stored at midnight UTC on their first day
    assert_eq!(
        fixture
            .store
            .conn()
            .query_row(
                "select group_concat(value) from release_date_tags",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "1136073600,1138752000,1141948800"
    );

    let refused = |tag: &Tag, value: &str| {
        invalid(fixture.set(TagOwner::Release, release, &[(tag, &[value])]))
    };
    assert_eq!(refused(&score, "abc"), "Score: Not a number: abc");
    assert_eq!(refused(&score, "1e400"), "Score: Not a number: 1e400");
    assert_eq!(
        refused(&bought, "06"),
        "Bought: Not a date: 06. Write it as 2006, 2006-01 or 2006-01-10."
    );
    assert_eq!(
        refused(&bought, "2006-02-30"),
        "Bought: There's no such date: 2006-02-30"
    );
    // Nothing changed
    assert_eq!(fixture.shown(TagOwner::Release, release).len(), 3);
}

#[test]
fn an_owners_values_are_saved_all_at_once() {
    let fixture = Fixture::new();
    let tagging = &fixture.tagging;
    let genre = fixture.tag(TagType::Text, "genre", true);
    let score = fixture.tag(TagType::Number, "Score", false);
    let mood = fixture.tag(TagType::Text, "Mood", true);
    let release = fixture.release("Release");

    // Blanks left out, a repeat kept once, ignoring case, and the order kept
    fixture
        .set(
            TagOwner::Release,
            release,
            &[
                (&score, &["9", " "]),
                (&genre, &["Rock", "Jazz", "ROCK", "", "jazz"]),
                (&mood, &["  "]),
                (&genre, &["Bebop", "rock"]),
            ],
        )
        .unwrap();
    // By name, ignoring case
    assert_eq!(
        fixture.shown(TagOwner::Release, release),
        ["genre: Rock, Jazz, Bebop", "Score: 9"]
    );
    // A number given twice is one value
    fixture
        .set(TagOwner::Release, release, &[(&score, &["9", "9.0", "+9"])])
        .unwrap();
    assert_eq!(fixture.shown(TagOwner::Release, release), ["Score: 9"]);

    assert_eq!(
        invalid(fixture.set(TagOwner::Release, release, &[(&score, &["8", "9"])])),
        "Score takes one value"
    );
    let unknown = TagValuesInput {
        kind: TagType::Text,
        tag_id: 99,
        values: vec![],
    };
    assert_eq!(
        not_found(tagging.set_tags(TagOwner::Release, release, &[unknown])),
        "There's no text tag 99"
    );
    // A bad value is found before a tag with two values
    assert_eq!(
        invalid(fixture.set(
            TagOwner::Release,
            release,
            &[(&score, &["8", "9"]), (&genre, &["Rock"]), (&score, &["x"])]
        )),
        "Score: Not a number: x"
    );
    assert_eq!(
        not_found(tagging.set_tags(TagOwner::Release, 99, &[])),
        "There's no release 99"
    );
    assert_eq!(
        not_found(tagging.set_tags(TagOwner::Track, 99, &[])),
        "There's no track 99"
    );

    // Saving none removes them all
    fixture.set(TagOwner::Release, release, &[]).unwrap();
    assert!(fixture.shown(TagOwner::Release, release).is_empty());
    assert!(
        tagging
            .tags_of(TagOwner::Release, &[release])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn values_read_back_by_tag_name_then_type() {
    let fixture = Fixture::new();
    let tagging = &fixture.tagging;
    let text = fixture.tag(TagType::Text, "Same", false);
    let number = fixture.tag(TagType::Number, "Another", false);
    // Only the app refuses a name another type has
    fixture.sql("insert into date_tags (name) values ('same')");
    let date = tagging.get_tag(TagType::Date, 1).unwrap().unwrap();
    let release = fixture.release("Release");
    fixture
        .set(
            TagOwner::Release,
            release,
            &[(&date, &["2020"]), (&text, &["x"]), (&number, &["1"])],
        )
        .unwrap();
    let tags = tagging.tags_of(TagOwner::Release, &[release]).unwrap();
    assert_eq!(
        tags[&release],
        [
            OwnerTag {
                kind: TagType::Number,
                tag_id: number.id,
                values: vec![TagValue::Number(1.0)]
            },
            OwnerTag {
                kind: TagType::Text,
                tag_id: text.id,
                values: vec![TagValue::Text("x".into())]
            },
            OwnerTag {
                kind: TagType::Date,
                tag_id: date.id,
                values: vec![TagValue::Date(1_577_836_800)]
            },
        ]
    );
}

#[test]
fn text_values_are_counted_the_most_used_first() {
    let fixture = Fixture::new();
    let tagging = &fixture.tagging;
    let genre = fixture.tag(TagType::Text, "Genre", true);
    let (one, two, three) = (
        fixture.release("One"),
        fixture.release("Two"),
        fixture.release("Three"),
    );
    let track = fixture.track(one, "Song");
    fixture
        .set(TagOwner::Release, one, &[(&genre, &["Rock", "Jazz"])])
        .unwrap();
    fixture
        .set(TagOwner::Release, two, &[(&genre, &["rock", "Bebop"])])
        .unwrap();
    fixture
        .set(TagOwner::Release, three, &[(&genre, &["Jazz"])])
        .unwrap();
    fixture
        .set(TagOwner::Track, track, &[(&genre, &["ROCK"])])
        .unwrap();

    let values = |args: ListArgs| {
        let page = tagging.tag_values(genre.id, &args).unwrap();
        let items: Vec<(String, i64, i64)> = page
            .items
            .into_iter()
            .map(
                |ValueCount {
                     value,
                     releases,
                     tracks,
                 }| (value.to_ascii_lowercase(), releases, tracks),
            )
            .collect();
        (items, page.total)
    };
    let pair = |value: &str, releases, tracks| (value.to_string(), releases, tracks);
    // "Rock", "rock" and "ROCK" count as one
    assert_eq!(
        values(ListArgs::default()),
        (
            vec![pair("rock", 2, 1), pair("jazz", 2, 0), pair("bebop", 1, 0)],
            3
        )
    );
    assert_eq!(
        values(ListArgs {
            q: Some("O".into()),
            ..ListArgs::default()
        }),
        (vec![pair("rock", 2, 1), pair("bebop", 1, 0)], 2)
    );
    assert_eq!(
        values(ListArgs {
            page: Some(2),
            per_page: Some(1),
            ..ListArgs::default()
        }),
        (vec![pair("jazz", 2, 0)], 3)
    );
    assert_eq!(values(ListArgs::default()).1, 3);
    assert_eq!(
        tagging.tag_values(99, &ListArgs::default()).unwrap().total,
        0
    );
}

#[test]
fn list_filters_are_checked_and_turned_into_ranges() {
    let fixture = Fixture::new();
    let genre = fixture.tag(TagType::Text, "Genre", true);
    let score = fixture.tag(TagType::Number, "Score", false);
    let bought = fixture.tag(TagType::Date, "Bought", false);
    let (one, two, three) = (
        fixture.release("One"),
        fixture.release("Two"),
        fixture.release("Three"),
    );
    fixture
        .set(
            TagOwner::Release,
            one,
            &[
                (&genre, &["Rock", "Jazz"]),
                (&score, &["8"]),
                (&bought, &["2024-03-31"]),
            ],
        )
        .unwrap();
    fixture
        .set(
            TagOwner::Release,
            two,
            &[
                (&genre, &["Hard Rock"]),
                (&score, &["9.5"]),
                (&bought, &["2024-04-01"]),
            ],
        )
        .unwrap();
    fixture
        .set(
            TagOwner::Release,
            three,
            &[(&score, &["6"]), (&bought, &["2023-12-31"])],
        )
        .unwrap();
    let keeps = |filters: &[TagFilterInput]| fixture.filtered(&tag_filters(filters).unwrap());
    let text = |parts: &[(&str, &str)]| filter(TagType::Text, genre.id, parts);
    let number = |parts: &[(&str, &str)]| filter(TagType::Number, score.id, parts);
    let date = |parts: &[(&str, &str)]| filter(TagType::Date, bought.id, parts);

    // Text: the value, or part of it, ignoring case
    assert_eq!(keeps(&[text(&[("equals", " rock ")])]), [one]);
    assert_eq!(keeps(&[text(&[("contains", "ROCK")])]), [one, two]);
    // Several filters all match, even on the same tag
    assert_eq!(
        keeps(&[text(&[("contains", "rock")]), text(&[("equals", "jazz")])]),
        [one]
    );
    // Numbers: a value, or a range with both ends included
    assert_eq!(keeps(&[number(&[("equals", "8.0")])]), [one]);
    assert_eq!(
        keeps(&[number(&[("min", "8"), ("max", "9.5")])]),
        [one, two]
    );
    assert_eq!(keeps(&[number(&[("max", "8")])]), [one, three]);
    // Dates: a whole year, month or day
    assert_eq!(keeps(&[date(&[("equals", "2024")])]), [one, two]);
    assert_eq!(keeps(&[date(&[("equals", "2024-03")])]), [one]);
    assert_eq!(keeps(&[date(&[("max", "2024-03")])]), [one, three]);
    assert_eq!(keeps(&[date(&[("min", "2024-04-01")])]), [two]);
    assert_eq!(
        tag_filters(&[date(&[("equals", "2024")])]).unwrap()[0],
        TagFilter {
            kind: TagType::Date,
            tag_id: bought.id,
            equals: None,
            contains: None,
            min: Some(TagValue::Date(1_704_067_200)),
            max: Some(TagValue::Date(1_735_603_200)),
        }
    );

    let refused = |filter: TagFilterInput| invalid(tag_filters(&[filter]));
    for parts in [
        &[][..],
        &[("equals", "Rock"), ("contains", "o")],
        &[("equals", "Rock"), ("min", "a")],
        &[("equals", "  ")],
    ] {
        assert_eq!(
            refused(text(parts)),
            "A text tag filter takes a value, or part of one"
        );
    }
    for parts in [
        &[][..],
        &[("contains", "8")],
        &[("equals", "8"), ("max", "9")],
    ] {
        assert_eq!(
            refused(number(parts)),
            "A number tag filter takes a value, or a range"
        );
    }
    assert_eq!(
        refused(date(&[("equals", "2024"), ("min", "2023")])),
        "A date tag filter takes a value, or a range"
    );
    assert_eq!(refused(number(&[("min", "x")])), "Not a number: x");
    assert_eq!(
        refused(date(&[("max", "2024-13")])),
        "There's no such date: 2024-13"
    );
}

#[test]
fn bulk_edits_change_one_tag_on_many_releases() {
    let fixture = Fixture::new();
    let tagging = &fixture.tagging;
    let genre = fixture.tag(TagType::Text, "Genre", true);
    let score = fixture.tag(TagType::Number, "Score", false);
    let (one, two, three) = (
        fixture.release("One"),
        fixture.release("Two"),
        fixture.release("Three"),
    );
    fixture
        .set(
            TagOwner::Release,
            one,
            &[(&genre, &["rock"]), (&score, &["7"])],
        )
        .unwrap();
    fixture
        .set(TagOwner::Release, two, &[(&genre, &["Jazz"])])
        .unwrap();
    let ids = [one, two, three, 99];

    // A multi-value tag gets the value added, except where it is already
    assert_eq!(
        tagging
            .set_tag_on_releases(&ids, TagType::Text, genre.id, " Rock ")
            .unwrap(),
        2
    );
    // A single-value tag's value is replaced
    assert_eq!(
        tagging
            .set_tag_on_releases(&ids, TagType::Number, score.id, "8")
            .unwrap(),
        3
    );
    assert_eq!(
        fixture.shown(TagOwner::Release, one),
        ["Genre: rock", "Score: 8"]
    );
    assert_eq!(
        fixture.shown(TagOwner::Release, two),
        ["Genre: Jazz, Rock", "Score: 8"]
    );
    assert_eq!(
        fixture.shown(TagOwner::Release, three),
        ["Genre: Rock", "Score: 8"]
    );
    assert_eq!(
        tagging
            .set_tag_on_releases(&ids, TagType::Number, score.id, "8")
            .unwrap(),
        0
    );
    assert_eq!(
        invalid(tagging.set_tag_on_releases(&ids, TagType::Number, score.id, " ")),
        "Type a value to set"
    );
    assert_eq!(
        invalid(tagging.set_tag_on_releases(&ids, TagType::Number, score.id, "x")),
        "Score: Not a number: x"
    );
    assert_eq!(
        not_found(tagging.set_tag_on_releases(&ids, TagType::Date, 99, "2020")),
        "There's no date tag 99"
    );

    // One value, ignoring case, or every value
    assert_eq!(
        tagging
            .remove_tag_from_releases(&ids, TagType::Text, genre.id, Some("ROCK"))
            .unwrap(),
        3
    );
    assert_eq!(
        fixture.shown(TagOwner::Release, two),
        ["Genre: Jazz", "Score: 8"]
    );
    assert_eq!(
        tagging
            .remove_tag_from_releases(&ids, TagType::Number, score.id, None)
            .unwrap(),
        3
    );
    assert_eq!(fixture.shown(TagOwner::Release, one), Vec::<String>::new());
    assert_eq!(
        tagging
            .remove_tag_from_releases(&ids, TagType::Text, genre.id, Some(" "))
            .unwrap(),
        1
    );
}
