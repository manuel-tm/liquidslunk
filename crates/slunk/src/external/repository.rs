//! The records the external libraries context reads and writes, which
//! `repo::external_libraries` stores.

use crate::tagging::TagType;

/// Where an external library's files go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalKind {
    /// A folder, with its files laid out by a path template
    Folder,
    /// An iPod with video, 5th or 5.5th generation
    Ipod,
}

impl ExternalKind {
    /// "folder" or "ipod", as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            ExternalKind::Folder => "folder",
            ExternalKind::Ipod => "ipod",
        }
    }

    pub(crate) fn named(text: &str) -> Option<ExternalKind> {
        [ExternalKind::Folder, ExternalKind::Ipod]
            .into_iter()
            .find(|kind| kind.as_str() == text)
    }
}

/// What lossless sources become. Lossy ones are copied (backend.md, "Syncing
/// a folder library").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    /// Kept as they are: a folder library only
    Original,
    /// A folder library only
    Flac,
    Alac,
    Aac,
}

impl OutputFormat {
    /// "original", "flac", "alac" or "aac", as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            OutputFormat::Original => "original",
            OutputFormat::Flac => "flac",
            OutputFormat::Alac => "alac",
            OutputFormat::Aac => "aac",
        }
    }

    pub(crate) fn named(text: &str) -> Option<OutputFormat> {
        [
            OutputFormat::Original,
            OutputFormat::Flac,
            OutputFormat::Alac,
            OutputFormat::Aac,
        ]
        .into_iter()
        .find(|format| format.as_str() == text)
    }
}

/// What an external library's files are (backend.md, "External libraries").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub format: OutputFormat,
    /// AAC's bitrate
    pub aac_kbps: i64,
    /// Lossless at most 16-bit and 48 kHz
    pub cd_resolution: bool,
    /// A folder library's. `None` on an iPod, which names its own files.
    pub path_template: Option<String>,
    pub embed_covers: bool,
    /// The longest side of an embedded cover, in pixels
    pub cover_px: i64,
    /// Also a `cover.jpg` in each folder that holds one release's tracks
    pub cover_file: bool,
}

/// An external library: a destination, a selection and a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLibrary {
    pub id: i64,
    /// What its destination's manifest names it by
    pub uuid: String,
    pub name: String,
    pub kind: ExternalKind,
    /// A folder library's folder, an absolute path
    pub folder: Option<String>,
    pub profile: Profile,
    /// When its last sync ended, in epoch seconds
    pub last_synced_at: Option<i64>,
    pub inserted_at: i64,
    pub updated_at: i64,
}

/// An external library's columns, as they're saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLibraryValues {
    pub name: String,
    pub kind: ExternalKind,
    pub folder: Option<String>,
    pub profile: Profile,
}

/// What a rule names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RuleTarget {
    /// A release's tracks
    Release(i64),
    /// The tracks of every release the artist is a release artist of
    Artist(i64),
    Track(i64),
    /// The releases with a tag value, whole, and the tracks with it: a tag
    /// filter, as the lists take them, as typed. Text matches `equals`, and
    /// numbers and dates `equals`, or the range from `min` to `max`.
    Tag {
        kind: TagType,
        tag_id: i64,
        equals: Option<String>,
        min: Option<String>,
        max: Option<String>,
    },
}

/// What a rule says: what it names, and whether that's in or left out.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleValues {
    /// Left out, whatever else includes it: a release or a track only
    pub exclude: bool,
    pub target: RuleTarget,
}

/// A rule of an external library's selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: i64,
    pub exclude: bool,
    pub target: RuleTarget,
}

impl Rule {
    pub fn values(&self) -> RuleValues {
        RuleValues {
            exclude: self.exclude,
            target: self.target.clone(),
        }
    }
}
