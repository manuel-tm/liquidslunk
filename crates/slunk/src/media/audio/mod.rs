//! An audio file's duration and embedded tags, as suggestions only. It only
//! reads the file. A file it can't read has no duration and no tags.
//!
//! slunk read them with music-metadata, and liquidslunk with lofty (design
//! §5.3). lofty reads each format's own tags, and `Collector` picks the
//! values from them as music-metadata did: which keys map to what, which tag
//! wins when a file has several, and how lists and numbers are read.
//!
//! Durations are lofty's, in whole milliseconds, except where lofty works
//! them out differently from music-metadata: `durations` does those as
//! music-metadata did.

use std::path::Path;
use std::time::Duration;

use lofty::ape::ApeTag;
use lofty::config::ParseOptions;
use lofty::file::{AudioFile, FileType};
use lofty::id3::v1::Id3v1Tag;
use lofty::id3::v2::{Frame, Id3v2Tag, Id3v2Version};
use lofty::iff::aiff::AiffTextChunks;
use lofty::iff::wav::RiffInfoList;
use lofty::mp4::{AtomData, AtomIdent, Ilst};
use lofty::ogg::tag::VorbisComments;
use lofty::probe::Probe;
use lofty::properties::FileProperties;
use lofty::tag::ItemValue;

use crate::text::{is_js_space, trim};

mod durations;

/// What an audio file says about itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Audio {
    pub duration_ms: Option<i64>,
    pub tags: AudioTags,
}

/// What an audio file's embedded tags say, as suggestions only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioTags {
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub album_artists: Vec<String>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    /// The disc's own title: DISCSUBTITLE, or TSST in MP3s
    pub disc_title: Option<String>,
    /// `2006`, `2006-01` or `2006-01-10`
    pub date: Option<String>,
}

/// Reads an audio file's duration and embedded tags.
pub fn read_audio(path: &Path) -> Audio {
    read(path).unwrap_or_default()
}

fn read(path: &Path) -> Option<Audio> {
    let options = ParseOptions::new()
        .read_cover_art(false)
        // Keeps ID3v2.3's frames as they are, as music-metadata read them:
        // otherwise lofty renames TYER to TDRC, and adds TDAT and TIME to it
        .implicit_conversions(false);
    let probe = Probe::open(path)
        .ok()?
        .options(options)
        .guess_file_type()
        .ok()?;
    let file_type = probe.file_type()?;
    let mut reader = probe.into_inner();
    let mut tags = Collector::default();

    // Each format's tags, in the order music-metadata read them
    let duration_ms = match file_type {
        FileType::Flac => {
            let file = lofty::flac::FlacFile::read_from(&mut reader, options).ok()?;
            tags.id3v2(file.id3v2());
            tags.vorbis(file.vorbis_comments());
            durations::flac(path)
        }
        FileType::Aiff => {
            let file = lofty::iff::aiff::AiffFile::read_from(&mut reader, options).ok()?;
            tags.aiff_text(file.text_chunks());
            tags.id3v2(file.id3v2());
            durations::aiff(path)
        }
        FileType::Mpeg => {
            let file = lofty::mpeg::MpegFile::read_from(&mut reader, options).ok()?;
            tags.id3v2(file.id3v2());
            tags.ape(file.ape());
            tags.id3v1(file.id3v1());
            durations::mpeg(path)
        }
        FileType::Aac => {
            let file = lofty::aac::AacFile::read_from(&mut reader, options).ok()?;
            tags.id3v2(file.id3v2());
            tags.id3v1(file.id3v1());
            durations::mpeg(path)
        }
        FileType::Mp4 => {
            let file = lofty::mp4::Mp4File::read_from(&mut reader, options).ok()?;
            tags.ilst(file.ilst());
            milliseconds(file.properties().duration())
        }
        FileType::Wav => {
            let file = lofty::iff::wav::WavFile::read_from(&mut reader, options).ok()?;
            tags.id3v2(file.id3v2());
            // music-metadata held RIFF INFO back until the end of the file
            tags.riff_info(file.riff_info());
            milliseconds(file.properties().duration())
        }
        FileType::Ape => {
            let file = lofty::ape::ApeFile::read_from(&mut reader, options).ok()?;
            tags.ape(file.ape());
            tags.id3v1(file.id3v1());
            milliseconds(file.properties().duration())
        }
        FileType::WavPack => {
            let file = lofty::wavpack::WavPackFile::read_from(&mut reader, options).ok()?;
            tags.ape(file.ape());
            tags.id3v1(file.id3v1());
            milliseconds(file.properties().duration())
        }
        FileType::Mpc => {
            let file = lofty::musepack::MpcFile::read_from(&mut reader, options).ok()?;
            tags.ape(file.ape());
            tags.id3v1(file.id3v1());
            milliseconds(FileProperties::from(file.properties().clone()).duration())
        }
        FileType::Vorbis => {
            let file = lofty::ogg::VorbisFile::read_from(&mut reader, options).ok()?;
            tags.vorbis(Some(file.vorbis_comments()));
            milliseconds(file.properties().duration())
        }
        FileType::Opus => {
            let file = lofty::ogg::OpusFile::read_from(&mut reader, options).ok()?;
            tags.vorbis(Some(file.vorbis_comments()));
            milliseconds(file.properties().duration())
        }
        FileType::Speex => {
            let file = lofty::ogg::SpeexFile::read_from(&mut reader, options).ok()?;
            tags.vorbis(Some(file.vorbis_comments()));
            milliseconds(file.properties().duration())
        }
        _ => return None,
    };
    Some(Audio {
        duration_ms,
        tags: tags.finish(),
    })
}

