//! The iPod (4_external_libraries.md §7): the iPod with video, 5th and 5.5th
//! generations, with Apple's firmware. It's found as a mounted volume with an
//! `iPod_Control/iTunes` folder, and written as iTunes wrote it: its music
//! files under names of liquidslunk's own in `iPod_Control/Music`, its
//! iTunesDB, and its artwork. `docs/reference/ipod.md` describes the
//! formats as they're written here.
//!
//! - `itunesdb`: the iTunesDB, read and written (§7.4).
//! - `artwork`: the ArtworkDB and its thumbnails (§7.5).
//! - `audio`: what the database needs of each file: Sound Check, and
//!   gapless playback's data (§7.6).
//! - `play_counts`: what the iPod counted since the last sync (§7.7).
//! - `sysinfo`: which iPod it is (§7.1).
//! - `sync`: running a plan on an iPod (§8.3).

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub mod artwork;
pub mod audio;
pub(crate) mod bytes;
pub mod itunesdb;
pub mod play_counts;
pub mod sync;
pub mod sysinfo;

use super::Folder;
pub use sysinfo::Model;

/// The database, beside liquidslunk's manifest
pub const ITUNES_DB: &str = "iPod_Control/iTunes/iTunesDB";
/// The database before liquidslunk last replaced it, to put back if the
/// iPod refuses the new one
pub const PREVIOUS_DB: &str = "iPod_Control/iTunes/iTunesDB.liquidslunk-previous";
pub const PLAY_COUNTS: &str = "iPod_Control/iTunes/Play Counts";
pub const ARTWORK: &str = "iPod_Control/Artwork";
pub const ARTWORK_DB: &str = "iPod_Control/Artwork/ArtworkDB";
pub const MUSIC: &str = "iPod_Control/Music";
const SYSINFO: &str = "iPod_Control/Device/SysInfo";

/// Where macOS mounts volumes
pub const VOLUMES: &str = "/Volumes";

/// What liquidslunk's files on an iPod are named, before 4 random letters
/// and digits
const PREFIX: &str = "LSK";

/// A mounted iPod.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipod {
    pub mount: PathBuf,
    /// Its volume's name, as Finder shows it
    pub name: String,
    pub model: Option<Model>,
    /// Its model number, when its SysInfo has one, even for another iPod
    pub model_number: Option<String>,
}

impl Ipod {
    /// The iPod mounted at a path, when the volume there is one: it has an
    /// `iPod_Control/iTunes` folder.
    pub fn at(mount: &Path) -> Option<Ipod> {
        if !mount.join(super::folder::IPOD_ITUNES).is_dir() {
            return None;
        }
        let info = std::fs::read_to_string(mount.join(SYSINFO))
            .map(|text| sysinfo::SysInfo::parse(&text))
            .unwrap_or_default();
        Some(Ipod {
            mount: mount.to_path_buf(),
            name: mount.file_name().map_or_else(
                || mount.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            ),
            model: info.model(),
            model_number: info.model_number().map(str::to_string),
        })
    }

    /// Why liquidslunk won't sync it: it's another iPod.
    pub fn refusal(&self) -> Option<String> {
        (self.model.is_none() && self.model_number.is_some()).then(|| {
            format!(
                "liquidslunk syncs only the iPod with video (5th and 5.5th generations). This one is model {}.",
                self.model_number.as_deref().unwrap_or_default()
            )
        })
    }

    /// What's worth knowing before syncing it: that its model can't be read.
    pub fn warning(&self) -> Option<String> {
        self.model_number.is_none().then(|| {
            "Its model can't be read from its SysInfo. liquidslunk syncs only the iPod with video (5th and 5.5th generations).".to_string()
        })
    }

    /// What it is, in words: "iPod with video (5.5th generation), 30 GB,
    /// black", or "An iPod".
    pub fn description(&self) -> String {
        self.model
            .map_or_else(|| "An iPod".to_string(), |model| model.description())
    }

    pub fn folder(&self) -> Folder {
        Folder::ipod(&self.mount)
    }
}

/// The iPods mounted in a folder of volumes, by name.
pub fn mounted_in(volumes: &Path) -> Vec<Ipod> {
    let Ok(entries) = std::fs::read_dir(volumes) else {
        return Vec::new();
    };
    let mut ipods: Vec<Ipod> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            // The Mac's own disk is a link to /
            if entry.file_type().ok()?.is_symlink() {
                return None;
            }
            Ipod::at(&entry.path())
        })
        .collect();
    ipods.sort_by(|a, b| crate::media::library::compare(&a.name, &b.name));
    ipods
}

/// A file's location as the iTunesDB has it, from its path on the iPod:
/// `iPod_Control/Music/F12/LSKQ2MX.m4a` is
/// `:iPod_Control:Music:F12:LSKQ2MX.m4a`.
pub fn location(path: &str) -> String {
    format!(":{}", path.replace('/', ":"))
}

/// A file's path on the iPod, from its location in the iTunesDB.
pub fn path_of(location: &str) -> String {
    location.trim_start_matches(':').replace(':', "/")
}

/// Paths for new files: in one of the music folders already on the iPod,
/// `F00` to `F49`, at random, under unique names like `LSKQ2MX.m4a`, with
/// their extension in lowercase, which some iPods need.
pub struct Names {
    folders: Vec<String>,
    random: Random,
    /// Paths given or found, in lowercase, since the disk may ignore case
    taken: HashSet<String>,
}

