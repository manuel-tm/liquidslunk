//! What the import makes of a folder, only reading it: a release from its
//! files' tags, or else its name; a row for each audio file; and the artist
//! each artist tag names.

use std::collections::{HashMap, HashSet};

use super::{
    Artist, ArtistSuggestion, Catalog, NameMatch, Release, ReleaseSuggestion, TrackSuggestion,
};
use crate::Result;
use crate::dates::parse_date;
use crate::media::paths::basename;
use crate::media::{FileType, FolderFile};
use crate::repo::named::{self, Table};
use crate::repo::releases;
use crate::text::{clean_text, is_js_space, trim};

// How many audio files a release's suggestion reads the tags of
const TAGGED_FILES: usize = 50;

impl Catalog {
    /// What a folder suggests for a new release: the most common album,
    /// date and album artist tags of its first 50 audio files, or else the
    /// folder's name. The title is kept whole, even with " = " in it, and
    /// the releases with that title already are found by each of its parts
    /// too, so "Scenery = シーナリィ" finds "Scenery".
    pub fn suggest_release(&self, folder: &str) -> Result<ReleaseSuggestion> {
        let files = self.media.scan_folder(folder)?;
        let of_type = |wanted: FileType| -> Vec<&FolderFile> {
            files
                .iter()
                .filter(|file| file.disk.file_type == wanted)
                .collect()
        };
        let music = of_type(FileType::Music);
        let tags: Vec<_> = music
            .iter()
            .take(TAGGED_FILES)
            .map(|file| self.media.read_audio_at(&file.disk.path).tags)
            .collect();
        let album = most_common(tags.iter().map(|tags| tags.album.as_deref()));
        let date = most_common(tags.iter().map(|tags| tags.date.as_deref()));
        // Each file's album artists, or else its artists, as one value
        let joined: Vec<Option<String>> = tags
            .iter()
            .map(|tags| {
                let names = if tags.album_artists.is_empty() {
                    &tags.artists
                } else {
                    &tags.album_artists
                };
                (!names.is_empty()).then(|| names.join("\0"))
            })
            .collect();
        let names: Vec<String> = most_common(joined.iter().map(Option::as_deref))
            .map(|names| names.split('\0').map(String::from).collect())
            .unwrap_or_default();
        let normalized = self.media.normalize_path(folder);
        let title = match album {
            Some(album) => album.to_string(),
            None if normalized.is_empty() => String::new(),
            None => basename(&normalized).to_string(),
        };
        let mut titles = vec![title.clone()];
        titles.extend(equals_parts(&title));
        Ok(ReleaseSuggestion {
            released: date
                .filter(|date| parse_date(date).is_ok())
                .map(String::from),
            artists: self.suggest_artists(&names)?,
            same_title: self.releases_titled_any(&titles)?,
            music: music.len() as i64,
            images: of_type(FileType::Image).len() as i64,
            others: of_type(FileType::Other).len() as i64,
            title,
        })
    }

    /// A row for each audio file in a folder and its subfolders: disc,
    /// number and title from its tags, or else from its path, so
    /// `CD2/03 Title.flac` is disc 2, track 3, "Title". A file with no
    /// number in either is numbered by its place on its disc.
    pub fn suggest_tracks(&self, folder: &str) -> Result<Vec<TrackSuggestion>> {
        let normalized = self.media.normalize_path(folder);
        let files: Vec<FolderFile> = self
            .media
            .scan_folder(folder)?
            .into_iter()
            .filter(|file| file.disk.file_type == FileType::Music)
            .collect();
        let registered: Vec<i64> = files
            .iter()
            .filter_map(|file| file.file.as_ref().map(|file| file.id))
            .collect();
        let tracks = self.tracks_of_files(&registered)?;
        let read: Vec<_> = files
            .iter()
            .map(|file| self.media.read_audio_at(&file.disk.path))
            .collect();
        // Every row's artist tags are matched at once
        let tagged: Vec<String> = read
            .iter()
            .flat_map(|audio| audio.tags.artists.iter().cloned())
            .collect();
        let mut artists = self.suggest_artists(&tagged)?.into_iter();
        let mut numbers: HashMap<i64, i64> = HashMap::new();
        let mut rows = Vec::new();
        for (file, audio) in files.into_iter().zip(read) {
            let from_name = from_file_name(&file.disk.path, &normalized);
            let cd_number = audio.tags.disc_number.unwrap_or(from_name.cd_number);
            let position = numbers.get(&cd_number).copied().unwrap_or(0) + 1;
            numbers.insert(cd_number, position);
            rows.push(TrackSuggestion {
                track: file
                    .file
                    .as_ref()
                    .and_then(|file| tracks.get(&file.id).cloned()),
                path: file.disk.path,
                file: file.file,
                cd_number,
                disc_title: audio.tags.disc_title,
                track_number: audio
                    .tags
                    .track_number
                    .or(from_name.track_number)
                    .unwrap_or(position),
                title: audio.tags.title.unwrap_or(from_name.title),
                artists: artists.by_ref().take(audio.tags.artists.len()).collect(),
                duration_ms: audio.duration_ms,
            });
        }
        Ok(rows)
    }