/// lofty's duration. It's zero when lofty couldn't work it out.
fn milliseconds(duration: Duration) -> Option<i64> {
    (!duration.is_zero()).then_some(duration.as_millis() as i64)
}

/// music-metadata's tag types, by priority: the lower number wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Ape = 2,
    Vorbis = 3,
    Id3v24 = 4,
    Id3v23 = 5,
    Id3v22 = 6,
    /// RIFF INFO
    Exif = 7,
    /// MP4
    ITunes = 9,
    Aiff = 10,
    Id3v1 = 12,
    /// Filled in from another tag
    Artificial = 500,
}

impl Origin {
    fn priority(self) -> u32 {
        self as u32
    }
}

/// The common tags slunk read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Common {
    Title,
    Artist,
    Artists,
    AlbumArtist,
    AlbumArtists,
    Album,
    Track,
    Disk,
    DiscSubtitle,
    Date,
    Year,
}

/// A single value, and the priority of the tag it came from.
#[derive(Debug, Clone)]
struct Single {
    value: String,
    priority: u32,
}

/// A list of values, each once, and the priority of the tag they came from.
#[derive(Debug, Clone)]
struct List {
    values: Vec<String>,
    priority: u32,
}

/// music-metadata's MetadataCollector, for the common tags slunk read.
#[derive(Debug, Default)]
struct Collector {
    title: Option<Single>,
    artist: Option<Single>,
    album_artist: Option<Single>,
    album: Option<Single>,
    disc_subtitle: Option<Single>,
    date: Option<Single>,
    artists: Option<List>,
    album_artists: Option<List>,
    /// `common.year`, which a date's first four digits set too, as a number
    year: Option<i64>,
    year_priority: Option<u32>,
    track: Option<i64>,
    disk: Option<i64>,
}

impl Collector {
    fn vorbis(&mut self, comments: Option<&VorbisComments>) {
        for (key, value) in comments.into_iter().flat_map(VorbisComments::items) {
            // music-metadata uppercased the keys as it read them
            if let Some(id) = vorbis_common(&key.to_uppercase()) {
                self.add(Origin::Vorbis, id, value);
            }
        }
    }

