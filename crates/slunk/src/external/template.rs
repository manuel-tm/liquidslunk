//! Path templates, and names on disk (backend.md, "Path templates"): where a
//! folder library puts each track, worked out from its tags, so a file's name
//! and its tags always agree.

use std::collections::{HashMap, HashSet};

use unicode_normalization::UnicodeNormalization;

use super::Tags;
use crate::text::clean_text;
use crate::{Error, Result};

/// A folder library's template when none is given: `Ryo Fukui/1976 -
/// Scenery/01 It Could Happen to You.flac`. A release with no date leaves out
/// its year, and one with a single disc its disc.
pub const DEFAULT_TEMPLATE: &str =
    "${album_artist}/[${year} - ]${album}/[${disc}-]${track_number:2} ${track_title}.${format}";

/// The longest name FAT32 and exFAT take, in UTF-16 code units
const MAX_NAME: usize = 255;

/// A path template, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// What comes before its final `.${format}`, which every template ends
    /// in
    parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    /// A variable, and the digits a number is padded to
    Variable(Variable, Option<usize>),
    /// `[…]`: left out when a variable in it is empty
    Optional(Vec<Part>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variable {
    AlbumArtist,
    Artist,
    Album,
    Year,
    Date,
    CdNumber,
    Disc,
    CdCount,
    DiscTitle,
    TrackNumber,
    TrackCount,
    TrackTitle,
    Label,
    CatalogNumber,
    Format,
}

const VARIABLES: &[(&str, Variable)] = &[
    ("album_artist", Variable::AlbumArtist),
    ("artist", Variable::Artist),
    ("album", Variable::Album),
    ("year", Variable::Year),
    ("date", Variable::Date),
    ("cd_number", Variable::CdNumber),
    ("disc", Variable::Disc),
    ("cd_count", Variable::CdCount),
    ("disc_title", Variable::DiscTitle),
    ("track_number", Variable::TrackNumber),
    ("track_count", Variable::TrackCount),
    ("track_title", Variable::TrackTitle),
    ("label", Variable::Label),
    ("catalog_number", Variable::CatalogNumber),
    ("format", Variable::Format),
];

impl Variable {
    fn is_number(self) -> bool {
        matches!(
            self,
            Variable::Year
                | Variable::CdNumber
                | Variable::Disc
                | Variable::CdCount
                | Variable::TrackNumber
                | Variable::TrackCount
        )
    }

    /// Its value for a track, or `None` when it's empty.
    fn value(self, tags: &Tags, extension: &str, pad: Option<usize>) -> Option<String> {
        let number = |n: i64| match pad {
            Some(width) => format!("{n:0width$}"),
            None => n.to_string(),
        };
        match self {
            Variable::AlbumArtist => tags.album_artist.clone(),
            Variable::Artist => tags.artist.clone(),
            Variable::Album => Some(tags.album.clone()),
            Variable::Year => tags.year.map(number),
            Variable::Date => tags.date.clone(),
            Variable::CdNumber => Some(number(tags.disc_number)),
            Variable::Disc => (tags.disc_total > 1).then(|| number(tags.disc_number)),
            Variable::CdCount => Some(number(tags.disc_total)),
            Variable::DiscTitle => tags.disc_title.clone(),
            Variable::TrackNumber => Some(number(tags.track_number)),
            Variable::TrackCount => Some(number(tags.track_total)),
            Variable::TrackTitle => Some(tags.title.clone()),
            Variable::Label => tags.labels.first().cloned(),
            Variable::CatalogNumber => tags.catalog_number.clone(),
            Variable::Format => Some(extension.to_string()),
        }
    }
}

impl Template {
    /// Reads a template, cleaned like other text. Refuses a variable it
    /// doesn't know, padding a variable that isn't a number, a `${` or `[`
    /// that isn't closed, a `]` with no `[`, brackets inside brackets, a
    /// template that doesn't end in `.${format}`, and one with neither
    /// `${track_number}` nor `${track_title}`, which would give every track
    /// of a release the same name.
    pub fn parse(text: &str) -> Result<Template> {
        let text = clean_text(text);
        let mut parts: Vec<Part> = Vec::new();
        let mut group: Option<Vec<Part>> = None;
        let mut rest = text.as_str();
        while !rest.is_empty() {
            let part = if let Some(after) = rest.strip_prefix("${") {
                let end = after
                    .find('}')
                    .ok_or_else(|| invalid("The template has a ${ that isn't closed"))?;
                rest = &after[end + 1..];
                variable(&after[..end])?
            } else if let Some(after) = rest.strip_prefix('[') {
                if group.is_some() {
                    return Err(invalid("A template can't have brackets inside brackets"));
                }
                group = Some(Vec::new());
                rest = after;
                continue;
            } else if let Some(after) = rest.strip_prefix(']') {
                let inner = group
                    .take()
                    .ok_or_else(|| invalid("The template has a ] with no [ before it"))?;
                rest = after;
                Part::Optional(inner)
            } else {
                let end = [rest.find("${"), rest.find(['[', ']'])]
                    .into_iter()
                    .flatten()
                    .min()
                    .unwrap_or(rest.len());
                let (text, after) = rest.split_at(end);
                rest = after;
                Part::Text(text.to_string())
            };
            group.as_mut().unwrap_or(&mut parts).push(part);
        }
        if group.is_some() {
            return Err(invalid("The template has a [ that isn't closed"));
        }
        if !text.ends_with(".${format}") {
            return Err(invalid("The template has to end in .${format}"));
        }
        if !mentions(&parts, Variable::TrackNumber) && !mentions(&parts, Variable::TrackTitle) {
            return Err(invalid(
                "The template needs ${track_number} or ${track_title}, or every track of a release gets the same name",
            ));
        }
        // The final ".${format}" is the file's extension, which a name keeps
        // when it's cut
        parts.pop();
        if let Some(Part::Text(last)) = parts.last_mut() {
            last.pop();
            if last.is_empty() {
                parts.pop();
            }
        }
        Ok(Template { parts })
    }

    /// Where a track goes, from its tags and its file's extension, like
    /// `flac` or `m4a`.
    pub fn path(&self, tags: &Tags, extension: &str) -> TrackPath {
        let mut names = vec![String::new()];
        render(&self.parts, tags, extension, &mut names);
        let stem = names.pop().unwrap_or_default();
        TrackPath {
            folders: names,
            stem,
            extension: extension.to_string(),
        }
    }
}

/// A track's path in a folder library, as its template gives it: its folders,
/// and its file's name before its extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackPath {
    folders: Vec<String>,
    stem: String,
    extension: String,
}

