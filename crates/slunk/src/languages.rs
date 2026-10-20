//! Languages of names and titles: BCP 47 tags, in the canonical form slunk
//! saved them in, which is what Node 26's `Intl.getCanonicalLocales` gives,
//! with ICU 78 and CLDR 48 (backend.md, "Languages").
//!
//! ICU4X's canonicalizer does most of it. V8 and ICU4C do a few things it
//! doesn't, which `canonical_locale()` adds: `tests/languages.rs` checks the
//! result against what Node gave, for every 2- and 3-letter code and the
//! tags around them.

use icu_locale::extensions::{transform, unicode};
use icu_locale::subtags::{language, variant};
use icu_locale::{Locale, LocaleCanonicalizer};
use icu_properties::CodePointMapData;
use icu_properties::props::Script;

use crate::text::trim;
use crate::{Error, Result};

/// A language tag as it's saved: canonical, so "JA-latn" becomes "ja-Latn", or
/// `None` when it's blank. Refuses a malformed tag, one whose language isn't 2
/// or 3 letters ("japanese" is well formed, but isn't a language), and `und`,
/// since an unknown language is `None`.
pub fn language_tag(text: Option<&str>) -> Result<Option<String>> {
    let Some(typed) = text.map(trim).filter(|typed| !typed.is_empty()) else {
        return Ok(None);
    };
    let canonical = canonical_locale(typed).ok_or_else(|| {
        Error::Invalid(format!(
            "\"{typed}\" isn't a language tag, like en, ja or ja-Latn"
        ))
    })?;
    let language = canonical.split('-').next().unwrap_or_default();
    if language == "und" {
        return Err(Error::Invalid(
            "Leave the language blank when it isn't known".into(),
        ));
    }
    if !(2..=3).contains(&language.len()) || !language.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err(Error::Invalid(format!(
            "\"{typed}\" isn't a language tag: it starts with a 2- or 3-letter code, like en or jpn"
        )));
    }
    Ok(Some(canonical))
}

/// The language a name's script gives away, for a name added as it was
/// credited: kana are only Japanese, and Hangul only Korean. Han characters
/// could be Chinese or Japanese, and Cyrillic several languages, so they give
/// none.
pub fn script_language(text: &str) -> Option<&'static str> {
    let scripts = CodePointMapData::<Script>::new();
    let has = |wanted: &[Script]| text.chars().any(|c| wanted.contains(&scripts.get(c)));
    if has(&[Script::Hiragana, Script::Katakana]) {
        Some("ja")
    } else if has(&[Script::Hangul]) {
        Some("ko")
    } else {
        None
    }
}

/// What `Intl.getCanonicalLocales(text)[0]` gives in Node 26, or `None`
/// where it throws.
pub fn canonical_locale(text: &str) -> Option<String> {
    // V8 returns a lowercase two-letter code as it is, without asking ICU,
    // unless it's deprecated or legacy: "bh" stays "bh", but "BH" is "bho"
    let two_letters = text.len() == 2 && text.bytes().all(|b| b.is_ascii_lowercase());
    if (two_letters && !["in", "iw", "ji", "jw", "mo", "sh", "tl", "no"].contains(&text))
        || text == "fil"
    {
        return Some(text.to_string());
    }
    // ICU4X's languages are 2 or 3 letters, and ICU's can be 5 to 8, like
    // "japanese": the rest is canonicalized with a placeholder, which has no
    // aliases, then the language goes back
    let lowered = text.to_ascii_lowercase();
    let first = lowered.split('-').next().unwrap_or_default();
    let long = (5..=8).contains(&first.len()) && first.bytes().all(|b| b.is_ascii_lowercase());
    let parsed = if long {
        format!("qaa{}", &lowered[first.len()..])
    } else {
        lowered.clone()
    };
    let mut locale = Locale::try_from_str(&parsed).ok()?;
    // ICU4C's alias, which ICU4X's data doesn't have
    if locale.id.language == language!("jaw") {
        locale.id.language = language!("jv");
    }
    LocaleCanonicalizer::new_extended().canonicalize(&mut locale);
    // ICU4C, after UTS #35: a POSIX variant on its own is `-u-va-posix`
    let posix = variant!("posix");
    if *locale.id.variants == [posix] {
        locale.id.variants.remove(&posix);
        let keywords = &mut locale.extensions.unicode.keywords;
        if !keywords.contains_key(&unicode::key!("va")) {
            keywords.set(unicode::key!("va"), unicode::value!("posix"));
        }
    }
    replace_deprecated_values(&mut locale);
    let mut canonical = locale.to_string();
    if long {
        canonical.replace_range(..3, first);
    }
    Some(canonical)
}