    fn id3v2(&mut self, tag: Option<&Id3v2Tag>) {
        let Some(tag) = tag else { return };
        let version = tag.original_version();
        let origin = match version {
            Id3v2Version::V2 => Origin::Id3v22,
            Id3v2Version::V3 => Origin::Id3v23,
            _ => Origin::Id3v24,
        };
        for frame in tag {
            let (key, values) = match frame {
                Frame::Text(frame) => {
                    let id = frame.id().as_str();
                    (id.to_string(), id3v2_values(version, id, &frame.value))
                }
                Frame::UserText(frame) => (
                    format!("TXXX:{}", frame.description),
                    id3v2_values(version, "TXXX", &frame.content),
                ),
                Frame::Timestamp(frame) => (
                    frame.id().as_str().to_string(),
                    vec![frame.timestamp.to_string()],
                ),
                _ => continue,
            };
            if let Some(id) = id3v2_common(version, &key.to_uppercase()) {
                for value in values {
                    self.add(origin, id, &value);
                }
            }
        }
    }

    fn id3v1(&mut self, tag: Option<&Id3v1Tag>) {
        let Some(tag) = tag else { return };
        let mut add = |id, value: Option<String>| {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                self.add(Origin::Id3v1, id, &value);
            }
        };
        add(Common::Title, tag.title.clone());
        add(Common::Artist, tag.artist.clone());
        add(Common::Album, tag.album.clone());
        add(
            Common::Track,
            tag.track_number
                .filter(|track| *track > 0)
                .map(|track| track.to_string()),
        );
        add(Common::Year, tag.year.map(|year| year.to_string()));
    }

    fn ape(&mut self, tag: Option<&ApeTag>) {
        for item in tag.into_iter().flatten() {
            let ItemValue::Text(text) = item.value() else {
                continue;
            };
            if let Some(id) = ape_common(&item.key().to_uppercase()) {
                for value in text.split('\0') {
                    self.add(Origin::Ape, id, value);
                }
            }
        }
    }

    fn ilst(&mut self, ilst: Option<&Ilst>) {
        for atom in ilst.into_iter().flatten() {
            let key = match atom.ident() {
                // FourCCs are Latin-1, so 0xA9 is "©"
                AtomIdent::Fourcc(fourcc) => fourcc.iter().map(|&byte| char::from(byte)).collect(),
                AtomIdent::Freeform { mean, name } => format!("----:{mean}:{name}"),
            };
            let Some(id) = mp4_common(&key.to_uppercase()) else {
                continue;
            };
            for data in atom.data() {
                let value = match data {
                    AtomData::UTF8(text) | AtomData::UTF16(text) => text.clone(),
                    // trkn and disk: music-metadata read one byte of each number
                    AtomData::Unknown { code, data } if u32::from(*code) == 0 => {
                        if !matches!(id, Common::Track | Common::Disk) || data.len() < 6 {
                            continue;
                        }
                        format!("{}/{}", data[3], data[5])
                    }
                    // music-metadata read type 18 as UTF-8
                    AtomData::Unknown { code, data } if u32::from(*code) == 18 => {
                        String::from_utf8_lossy(data).into_owned()
                    }
                    AtomData::SignedInteger(number) => number.to_string(),
                    AtomData::UnsignedInteger(number) => number.to_string(),
                    // 8, 16 and 32-bit integers, which lofty leaves as bytes
                    AtomData::Unknown { code, data } => match (u32::from(*code), data.as_slice()) {
                        (65, [byte, ..]) => byte.to_string(),
                        (66, [a, b, ..]) => u16::from_be_bytes([*a, *b]).to_string(),
                        (67, [a, b, c, d, ..]) => u32::from_be_bytes([*a, *b, *c, *d]).to_string(),
                        _ => continue,
                    },
                    _ => continue,
                };
                self.add(Origin::ITunes, id, &value);
            }
        }
    }

    fn riff_info(&mut self, info: Option<&RiffInfoList>) {
        for (key, value) in info.into_iter().flatten() {
            if let Some(id) = riff_common(key) {
                // music-metadata read up to the first null
                let value = value.split('\0').next().unwrap_or_default();
                self.add(Origin::Exif, id, value);
            }
        }
    }

    fn aiff_text(&mut self, chunks: Option<&AiffTextChunks>) {
        let Some(chunks) = chunks else { return };
        for (id, text) in [
            (Common::Title, &chunks.name),
            (Common::Artist, &chunks.author),
        ] {
            for value in text.iter().flat_map(|text| text.split('\0')) {
                let value = value.trim();
                if !value.is_empty() {
                    self.add(Origin::Aiff, id, value);
                }
            }
        }
    }

    /// A native tag's value, mapped to a common tag: `postMap()`.
    fn add(&mut self, origin: Origin, id: Common, value: &str) {
        match id {
            Common::Artist => self.singular(origin, value, Common::Artist, Common::Artists),
            Common::AlbumArtist => {
                self.singular(origin, value, Common::AlbumArtist, Common::AlbumArtists)
            }
            Common::Artists => self.plural(origin, value, Common::Artist, Common::Artists),
            Common::AlbumArtists => {
                self.plural(origin, value, Common::AlbumArtist, Common::AlbumArtists)
            }
            // The last one wins, whatever its tag
            Common::Track => self.track = track_number(value),
            Common::Disk => self.disk = track_number(value),
            Common::Date => {
                let year: String = value.chars().take(4).collect();
                if let Some(year) = parse_int(&year) {
                    self.year = Some(year);
                }
                self.set(origin, id, value);
            }
            _ => self.set(origin, id, value),
        }
    }

    /// `handleSingularArtistTag()`: a second value from the same tag goes to
    /// the list, and the first fills the list if it's empty.
    fn singular(&mut self, origin: Origin, value: &str, singular: Common, plural: Common) {
        if self.single(singular).map(|single| single.priority) == Some(origin.priority()) {
            return self.plural(Origin::Artificial, value, singular, plural);
        }
        if self.list(plural).is_none() {
            self.set(Origin::Artificial, plural, value);
        }
        self.set(origin, singular, value);
    }

    /// `handlePluralArtistTag()`: the list's values fill the single value,
    /// joined, unless a tag set it.
    fn plural(&mut self, origin: Origin, value: &str, singular: Common, plural: Common) {
        let single = self.single(singular);
        let unset = single.is_none_or(|single| single.value.is_empty());
        let artificial =
            single.is_some_and(|single| single.priority == Origin::Artificial.priority());
        if unset || artificial {
            let listed = self.list(plural).map(|list| list.values.clone());
            if listed
                .as_ref()
                .is_none_or(|listed| !listed.iter().any(|listed| listed == value))
            {
                let mut values = listed.unwrap_or_default();
                values.push(value.to_string());
                self.set(Origin::Artificial, singular, &join_artists(&values));
            }
        }
        self.set(origin, plural, value);
    }

    /// `setGenericTag()`: a tag of higher or the same priority replaces a
    /// single value, and one of the same priority adds to a list.
    fn set(&mut self, origin: Origin, id: Common, value: &str) {
        let priority = origin.priority();
        match id {
            Common::Artists | Common::AlbumArtists => {
                let list = match id {
                    Common::Artists => &mut self.artists,
                    _ => &mut self.album_artists,
                };
                match list {
                    Some(list) if list.priority == priority => {
                        if !list.values.iter().any(|listed| listed == value) {
                            list.values.push(value.to_string());
                        }
                    }
                    Some(list) if priority > list.priority => {}
                    _ => {
                        *list = Some(List {
                            values: vec![value.to_string()],
                            priority,
                        })
                    }
                }
            }
            Common::Year => {
                if priority <= self.year_priority.unwrap_or(1000) {
                    self.year = parse_int(value);
                    self.year_priority = Some(priority);
                }
            }
            _ => {
                let single = match id {
                    Common::Title => &mut self.title,
                    Common::Artist => &mut self.artist,
                    Common::AlbumArtist => &mut self.album_artist,
                    Common::Album => &mut self.album,
                    Common::DiscSubtitle => &mut self.disc_subtitle,
                    Common::Date => &mut self.date,
                    _ => unreachable!("{id:?} isn't a single value"),
                };
                if single
                    .as_ref()
                    .is_none_or(|single| priority <= single.priority)
                {
                    *single = Some(Single {
                        value: value.to_string(),
                        priority,
                    });
                }
            }
        }
    }

    fn single(&self, id: Common) -> Option<&Single> {
        match id {
            Common::Artist => self.artist.as_ref(),
            Common::AlbumArtist => self.album_artist.as_ref(),
            _ => None,
        }
    }

    fn list(&self, id: Common) -> Option<&List> {
        match id {
            Common::Artists => self.artists.as_ref(),
            Common::AlbumArtists => self.album_artists.as_ref(),
            _ => None,
        }
    }

    /// The tags as slunk's `readAudio()` returned them: trimmed, and blank
    /// ones left out.
    fn finish(self) -> AudioTags {
        let text = |single: Option<Single>| {
            single
                .map(|single| trim(&single.value).to_string())
                .filter(|text| !text.is_empty())
        };
        let artists = self.artists.map(|list| list.values).unwrap_or_default();
        AudioTags {
            artists: artists
                .iter()
                .map(|artist| trim(artist).to_string())
                .filter(|artist| !artist.is_empty())
                .collect(),
            album_artists: text(self.album_artist).into_iter().collect(),
            date: date_of(
                self.date.as_ref().map(|date| date.value.as_str()),
                self.year,
            ),
            title: text(self.title),
            album: text(self.album),
            track_number: self.track,
            disc_number: self.disk,
            disc_title: text(self.disc_subtitle),
        }
    }
}