impl Names {
    /// The music folders on the iPod at `mount`, with paths already taken.
    pub fn new<'a>(mount: &Path, taken: impl IntoIterator<Item = &'a str>) -> Names {
        let mut folders: Vec<String> = std::fs::read_dir(mount.join(MUSIC))
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let numbered = name.len() == 3
                    && name.starts_with(['F', 'f'])
                    && name[1..].chars().all(|c| c.is_ascii_digit());
                (numbered && entry.path().is_dir()).then_some(name)
            })
            .collect();
        folders.sort();
        if folders.is_empty() {
            folders.push("F00".into());
        }
        Names {
            folders,
            random: Random::new(),
            taken: taken.into_iter().map(str::to_lowercase).collect(),
        }
    }

    /// A free path for a file with this extension. `exists` says whether
    /// something is already at a path.
    pub fn next(&mut self, extension: &str, exists: &dyn Fn(&str) -> bool) -> String {
        const CHARACTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        loop {
            let number = self.random.next_u64();
            let folder = &self.folders[(number % self.folders.len() as u64) as usize];
            let stem: String = (0..4)
                .map(|i| {
                    CHARACTERS[((number >> (8 + i * 8)) % CHARACTERS.len() as u64) as usize] as char
                })
                .collect();
            let path = format!(
                "{MUSIC}/{folder}/{PREFIX}{stem}.{}",
                extension.to_lowercase()
            );
            if !self.taken.contains(&path.to_lowercase()) && !exists(&path) {
                self.taken.insert(path.to_lowercase());
                return path;
            }
        }
    }
}

/// Random numbers, from a seed from the system's: for file names and
/// database ids.
pub struct Random(u64);

impl Random {
    pub fn new() -> Random {
        let mut seed = [0u8; 8];
        let read = File::open("/dev/urandom").and_then(|mut file| file.read_exact(&mut seed));
        let seed = match read {
            Ok(()) => u64::from_le_bytes(seed),
            Err(_) => std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos() as u64),
        };
        Random(seed)
    }

    /// The next number, by SplitMix64.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

impl Default for Random {
    fn default() -> Random {
        Random::new()
    }
}

/// Seconds between 1904, when Mac times start, and 1970
const MAC_EPOCH: i64 = 2_082_844_800;

/// A time as the iPod keeps it: seconds since 1904, in the Mac's local
/// time then. 0 stays 0, for a time that isn't known.
pub fn mac_time(epoch_seconds: i64) -> u32 {
    if epoch_seconds <= 0 {
        return 0;
    }
    u32::try_from(epoch_seconds + MAC_EPOCH + local_offset(epoch_seconds)).unwrap_or(0)
}

/// How far local time was from UTC at a time, in seconds.
#[cfg(unix)]
fn local_offset(epoch_seconds: i64) -> i64 {
    let time = epoch_seconds as libc::time_t;
    let mut local: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both are ours, and localtime_r only fills `local`
    if unsafe { libc::localtime_r(&time, &mut local) }.is_null() {
        return 0;
    }
    local.tm_gmtoff
}

#[cfg(not(unix))]
fn local_offset(_epoch_seconds: i64) -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipods_are_found_by_their_folders() {
        let volumes = tempfile::tempdir().unwrap();
        let ipod = volumes.path().join("IPOD");
        std::fs::create_dir_all(ipod.join("iPod_Control/iTunes")).unwrap();
        std::fs::create_dir_all(ipod.join("iPod_Control/Device")).unwrap();
        std::fs::write(
            ipod.join("iPod_Control/Device/SysInfo"),
            "ModelNumStr: xA446\n",
        )
        .unwrap();
        let classic = volumes.path().join("CLASSIC");
        std::fs::create_dir_all(classic.join("iPod_Control/iTunes")).unwrap();
        std::fs::create_dir_all(classic.join("iPod_Control/Device")).unwrap();
        std::fs::write(
            classic.join("iPod_Control/Device/SysInfo"),
            "ModelNumStr: MB029\n",
        )
        .unwrap();
        std::fs::create_dir(volumes.path().join("CARD")).unwrap();
        let found = mounted_in(volumes.path());
        assert_eq!(
            found
                .iter()
                .map(|ipod| ipod.name.as_str())
                .collect::<Vec<_>>(),
            vec!["CLASSIC", "IPOD"]
        );
        assert!(found[0].refusal().is_some());
        assert!(found[1].refusal().is_none() && found[1].warning().is_none());
        assert_eq!(
            found[1].description(),
            "iPod with video (5.5th generation), 30 GB, black"
        );
    }

    #[test]
    fn new_names_are_free_and_in_the_music_folders() {
        let mount = tempfile::tempdir().unwrap();
        for folder in ["F00", "F01"] {
            std::fs::create_dir_all(mount.path().join(MUSIC).join(folder)).unwrap();
        }
        let mut names = Names::new(mount.path(), ["iPod_Control/Music/F00/LSKAAAA.m4a"]);
        let mut seen = HashSet::new();
        for _ in 0..200 {
            let path = names.next("M4A", &|_| false);
            assert!(path.starts_with("iPod_Control/Music/F0"), "{path}");
            assert!(
                path.ends_with(".m4a") && path.len() == "iPod_Control/Music/F00/LSKAAAA.m4a".len()
            );
            assert!(seen.insert(path));
        }
        assert_eq!(
            location("iPod_Control/Music/F00/LSKQ2MX.m4a"),
            ":iPod_Control:Music:F00:LSKQ2MX.m4a"
        );
        assert_eq!(
            path_of(":iPod_Control:Music:F00:LSKQ2MX.m4a"),
            "iPod_Control/Music/F00/LSKQ2MX.m4a"
        );
    }

    #[test]
    fn mac_times_start_in_1904() {
        assert_eq!(mac_time(0), 0);
        let now = 1_760_000_000;
        let offset = mac_time(now) as i64 - now - MAC_EPOCH;
        assert!(offset.abs() <= 14 * 3600);
    }
}