/// Deprecated values of `-u-` keywords, by key, which ICU replaces and
/// ICU4X doesn't. Node gave them: these are all it replaces among the values
/// of 3 and 4 letters, every `tz` of 5 letters, and the legacy names of up to
/// 8 that ICU's data lists.
const KEYWORD_ALIASES: &[(&str, &str, &str)] = &[
    ("ca", "islamicc", "islamic-civil"),
    ("ca", "ethiopic-amete-alem", "ethioaa"),
    ("ks", "primary", "level1"),
    ("ks", "tertiary", "level3"),
    ("ms", "imperial", "uksystem"),
    ("tz", "aqams", "aqmcm"),
    ("tz", "aukns", "auhba"),
    ("tz", "caffs", "cawnp"),
    ("tz", "camtr", "cator"),
    ("tz", "canpg", "cator"),
    ("tz", "capnt", "caiql"),
    ("tz", "cathu", "cator"),
    ("tz", "cayzf", "caedm"),
    ("tz", "cet", "bebru"),
    ("tz", "cnckg", "cnsha"),
    ("tz", "cnhrb", "cnsha"),
    ("tz", "cnkhg", "cnurc"),
    ("tz", "cst6cdt", "uschi"),
    ("tz", "cuba", "cuhav"),
    ("tz", "eet", "grath"),
    ("tz", "egypt", "egcai"),
    ("tz", "eire", "iedub"),
    ("tz", "est", "papty"),
    ("tz", "est5edt", "usnyc"),
    ("tz", "factory", "unk"),
    ("tz", "gaza", "gazastrp"),
    ("tz", "gmt0", "gmt"),
    ("tz", "hongkong", "hkhkg"),
    ("tz", "hst", "ushnl"),
    ("tz", "iceland", "isrey"),
    ("tz", "iran", "irthr"),
    ("tz", "israel", "jeruslm"),
    ("tz", "jamaica", "jmkin"),
    ("tz", "japan", "jptyo"),
    ("tz", "libya", "lytip"),
    ("tz", "met", "bebru"),
    ("tz", "mncoq", "mnuln"),
    ("tz", "mst", "usphx"),
    ("tz", "mst7mdt", "usden"),
    ("tz", "navajo", "usden"),
    ("tz", "poland", "plwaw"),
    ("tz", "portugal", "ptlis"),
    ("tz", "prc", "cnsha"),
    ("tz", "pst8pdt", "uslax"),
    ("tz", "roc", "twtpe"),
    ("tz", "rok", "krsel"),
    ("tz", "turkey", "trist"),
    ("tz", "uaozh", "uaiev"),
    ("tz", "uauzh", "uaiev"),
    ("tz", "uct", "utc"),
    ("tz", "umjon", "ushnl"),
    ("tz", "usnavajo", "usden"),
    ("tz", "wet", "ptlis"),
    ("tz", "zulu", "utc"),
];

/// Deprecated values of `-t-` fields, the same way.
const FIELD_ALIASES: &[(&str, &str, &str)] =
    &[("m0", "names", "prprname"), ("d0", "name", "charname")];

/// Replaces the deprecated values ICU replaces: those above, and `yes` for
/// any `-u-` keyword, which is `true`, and so left out.
fn replace_deprecated_values(locale: &mut Locale) {
    let keywords = &mut locale.extensions.unicode.keywords;
    let keys: Vec<unicode::Key> = keywords.iter().map(|(key, _)| *key).collect();
    for key in keys {
        let Some(value) = keywords.get_mut(&key) else {
            continue;
        };
        let text = value.to_string();
        if text == "yes" {
            *value = unicode::Value::new_empty();
        } else if let Some((_, _, to)) = KEYWORD_ALIASES
            .iter()
            .find(|(k, from, _)| *k == key.as_str() && *from == text)
        {
            *value = unicode::Value::try_from_str(to).expect("a valid value");
        }
    }
    let fields = &mut locale.extensions.transform.fields;
    for (key, from, to) in FIELD_ALIASES {
        let key = transform::Key::try_from_str(key).expect("a valid key");
        if fields
            .get(&key)
            .is_some_and(|value| value.to_string() == *from)
        {
            fields.set(
                key,
                transform::Value::try_from_str(to).expect("a valid value"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_saved_canonical_and_blank_is_none() {
        assert_eq!(
            language_tag(Some(" JA-latn ")).unwrap().as_deref(),
            Some("ja-Latn")
        );
        assert_eq!(language_tag(Some("iw")).unwrap().as_deref(), Some("he"));
        assert_eq!(language_tag(Some("jpn")).unwrap().as_deref(), Some("ja"));
        assert_eq!(language_tag(Some("  ")).unwrap(), None);
        assert_eq!(language_tag(None).unwrap(), None);
    }

    #[test]
    fn malformed_tags_und_and_long_languages_are_refused() {
        let refused = |text| language_tag(Some(text)).unwrap_err().to_string();
        assert_eq!(
            refused("ja_JP"),
            "\"ja_JP\" isn't a language tag, like en, ja or ja-Latn"
        );
        assert_eq!(
            refused(" root "),
            "\"root\" isn't a language tag, like en, ja or ja-Latn"
        );
        assert_eq!(
            refused("und"),
            "Leave the language blank when it isn't known"
        );
        assert_eq!(
            refused("und-Latn"),
            "Leave the language blank when it isn't known"
        );
        assert_eq!(
            refused("japanese"),
            "\"japanese\" isn't a language tag: it starts with a 2- or 3-letter code, like en or jpn"
        );
    }

    #[test]
    fn kana_give_japanese_and_hangul_korean() {
        assert_eq!(script_language("いとう・まさたか"), Some("ja"));
        assert_eq!(script_language("福居良トリオ"), Some("ja"));
        assert_eq!(script_language("이소라"), Some("ko"));
        assert_eq!(script_language("福居良"), None);
        assert_eq!(script_language("Алика"), None);
        assert_eq!(script_language("ALIKA"), None);
    }
}