/// An ID3v2 frame's values, as music-metadata split them. ID3v2.4 separates
/// values with a null. ID3v2.3 does too, unofficially, and TPE1 and TXXX
/// were split at "/" too. ID3v2.2's, and other ID3v2.3 frames, are kept
/// whole.
fn id3v2_values(version: Id3v2Version, id: &str, text: &str) -> Vec<String> {
    let text = text.trim_end_matches('\0');
    let split = |text: &str| -> Vec<String> {
        let values: Vec<&str> = if version != Id3v2Version::V4 {
            let values: Vec<&str> = text.split('\0').collect();
            if values.len() == 1 && matches!(id, "TPE1" | "TXXX") {
                text.split('/').collect()
            } else {
                values
            }
        } else {
            text.split('\0').collect()
        };
        values
            .into_iter()
            .map(|value| trim(value.trim_end_matches('\0')).to_string())
            .collect()
    };
    match (version, id) {
        (_, "TRCK" | "TPOS" | "TIT1" | "TIT2" | "TIT3") => vec![text.to_string()],
        (Id3v2Version::V2, _) => vec![text.to_string()],
        (_, "TPE1" | "TXXX") => split(text),
        (Id3v2Version::V4, _) => split(text),
        _ => vec![text.to_string()],
    }
}

