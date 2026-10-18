//! Language tags, checked against what Node's `Intl.getCanonicalLocales`
//! gave, which slunk saved them with (design §10). expected.json comes from
//! `fixtures/languages/generate.ts`, which says how to make it again.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use slunk::languages::{canonical_locale, language_tag};

#[derive(Debug, Deserialize)]
struct Expected {
    aliases: BTreeMap<String, Option<String>>,
    cases: Vec<(String, Option<String>)>,
}

fn expected() -> Expected {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/languages/expected.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn every_two_and_three_letter_code_is_canonical_as_in_node() {
    let Expected { aliases, .. } = expected();
    assert!(aliases.len() > 400);
    let letters = 'a'..='z';
    let mut codes = Vec::new();
    for a in letters.clone() {
        for b in letters.clone() {
            codes.push(format!("{a}{b}"));
            for c in letters.clone() {
                codes.push(format!("{a}{b}{c}"));
            }
        }
    }
    let mismatched: Vec<String> = codes
        .iter()
        .filter_map(|code| {
            let node = aliases.get(code).cloned().unwrap_or(Some(code.clone()));
            let ours = canonical_locale(code);
            (ours != node).then(|| format!("{code}: Node {node:?}, ours {ours:?}"))
        })
        .collect();
    assert!(mismatched.is_empty(), "{}", mismatched.join("\n"));
}

#[test]
fn tags_are_canonical_as_in_node() {
    let Expected { cases, .. } = expected();
    assert!(cases.len() > 400);
    let mismatched: Vec<String> = cases
        .iter()
        .filter_map(|(tag, node)| {
            let ours = canonical_locale(tag);
            (ours != *node).then(|| format!("{tag:?}: Node {node:?}, ours {ours:?}"))
        })
        .collect();
    assert!(mismatched.is_empty(), "{}", mismatched.join("\n"));
}

#[test]
fn saved_tags_are_the_canonical_ones_slunk_accepted() {
    let Expected { cases, .. } = expected();
    for (tag, node) in cases {
        let typed = tag.trim_matches(|c: char| c == ' ' || c == '\u{a0}');
        match language_tag(Some(&tag)) {
            Ok(Some(saved)) => assert_eq!(Some(saved), canonical_locale(typed), "{tag:?}"),
            Ok(None) => assert!(typed.is_empty(), "{tag:?}"),
            // Refused: Intl threw, or the language is und or isn't 2 or 3
            // letters
            Err(_) => assert!(
                node.is_none()
                    || canonical_locale(typed).is_none_or(|canonical| {
                        let language = canonical.split('-').next().unwrap().to_string();
                        language == "und" || !(2..=3).contains(&language.len())
                    }),
                "{tag:?} refused, but Node gives {node:?}"
            ),
        }
    }
}