    /// The artist each tag names, matching in turn: the main name exactly;
    /// an alternate name exactly, credited with it; or either one ignoring
    /// ASCII case, credited as tagged, so "ALIKA" is still printed. The
    /// first step that finds anyone decides, and finding more than one
    /// artist picks none. A tag with " = " is matched by its first part, and
    /// its other parts are offered as names of the artist it matched.
    fn suggest_artists(&self, tags: &[String]) -> Result<Vec<ArtistSuggestion>> {
        let parsed: Vec<(String, Vec<String>)> = tags
            .iter()
            .map(|tag| {
                let mut parts = equals_parts(&clean_text(tag)).into_iter();
                (parts.next().unwrap_or_default(), parts.collect())
            })
            .collect();
        let wanted: Vec<String> = parsed
            .iter()
            .map(|(name, _)| name.clone())
            .filter(|name| !name.is_empty())
            .collect();
        let (found, names) = {
            let conn = self.store.conn();
            let found = named::find_by_names(&conn, Table::Artists, &wanted)?;
            let mut ids: Vec<i64> = Vec::new();
            for found in &found {
                if !ids.contains(&found.item.id) {
                    ids.push(found.item.id);
                }
            }
            let names = named::alternate_names_of(&conn, Table::Artists, &ids)?;
            (found, names)
        };
        Ok(parsed
            .into_iter()
            .map(|(name, others)| {
                let matches: Vec<&NameMatch> = found
                    .iter()
                    .filter(|found| {
                        let matched = found
                            .alternate
                            .as_ref()
                            .map_or(&found.item.name, |a| &a.name);
                        matched.eq_ignore_ascii_case(&name)
                    })
                    .collect();
                let steps: [Vec<&NameMatch>; 3] = [
                    matches
                        .iter()
                        .copied()
                        .filter(|found| found.alternate.is_none() && found.item.name == name)
                        .collect(),
                    matches
                        .iter()
                        .copied()
                        .filter(|found| found.alternate.as_ref().is_some_and(|a| a.name == name))
                        .collect(),
                    matches,
                ];
                let step = steps.iter().position(|step| !step.is_empty());
                let mut artists: Vec<&Artist> = Vec::new();
                for found in step.map(|step| steps[step].as_slice()).unwrap_or_default() {
                    if !artists.iter().any(|artist| artist.id == found.item.id) {
                        artists.push(&found.item);
                    }
                }
                let [artist] = artists[..] else {
                    return ArtistSuggestion {
                        name,
                        artist: None,
                        credited_as: None,
                        matches: artists.len() as i64,
                        other_names: Vec::new(),
                    };
                };
                let known: HashSet<&str> = [artist.name.as_str(), name.as_str()]
                    .into_iter()
                    .chain(
                        names
                            .get(&artist.id)
                            .into_iter()
                            .flatten()
                            .map(|known| known.name.as_str()),
                    )
                    .collect();
                let other_names = others
                    .iter()
                    .filter(|other| !known.contains(other.as_str()))
                    .cloned()
                    .collect();
                let artist = artist.clone();
                ArtistSuggestion {
                    // Only a match of the main name shows the main name
                    credited_as: (step != Some(0)).then(|| name.clone()),
                    name,
                    artist: Some(artist),
                    matches: 1,
                    other_names,
                }
            })
            .collect())
    }

    /// Releases with any of these titles, main or alternate, ignoring case,
    /// each once, in the order they're found.
    fn releases_titled_any(&self, titles: &[String]) -> Result<Vec<Release>> {
        let conn = self.store.conn();
        let mut searched: Vec<&str> = Vec::new();
        let mut found: Vec<Release> = Vec::new();
        for title in titles.iter().filter(|title| !title.is_empty()) {
            if searched.contains(&title.as_str()) {
                continue;
            }
            searched.push(title);
            for release in releases::find_by_title(&conn, title)? {
                match found.iter_mut().find(|known| known.id == release.id) {
                    Some(known) => *known = release,
                    None => found.push(release),
                }
            }
        }
        Ok(found)
    }
}

