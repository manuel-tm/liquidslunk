//! What a track's file will take on a destination, estimated before
//! anything is converted, by what each source becomes (backend.md, "Rules and
//! what they come to").

use super::{ExternalKind, OutputFormat, Profile};

/// A track's file on a destination, estimated from its extension: its
/// duration at the profile's bitrate when it becomes AAC, and its own size
/// when it's copied or stays lossless. `None` when it's left out: Core Audio
/// can't decode APE, WavPack or Musepack, so they're only copied, to a folder
/// library that keeps the originals, and lofty can't tag DSF, DFF or WMA.
///
/// An `.m4a` is AAC or Apple Lossless, which its extension doesn't say, so it
/// counts at its own size. So do covers, which aren't counted.
pub fn estimated_size(
    extension: &str,
    size_bytes: i64,
    duration_ms: Option<i64>,
    kind: ExternalKind,
    profile: &Profile,
) -> Option<i64> {
    let extension = extension.to_ascii_lowercase();
    let is = |extensions: &[&str]| extensions.contains(&extension.as_str());
    if is(&["dsf", "dff", "wma"]) {
        return None;
    }
    if is(&["ape", "wv", "mpc"]) {
        let kept = kind == ExternalKind::Folder && profile.format == OutputFormat::Original;
        return kept.then_some(size_bytes);
    }
    let lossless = is(&["flac", "alac", "wav", "aif", "aiff"]);
    // The iPod can't play Ogg, so it gets AAC
    let ogg = is(&["ogg", "oga", "opus"]);
    let to_aac =
        (lossless && profile.format == OutputFormat::Aac) || (ogg && kind == ExternalKind::Ipod);
    Some(match (to_aac, duration_ms) {
        // kbps is bits per millisecond
        (true, Some(ms)) => ms * profile.aac_kbps / 8,
        _ => size_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(format: OutputFormat) -> Profile {
        Profile {
            format,
            aac_kbps: 256,
            cd_resolution: false,
            path_template: None,
            embed_covers: true,
            cover_px: 600,
            cover_file: false,
        }
    }

    #[test]
    fn sizes_follow_what_each_source_becomes() {
        let estimate = |extension, kind, format| {
            estimated_size(extension, 30_000_000, Some(240_000), kind, &profile(format))
        };
        use ExternalKind::{Folder, Ipod};
        use OutputFormat::{Aac, Alac, Flac, Original};
        // 4 minutes of AAC at 256 kbps
        assert_eq!(estimate("FLAC", Folder, Aac), Some(7_680_000));
        assert_eq!(estimate("wav", Ipod, Aac), Some(7_680_000));
        // Lossless stays about its size, and lossy sources are copied
        assert_eq!(estimate("flac", Folder, Alac), Some(30_000_000));
        assert_eq!(estimate("mp3", Folder, Aac), Some(30_000_000));
        assert_eq!(estimate("m4a", Ipod, Aac), Some(30_000_000));
        // Ogg is copied to a folder, and becomes AAC on an iPod
        assert_eq!(estimate("ogg", Folder, Flac), Some(30_000_000));
        assert_eq!(estimate("opus", Ipod, Alac), Some(7_680_000));
        // What Core Audio can't read is only kept as it is
        assert_eq!(estimate("ape", Folder, Original), Some(30_000_000));
        assert_eq!(estimate("ape", Folder, Flac), None);
        assert_eq!(estimate("wv", Ipod, Alac), None);
        assert_eq!(estimate("dsf", Folder, Original), None);
        // Without a duration, a file counts at its own size
        assert_eq!(
            estimated_size("flac", 30_000_000, None, Folder, &profile(Aac)),
            Some(30_000_000)
        );
    }
}