fn vorbis_common(key: &str) -> Option<Common> {
    Some(match key {
        "TITLE" => Common::Title,
        "ARTIST" => Common::Artist,
        "ARTISTS" | "DISCOGS_ARTISTS" | "DISCOGS_ARTIST_NAME" => Common::Artists,
        "ALBUMARTIST" | "ALBUM ARTIST" | "DISCOGS_ALBUM_ARTISTS" => Common::AlbumArtist,
        "ALBUM" => Common::Album,
        "DATE" | "DISCOGS_RELEASED" => Common::Date,
        "TRACKNUMBER" => Common::Track,
        "DISCNUMBER" => Common::Disk,
        "DISCSUBTITLE" => Common::DiscSubtitle,
        _ => return None,
    })
}

/// ID3v2.2's frames come from lofty renamed to ID3v2.4's: TYE is TDRC, and
/// TDR is TDRL.
fn id3v2_common(version: Id3v2Version, key: &str) -> Option<Common> {
    if version == Id3v2Version::V2 {
        return Some(match key {
            "TIT2" => Common::Title,
            "TPE1" => Common::Artist,
            "TPE2" => Common::AlbumArtist,
            "TALB" => Common::Album,
            "TDRC" => Common::Year,
            "TRCK" => Common::Track,
            "TPOS" => Common::Disk,
            "TDRL" => Common::Date,
            _ => return None,
        });
    }
    Some(match key {
        "TIT2" => Common::Title,
        "TPE1" => Common::Artist,
        "TXXX:ARTISTS" | "TXXX:DISCOGS_ARTISTS" | "TXXX:DISCOGS_ARTIST_NAME" => Common::Artists,
        "TPE2" | "TXXX:DISCOGS_ALBUM_ARTISTS" => Common::AlbumArtist,
        "TALB" => Common::Album,
        "TDRC" | "TDRV" | "TXXX:DISCOGS_RELEASED" => Common::Date,
        "TYER" => Common::Year,
        "TRCK" => Common::Track,
        "TPOS" => Common::Disk,
        "TSST" => Common::DiscSubtitle,
        _ => return None,
    })
}