impl TrackPath {
    /// The path on disk, relative to the folder library: each name made safe
    /// (`safe_name()`), joined with `/`.
    pub fn safe(&self) -> String {
        self.numbered(1)
    }

    /// Which of this path's numbered forms a path is: 1 for its own, 2 for
    /// " (2)", … `None` for a path that isn't one of them.
    pub fn number_of(&self, path: &str) -> Option<usize> {
        if self.safe() == path {
            return Some(1);
        }
        let stem = path.strip_suffix(&format!(".{}", replaced(&self.extension)))?;
        let open = stem.rfind(" (")?;
        let n: usize = stem[open + 2..].strip_suffix(')')?.parse().ok()?;
        (n >= 2 && self.numbered(n) == path).then_some(n)
    }

    /// The same, for the `n`th track with this path: from 2, " (2)", " (3)",
    /// … go before the extension.
    pub fn numbered(&self, n: usize) -> String {
        let suffix = if n > 1 {
            format!(" ({n})")
        } else {
            String::new()
        };
        let mut names: Vec<String> = self.folders.iter().map(|name| safe_name(name)).collect();
        names.push(safe_file_name(&self.stem, &suffix, &self.extension));
        names.join("/")
    }
}

/// A folder's or file's name, made safe for FAT32 and exFAT, which take
/// fewer names than macOS: in NFC; with `/ \ : * ? " < > |` and control
/// characters as `_`; without leading dots, which hide a file on macOS and
/// Linux, or trailing dots and spaces, which Windows drops; at most 255
/// UTF-16 code units; and `_` when that leaves nothing.
pub fn safe_name(name: &str) -> String {
    let name = without_trailing(&cut(&replaced(name), MAX_NAME)).to_string();
    if name.is_empty() { "_".into() } else { name }
}

/// A file's name, made safe the same way. A long one is cut before its
/// extension and its number.
fn safe_file_name(stem: &str, suffix: &str, extension: &str) -> String {
    let tail = format!("{suffix}.{}", replaced(extension));
    let room = MAX_NAME.saturating_sub(tail.encode_utf16().count());
    let stem = cut(&replaced(stem), room);
    let stem = if stem.is_empty() { "_" } else { &stem };
    format!("{stem}{tail}")
}