/// A tag's parts around " = ", as Discogs joins a name or title in two
/// languages: "Ryo Fukui = 福居良". The "=" needs white space on both sides, since
/// it's part of titles like "E=MC²". Parts are trimmed, and empty and repeated
/// ones left out. White space is what JavaScript's `\s` matches, as slunk split
/// with it.
pub fn equals_parts(text: &str) -> Vec<String> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let at = |i: usize| chars.get(i).map_or(text.len(), |(byte, _)| *byte);
    let spaces_from = |mut i: usize| {
        while chars.get(i).is_some_and(|(_, c)| is_js_space(*c)) {
            i += 1;
        }
        i
    };
    let mut pieces: Vec<&str> = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < chars.len() {
        if !is_js_space(chars[i].1) {
            i += 1;
            continue;
        }
        // White space, "=", white space. No match starts later in this run
        // of white space if none starts at its beginning.
        let equals = spaces_from(i);
        if chars.get(equals).is_some_and(|(_, c)| *c == '=') {
            let end = spaces_from(equals + 1);
            if end > equals + 1 {
                pieces.push(&text[start..at(i)]);
                start = at(end);
                i = end;
                continue;
            }
        }
        i = equals;
    }
    pieces.push(&text[start..]);
    let mut parts: Vec<String> = Vec::new();
    for piece in pieces.into_iter().map(trim) {
        if !piece.is_empty() && !parts.iter().any(|part| part == piece) {
            parts.push(piece.to_string());
        }
    }
    parts
}

/// What a file's path says, without tags.
#[derive(Debug, PartialEq, Eq)]
struct FromName {
    cd_number: i64,
    track_number: Option<i64>,
    title: String,
}

/// `CD2/03 Title.flac` → disc 2, track 3, "Title", relative to the folder
/// imported. A folder named `cd`, `disc` or `disk` with a number, in any case,
/// gives the disc. A name that starts with up to three digits, then white
/// space, `.`, `_` or `-`, gives the number and the title. These match what
/// slunk's regular expressions matched.
fn from_file_name(path: &str, folder: &str) -> FromName {
    let relative = match path
        .strip_prefix(folder)
        .and_then(|rest| rest.strip_prefix('/'))
    {
        Some(rest) if !folder.is_empty() => rest,
        _ => path,
    };
    let (folders, file) = match relative.rfind('/') {
        Some(slash) => (&relative[..slash], &relative[slash + 1..]),
        None => (".", relative),
    };
    // The last extension goes, unless it's empty
    let name = match file.rfind('.') {
        Some(dot) if dot + 1 < file.len() => &file[..dot],
        _ => file,
    };
    let numbered = numbered(name);
    let title = numbered
        .as_ref()
        .and_then(|(_, rest)| *rest)
        .unwrap_or(name);
    FromName {
        cd_number: disc_of(&format!("{folders}/"))
            .filter(|&disc| disc != 0)
            .unwrap_or(1),
        track_number: numbered
            .and_then(|(digits, _)| digits.parse::<i64>().ok())
            .filter(|&number| number != 0),
        title: match trim(title) {
            "" => name.to_string(),
            title => title.to_string(),
        },
    }
}

/// The number of the first folder named like `CD2`, `disc 2` or `Disk2`, in
/// folders written as `a/b/`, as `(?:^|\/)(?:cd|disc|disk)\s*(\d+)\/` with
/// the `i` flag matched in JavaScript: ASCII letters in any case, then
/// white space, then ASCII digits.
fn disc_of(folders: &str) -> Option<i64> {
    let starts = std::iter::once(0).chain(folders.match_indices('/').map(|(slash, _)| slash + 1));
    for start in starts {
        let rest = &folders[start..];
        let Some(after) = ["cd", "disc", "disk"].iter().find_map(|prefix| {
            let head = rest.get(..prefix.len())?;
            head.eq_ignore_ascii_case(prefix)
                .then(|| &rest[prefix.len()..])
        }) else {
            continue;
        };
        let after = after.trim_start_matches(is_js_space);
        let digits = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits > 0 && after[digits..].starts_with('/') {
            // JavaScript's Number() of a long run of digits is huge, not an
            // error: it saturates here
            return Some(after[..digits].parse().unwrap_or(i64::MAX));
        }
    }
    None
}