fn mp4_common(key: &str) -> Option<Common> {
    Some(match key {
        "©NAM" => Common::Title,
        "©ART" => Common::Artist,
        "----:COM.APPLE.ITUNES:ARTISTS" => Common::Artists,
        "AART" | "----:COM.APPLE.ITUNES:BAND" => Common::AlbumArtist,
        "©ALB" => Common::Album,
        "©DAY" => Common::Date,
        "TRKN" => Common::Track,
        "DISK" => Common::Disk,
        "----:COM.APPLE.ITUNES:DISCSUBTITLE" => Common::DiscSubtitle,
        _ => return None,
    })
}

fn ape_common(key: &str) -> Option<Common> {
    Some(match key {
        "TITLE" => Common::Title,
        "ARTIST" => Common::Artist,
        "ARTISTS" => Common::Artists,
        "ALBUM ARTIST" => Common::AlbumArtist,
        "ALBUM" => Common::Album,
        "YEAR" | "DATE" => Common::Date,
        "TRACK" => Common::Track,
        "DISC" | "DISCNUMBER" => Common::Disk,
        "DISCSUBTITLE" => Common::DiscSubtitle,
        _ => return None,
    })
}

fn riff_common(key: &str) -> Option<Common> {
    Some(match key {
        "INAM" | "TITL" => Common::Title,
        "IART" => Common::Artist,
        "IPRD" | "IRPD" => Common::Album,
        "ICRD" => Common::Date,
        "ITRK" | "IPRT" => Common::Track,
        "YEAR" => Common::Year,
        _ => return None,
    })
}

/// `normalizeTrack()`: the number before a "/", or `None` for 0.
fn track_number(value: &str) -> Option<i64> {
    let number = value.split('/').next().and_then(parse_int);
    number.filter(|number| *number != 0)
}

/// JavaScript's `parseInt(text, 10)`: leading white space, a sign, then
/// digits, up to the first that isn't one. `None` is NaN.
fn parse_int(text: &str) -> Option<i64> {
    let text = text.trim_start_matches(is_js_space);
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let digits: String = digits.chars().take_while(char::is_ascii_digit).collect();
    let number: i64 = digits.parse().ok()?;
    Some(if negative { -number } else { number })
}

/// music-metadata's way of joining artists: "A, B & C".
fn join_artists(artists: &[String]) -> String {
    match artists {
        [.., last] if artists.len() > 2 => {
            format!("{} & {last}", artists[..artists.len() - 1].join(", "))
        }
        _ => artists.join(" & "),
    }
}

