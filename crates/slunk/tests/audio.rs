//! Audio files' durations and tags, checked against what slunk read from the
//! same files. The fixtures and expected.json come from
//! `fixtures/audio/generate.ts`, which says how to make them again.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use slunk::media::audio::{Audio, AudioTags, read_audio};

/// What slunk's readAudio() returned.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Expected {
    duration_ms: Option<i64>,
    tags: ExpectedTags,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedTags {
    title: Option<String>,
    artists: Vec<String>,
    album: Option<String>,
    album_artists: Vec<String>,
    track_number: Option<i64>,
    disc_number: Option<i64>,
    disc_title: Option<String>,
    date: Option<String>,
}

impl From<Expected> for Audio {
    fn from(expected: Expected) -> Audio {
        let tags = expected.tags;
        Audio {
            duration_ms: expected.duration_ms,
            tags: AudioTags {
                title: tags.title,
                artists: tags.artists,
                album: tags.album,
                album_artists: tags.album_artists,
                track_number: tags.track_number,
                disc_number: tags.disc_number,
                disc_title: tags.disc_title,
                date: tags.date,
            },
        }
    }
}

#[test]
fn durations_and_tags_match_slunks_on_every_sample_format() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio");
    let expected: BTreeMap<String, Expected> =
        serde_json::from_str(&fs::read_to_string(fixtures.join("expected.json")).unwrap()).unwrap();
    assert!(expected.len() >= 9);
    let mismatched: Vec<String> = expected
        .into_iter()
        .filter_map(|(name, expected)| {
            let expected = Audio::from(expected);
            let read = read_audio(&fixtures.join(&name));
            (read != expected).then(|| format!("{name}:\n  slunk: {expected:?}\n  read:  {read:?}"))
        })
        .collect();
    assert!(mismatched.is_empty(), "{}", mismatched.join("\n"));
}