/// In NFC, with the characters FAT32 and exFAT refuse as `_`, and without
/// leading dots.
fn replaced(name: &str) -> String {
    let name: String = name
        .nfc()
        .map(|c| {
            if c.is_control() || r#"/\:*?"<>|"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    name.trim_start_matches('.').to_string()
}

fn without_trailing(name: &str) -> &str {
    name.trim_end_matches(['.', ' '])
}

/// At most `room` UTF-16 code units, cut at a character.
fn cut(name: &str, room: usize) -> String {
    let mut used = 0;
    name.chars()
        .take_while(|c| {
            used += c.len_utf16();
            used <= room
        })
        .collect()
}

/// Tells apart tracks whose paths are the same, comparing them ignoring case
/// and Unicode normalization, as FAT32, exFAT and APFS compare them. Each
/// path goes to its track with the lowest id, and the others get " (2)",
/// " (3)", … before the extension, skipping any path another track has. By
/// track id.
pub fn unique_paths(paths: &[(i64, TrackPath)]) -> HashMap<i64, String> {
    let mut sorted: Vec<&(i64, TrackPath)> = paths.iter().collect();
    sorted.sort_by_key(|(id, _)| *id);
    let mut taken = HashSet::new();
    let mut unique = HashMap::new();
    let mut repeated = Vec::new();
    // Every track's own path first, so a number never takes one
    for (id, path) in sorted {
        let safe = path.safe();
        if taken.insert(folded(&safe)) {
            unique.insert(*id, safe);
        } else {
            repeated.push((*id, path));
        }
    }
    for (id, path) in repeated {
        let numbered = (2..)
            .map(|n| path.numbered(n))
            .find(|numbered| taken.insert(folded(numbered)))
            .expect("a number that's free");
        unique.insert(id, numbered);
    }
    unique
}

/// A path as a disk that ignores case and Unicode normalization compares
/// it.
pub(crate) fn folded(path: &str) -> String {
    path.nfd()
        .collect::<String>()
        .to_lowercase()
        .nfd()
        .collect()
}

/// Expands parts into names, a new one at each `/` of the template's own
/// text. A value's own `/` doesn't start one: `safe_name()` replaces it, so
/// a title can't make a folder.
fn render(parts: &[Part], tags: &Tags, extension: &str, names: &mut Vec<String>) {
    for part in parts {
        match part {
            Part::Text(text) => {
                let mut pieces = text.split('/');
                if let (Some(name), Some(first)) = (names.last_mut(), pieces.next()) {
                    name.push_str(first);
                }
                names.extend(pieces.map(str::to_string));
            }
            Part::Variable(variable, pad) => {
                if let (Some(name), Some(value)) =
                    (names.last_mut(), variable.value(tags, extension, *pad))
                {
                    name.push_str(&value);
                }
            }
            Part::Optional(inner) => {
                let filled = inner.iter().all(|part| match part {
                    Part::Variable(variable, pad) => {
                        variable.value(tags, extension, *pad).is_some()
                    }
                    _ => true,
                });
                if filled {
                    render(inner, tags, extension, names);
                }
            }
        }
    }
}

/// A variable, from what's between `${` and `}`: a name, and maybe `:` and
/// the digits to pad a number to.
fn variable(text: &str) -> Result<Part> {
    let (name, pad) = match text.split_once(':') {
        Some((name, pad)) => (name, Some(pad)),
        None => (text, None),
    };
    let variable = VARIABLES
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, variable)| *variable)
        .ok_or_else(|| invalid(&format!("${{{name}}} isn't a template variable")))?;
    let pad = match pad {
        None => None,
        Some(_) if !variable.is_number() => {
            return Err(invalid(&format!(
                "Only numbers can be padded, and ${{{name}}} isn't one"
            )));
        }
        Some(digits) => Some(
            digits
                .parse::<usize>()
                .ok()
                .filter(|width| (1..=9).contains(width))
                .ok_or_else(|| {
                    invalid(&format!(
                        "Pad a number to 1 to 9 digits, like ${{{name}:2}}"
                    ))
                })?,
        ),
    };
    Ok(Part::Variable(variable, pad))
}

fn mentions(parts: &[Part], wanted: Variable) -> bool {
    parts.iter().any(|part| match part {
        Part::Variable(variable, _) => *variable == wanted,
        Part::Optional(inner) => mentions(inner, wanted),
        Part::Text(_) => false,
    })
}

fn invalid(message: &str) -> Error {
    Error::Invalid(message.to_string())
}