/// slunk's `dateOf()`: the date's `2006`, `2006-01` or `2006-01-10`, else
/// the year.
fn date_of(date: Option<&str>, year: Option<i64>) -> Option<String> {
    if let Some(date) = date {
        let bytes = date.as_bytes();
        let digits = |range: std::ops::Range<usize>| {
            bytes
                .get(range)
                .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
        };
        if digits(0..4) {
            let length = if bytes.get(4) == Some(&b'-') && digits(5..7) {
                if bytes.get(7) == Some(&b'-') && digits(8..10) {
                    10
                } else {
                    7
                }
            } else {
                4
            };
            return Some(date[..length].to_string());
        }
    }
    year.filter(|year| *year != 0).map(|year| year.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_parse_as_javascripts_parse_int() {
        assert_eq!(parse_int("3"), Some(3));
        assert_eq!(parse_int(" 03/12"), Some(3));
        assert_eq!(parse_int("+4a"), Some(4));
        assert_eq!(parse_int("-2"), Some(-2));
        assert_eq!(parse_int("a3"), None);
        assert_eq!(parse_int(""), None);
        assert_eq!(track_number("3/12"), Some(3));
        assert_eq!(track_number("0"), None);
        assert_eq!(track_number("/12"), None);
    }

    #[test]
    fn dates_are_a_year_a_month_or_a_day() {
        assert_eq!(
            date_of(Some("2006-01-10T12:00"), None).as_deref(),
            Some("2006-01-10")
        );
        assert_eq!(date_of(Some("2006-01"), None).as_deref(), Some("2006-01"));
        assert_eq!(date_of(Some("2006-1-10"), None).as_deref(), Some("2006"));
        assert_eq!(date_of(Some("c2006"), Some(2006)).as_deref(), Some("2006"));
        assert_eq!(date_of(None, Some(1999)).as_deref(), Some("1999"));
        assert_eq!(date_of(None, None), None);
    }

    #[test]
    fn artists_join_as_music_metadata_joined_them() {
        let names = |names: &[&str]| {
            names
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(join_artists(&names(&["A"])), "A");
        assert_eq!(join_artists(&names(&["A", "B"])), "A & B");
        assert_eq!(join_artists(&names(&["A", "B", "C"])), "A, B & C");
    }

    #[test]
    fn a_second_artist_tag_adds_to_the_list_and_an_artists_tag_wins() {
        let mut tags = Collector::default();
        tags.add(Origin::Vorbis, Common::Artist, "A");
        tags.add(Origin::Vorbis, Common::Artist, "B");
        tags.add(Origin::Vorbis, Common::AlbumArtist, "C");
        tags.add(Origin::Vorbis, Common::AlbumArtist, "D");
        let read = tags.finish();
        assert_eq!(read.artists, ["A", "B"]);
        // The first album artist, since the second went to the list
        assert_eq!(read.album_artists, ["C"]);

        let mut tags = Collector::default();
        tags.add(Origin::Vorbis, Common::Artist, "A & B");
        tags.add(Origin::Vorbis, Common::Artists, "A");
        tags.add(Origin::Vorbis, Common::Artists, "B");
        assert_eq!(tags.finish().artists, ["A", "B"]);
    }

    #[test]
    fn a_tag_of_higher_priority_wins_but_the_last_track_number_does() {
        let mut tags = Collector::default();
        tags.add(Origin::Id3v24, Common::Title, "From ID3v2");
        tags.add(Origin::Id3v24, Common::Track, "3/12");
        tags.add(Origin::Id3v1, Common::Title, "From ID3v1");
        tags.add(Origin::Id3v1, Common::Track, "4");
        let read = tags.finish();
        assert_eq!(read.title.as_deref(), Some("From ID3v2"));
        assert_eq!(read.track_number, Some(4));
    }

    #[test]
    fn id3v2_values_split_by_version() {
        let v3 = |id, text| id3v2_values(Id3v2Version::V3, id, text);
        let v4 = |id, text| id3v2_values(Id3v2Version::V4, id, text);
        assert_eq!(v3("TPE1", "AC/DC"), ["AC", "DC"]);
        assert_eq!(v3("TPE2", "AC/DC"), ["AC/DC"]);
        assert_eq!(v3("TPE1", "A\0B"), ["A", "B"]);
        assert_eq!(v4("TPE1", "AC/DC"), ["AC/DC"]);
        assert_eq!(v4("TPE2", "A\0 B\0"), ["A", "B"]);
        assert_eq!(v4("TIT2", "A\0B"), ["A\0B"]);
    }

    #[test]
    fn an_unreadable_file_has_no_duration_and_no_tags() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.flac");
        std::fs::write(&path, "not audio").unwrap();
        assert_eq!(read_audio(&path), Audio::default());
        assert_eq!(read_audio(&dir.path().join("gone.flac")), Audio::default());
    }
}