/// A name's leading number, as `^(\d{1,3})(?:[\s._-]+(.*))?$` matched it in
/// JavaScript: up to three ASCII digits, then nothing, or separators and
/// the rest, which can't hold a line break. The rest is `None` when there's
/// nothing after the digits.
fn numbered(name: &str) -> Option<(&str, Option<&str>)> {
    let digits = name.len() - name.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if !(1..=3).contains(&digits) {
        return None;
    }
    let after = &name[digits..];
    if after.is_empty() {
        return Some((&name[..digits], None));
    }
    let separator = |c: char| is_js_space(c) || matches!(c, '.' | '_' | '-');
    let rest = after.trim_start_matches(separator);
    if rest.len() == after.len() || rest.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    Some((&name[..digits], Some(rest)))
}

/// The most common value, leaving out blanks. A tie goes to the one that
/// came first.
fn most_common<'a>(values: impl Iterator<Item = Option<&'a str>>) -> Option<&'a str> {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for value in values.flatten().filter(|value| !value.is_empty()) {
        match counts.iter_mut().find(|(known, _)| *known == value) {
            Some((_, count)) => *count += 1,
            None => counts.push((value, 1)),
        }
    }
    let mut best: Option<(&str, usize)> = None;
    for (value, count) in counts {
        if best.is_none_or(|(_, most)| count > most) {
            best = Some((value, count));
        }
    }
    best.map(|(value, _)| value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equals_parts_split_at_an_equals_sign_with_white_space_around_it() {
        assert_eq!(equals_parts("Ryo Fukui = 福居良"), ["Ryo Fukui", "福居良"]);
        assert_eq!(equals_parts("E=MC²"), ["E=MC²"]);
        assert_eq!(equals_parts("A  =\u{3000}B = A"), ["A", "B"]);
        // The first match takes one space after its "=", so the second "="
        // starts a part
        assert_eq!(equals_parts("A = = B"), ["A", "= B"]);
        assert_eq!(equals_parts(" = B"), ["B"]);
        assert_eq!(equals_parts("A ="), ["A ="]);
        assert_eq!(equals_parts("Scenery = "), ["Scenery"]);
        assert!(equals_parts("").is_empty());
    }

    #[test]
    fn file_names_give_a_disc_a_number_and_a_title() {
        let name = |path: &str, folder: &str| {
            let found = from_file_name(path, folder);
            (found.cd_number, found.track_number, found.title)
        };
        assert_eq!(
            name("music/A/CD2/03 Title.flac", "music/A"),
            (2, Some(3), "Title".into())
        );
        assert_eq!(
            name("Disc 3/1 - Intro.mp3", ""),
            (3, Some(1), "Intro".into())
        );
        assert_eq!(name("disk04/x.flac", ""), (4, None, "x".into()));
        // Kelvin's K isn't a k, as JavaScript matched without the u flag
        assert_eq!(name("DIS\u{212a}2/x.flac", ""), (1, None, "x".into()));
        assert_eq!(name("CD0/x.flac", ""), (1, None, "x".into()));
        assert_eq!(name("CDs/1/x.flac", ""), (1, None, "x".into()));
        // The folder imported isn't looked at
        assert_eq!(name("CD5/x.flac", "CD5"), (1, None, "x".into()));
        assert_eq!(name("1234 Title.flac", ""), (1, None, "1234 Title".into()));
        assert_eq!(name("007_Bond.flac", ""), (1, Some(7), "Bond".into()));
        assert_eq!(name("000 Zero.flac", ""), (1, None, "Zero".into()));
        assert_eq!(name("12.flac", ""), (1, Some(12), "12".into()));
        assert_eq!(name("03Title.flac", ""), (1, None, "03Title".into()));
        // Only separators after the number: the name, untrimmed
        assert_eq!(name("03 .flac", ""), (1, Some(3), "03 ".into()));
        assert_eq!(name("noext.", ""), (1, None, "noext.".into()));
        assert_eq!(name("a.b.c", ""), (1, None, "a.b".into()));
        assert_eq!(name("03 a\nb.flac", ""), (1, None, "03 a\nb".into()));
    }

    #[test]
    fn the_most_common_value_wins_and_a_tie_goes_to_the_first() {
        let values = [Some("b"), None, Some("a"), Some(""), Some("a"), Some("b")];
        assert_eq!(most_common(values.into_iter()), Some("b"));
        assert_eq!(
            most_common([Some("a"), Some("b"), Some("b")].into_iter()),
            Some("b")
        );
        assert_eq!(most_common([None, Some("")].into_iter()), None);
    }
}
