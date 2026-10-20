//! Tags written into files, with lofty (4_external_libraries.md §4.1): each
//! format's own fields, from the one mapping, and the cover as the front
//! cover. A file's tags are all replaced, whatever it had: only the
//! encoder's gapless data, which `afconvert` keeps among an M4A's tags, is
//! carried over. It only ever writes to a file liquidslunk made: a copy or a
//! conversion in the temporary folder.

use std::borrow::Cow;
use std::fs::File;
use std::path::Path;

use lofty::TextEncoding;
use lofty::ape::{ApeItem, ApeTag};
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::FileType;
use lofty::id3::v2::{
    ExtendedTextFrame, Frame, FrameId, Id3v2Tag, TextInformationFrame, TimestampFrame,
};
use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst, Mp4File};
use lofty::ogg::OggPictureStorage;
use lofty::ogg::tag::VorbisComments;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::items::Timestamp;
use lofty::tag::{ItemValue, TagType};

use super::Tags;
use crate::{Error, Result};

/// Writes a track's tags, and its cover as a JPEG, into a file, replacing
/// whatever tags it had.
pub fn write(path: &Path, tags: &Tags, cover: Option<&[u8]>) -> Result<()> {
    let file_type = Probe::open(path)
        .map_err(failed)?
        .guess_file_type()
        .map_err(Error::io(path))?
        .file_type()
        .ok_or_else(|| {
            Error::Invalid("Its tags couldn't be written: its format isn't known".into())
        })?;
    let options = WriteOptions::default();
    // The other tags it has, read without their pictures
    let present: Vec<TagType> = Probe::open(path)
        .map_err(failed)?
        .options(ParseOptions::new().read_cover_art(false))
        .read()
        .map(|file| file.tags().iter().map(|tag| tag.tag_type()).collect())
        .unwrap_or_default();
    let remove = |others: &[TagType]| -> Result<()> {
        for other in others.iter().filter(|other| present.contains(other)) {
            // lofty's remove_from_path() opens the file read-only, and then
            // fails to truncate it
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(Error::io(path))?;
            other.remove_from(&mut file, options).map_err(failed)?;
        }
        Ok(())
    };
    let picture = cover.map(|jpeg| {
        Picture::unchecked(jpeg.to_vec())
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Jpeg)
            .build()
    });
    match file_type {
        FileType::Flac | FileType::Vorbis | FileType::Opus | FileType::Speex => {
            remove(&[TagType::Id3v2])?;
            vorbis(tags, picture)?
                .save_to_path(path, options)
                .map_err(failed)
        }
        FileType::Mpeg | FileType::Aac | FileType::Wav | FileType::Aiff => {
            remove(&[
                TagType::Id3v1,
                TagType::Ape,
                TagType::RiffInfo,
                TagType::AiffText,
            ])?;
            id3v2(tags, picture)
                .save_to_path(path, options)
                .map_err(failed)
        }
        FileType::Mp4 => {
            let gapless = gapless_atom(path)?;
            let mut ilst = ilst(tags, picture);
            if let Some(atom) = gapless {
                ilst.insert(atom);
            }
            ilst.save_to_path(path, options).map_err(failed)
        }
        FileType::Ape | FileType::WavPack | FileType::Mpc => {
            remove(&[TagType::Id3v1, TagType::Id3v2])?;
            ape(tags, picture)?
                .save_to_path(path, options)
                .map_err(failed)
        }
        _ => Err(Error::Invalid(
            "Its tags couldn't be written: liquidslunk doesn't tag this format".into(),
        )),
    }
}

fn failed(error: impl std::error::Error) -> Error {
    // lofty's own message leaves out its cause
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    Error::Invalid(format!("Its tags couldn't be written: {message}"))
}

/// Vorbis comments, for FLAC and Ogg: one comment per value.
fn vorbis(tags: &Tags, picture: Option<Picture>) -> Result<VorbisComments> {
    let mut comments = VorbisComments::new();
    let mut push = |key: &str, value: &str| comments.push(key.into(), value.into());
    push("TITLE", &tags.title);
    each(&tags.artist, |value| push("ARTIST", value));
    for artist in &tags.artists {
        push("ARTISTS", artist);
    }
    push("ALBUM", &tags.album);
    each(&tags.album_artist, |value| push("ALBUMARTIST", value));
    if tags.compilation {
        push("COMPILATION", "1");
    }
    each(&tags.date, |value| push("DATE", value));
    push("TRACKNUMBER", &tags.track_number.to_string());
    push("TRACKTOTAL", &tags.track_total.to_string());
    push("DISCNUMBER", &tags.disc_number.to_string());
    push("DISCTOTAL", &tags.disc_total.to_string());
    each(&tags.disc_title, |value| push("DISCSUBTITLE", value));
    for label in &tags.labels {
        push("LABEL", label);
    }
    each(&tags.catalog_number, |value| push("CATALOGNUMBER", value));
    for medium in &tags.media {
        push("MEDIA", medium);
    }
    for release_type in &tags.release_types {
        push("RELEASETYPE", release_type);
    }
    each(&tags.sort.artist, |value| push("ARTISTSORT", value));
    each(&tags.sort.album_artist, |value| {
        push("ALBUMARTISTSORT", value)
    });
    each(&tags.sort.album, |value| push("ALBUMSORT", value));
    each(&tags.sort.title, |value| push("TITLESORT", value));
    if let Some(picture) = picture {
        comments
            .insert_picture(picture, None)
            .map_err(|error| Error::Invalid(format!("Its cover couldn't be written: {error}")))?;
    }
    Ok(comments)
}

/// An ID3v2.4 tag, for MP3, AAC in ADTS, WAV and AIFF. Several values are
/// one frame's, separated by nulls, as ID3v2.4 has them.
fn id3v2(tags: &Tags, picture: Option<Picture>) -> Id3v2Tag {
    let mut tag = Id3v2Tag::new();
    let mut text = |id: &'static str, values: &[String]| {
        if !values.is_empty() {
            tag.insert(Frame::Text(TextInformationFrame::new(
                FrameId::Valid(Cow::Borrowed(id)),
                TextEncoding::UTF8,
                values.join("\0"),
            )));
        }
    };
    text("TIT2", std::slice::from_ref(&tags.title));
    text("TPE1", &listed(&tags.artist));
    text("TALB", std::slice::from_ref(&tags.album));
    text("TPE2", &listed(&tags.album_artist));
    if tags.compilation {
        text("TCMP", &["1".into()]);
    }
    text(
        "TRCK",
        &[format!("{}/{}", tags.track_number, tags.track_total)],
    );
    text(
        "TPOS",
        &[format!("{}/{}", tags.disc_number, tags.disc_total)],
    );
    text("TSST", &listed(&tags.disc_title));
    text("TPUB", &tags.labels);
    text("TMED", &tags.media);
    text("TSOP", &listed(&tags.sort.artist));
    text("TSO2", &listed(&tags.sort.album_artist));
    text("TSOA", &listed(&tags.sort.album));
    text("TSOT", &listed(&tags.sort.title));
    let mut user = |description: &str, values: &[String]| {
        if !values.is_empty() {
            tag.insert(Frame::UserText(ExtendedTextFrame::new(
                TextEncoding::UTF8,
                Cow::Owned(description.into()),
                Cow::Owned(values.join("\0")),
            )));
        }
    };
    user("ARTISTS", &tags.artists);
    user("CATALOGNUMBER", &listed(&tags.catalog_number));
    user("RELEASETYPE", &tags.release_types);
    if let Some(timestamp) = tags.date.as_deref().and_then(timestamp) {
        tag.insert(Frame::Timestamp(TimestampFrame::new(
            FrameId::Valid(Cow::Borrowed("TDRC")),
            TextEncoding::UTF8,
            timestamp,
        )));
    }
    if let Some(picture) = picture {
        tag.insert_picture(picture);
    }
    tag
}

/// MP4 atoms, for M4A: one data atom per value, and freeform atoms as
/// MusicBrainz Picard writes them, `----:com.apple.iTunes:<NAME>`.
fn ilst(tags: &Tags, picture: Option<Picture>) -> Ilst {
    let mut ilst = Ilst::new();
    let mut atom = |ident: AtomIdent<'static>, values: Vec<String>| {
        let data = values.into_iter().map(AtomData::UTF8).collect();
        if let Some(atom) = Atom::from_collection(ident, data) {
            ilst.insert(atom);
        }
    };
    let fourcc = |code: &[u8; 4]| AtomIdent::Fourcc(*code);
    let freeform = |name: &'static str| AtomIdent::Freeform {
        mean: Cow::Borrowed("com.apple.iTunes"),
        name: Cow::Borrowed(name),
    };
    atom(fourcc(b"\xa9nam"), vec![tags.title.clone()]);
    atom(fourcc(b"\xa9ART"), listed(&tags.artist));
    atom(freeform("ARTISTS"), tags.artists.clone());
    atom(fourcc(b"\xa9alb"), vec![tags.album.clone()]);
    atom(fourcc(b"aART"), listed(&tags.album_artist));
    atom(fourcc(b"\xa9day"), listed(&tags.date));
    atom(freeform("DISCSUBTITLE"), listed(&tags.disc_title));
    atom(freeform("LABEL"), tags.labels.clone());
    atom(freeform("CATALOGNUMBER"), listed(&tags.catalog_number));
    atom(freeform("MEDIA"), tags.media.clone());
    atom(freeform("RELEASETYPE"), tags.release_types.clone());
    atom(fourcc(b"soar"), listed(&tags.sort.artist));
    atom(fourcc(b"soaa"), listed(&tags.sort.album_artist));
    atom(fourcc(b"soal"), listed(&tags.sort.album));
    atom(fourcc(b"sonm"), listed(&tags.sort.title));
    if tags.compilation {
        ilst.set_flag(fourcc(b"cpil"), true);
    }
    ilst.set_track(number(tags.track_number));
    ilst.set_track_total(number(tags.track_total));
    ilst.set_disk(number(tags.disc_number));
    ilst.set_disk_total(number(tags.disc_total));
    if let Some(picture) = picture {
        ilst.insert_picture(picture);
    }
    ilst
}

/// The encoder's gapless data in an M4A, `----:com.apple.iTunes:iTunSMPB`:
/// its delay and padding. `afconvert` writes it among the tags, and an AAC
/// file without it plays with gaps.
fn gapless_atom(path: &Path) -> Result<Option<Atom<'static>>> {
    let mut file = File::open(path).map_err(Error::io(path))?;
    let parsed = Mp4File::read_from(&mut file, ParseOptions::new().read_cover_art(false))
        .map_err(|error| Error::Invalid(format!("Its tags couldn't be read: {error}")))?;
    let ident = AtomIdent::Freeform {
        mean: Cow::Borrowed("com.apple.iTunes"),
        name: Cow::Borrowed("iTunSMPB"),
    };
    Ok(parsed.ilst().and_then(|ilst| ilst.get(&ident)).cloned())
}

/// An APEv2 tag, for APE, WavPack and Musepack, kept as they are in a folder
/// library: several values in one item, separated by nulls.
fn ape(tags: &Tags, picture: Option<Picture>) -> Result<ApeTag> {
    let mut tag = ApeTag::new();
    let mut item = |key: &str, values: &[String]| -> Result<()> {
        if !values.is_empty() {
            let item =
                ApeItem::new(key.into(), ItemValue::Text(values.join("\0"))).map_err(|error| {
                    Error::Invalid(format!("Its tags couldn't be written: {error}"))
                })?;
            tag.insert(item);
        }
        Ok(())
    };
    item("Title", std::slice::from_ref(&tags.title))?;
    item("Artist", &listed(&tags.artist))?;
    item("Artists", &tags.artists)?;
    item("Album", std::slice::from_ref(&tags.album))?;
    item("Album Artist", &listed(&tags.album_artist))?;
    if tags.compilation {
        item("Compilation", &["1".into()])?;
    }
    item("Year", &listed(&tags.date))?;
    item(
        "Track",
        &[format!("{}/{}", tags.track_number, tags.track_total)],
    )?;
    item(
        "Disc",
        &[format!("{}/{}", tags.disc_number, tags.disc_total)],
    )?;
    item("DiscSubtitle", &listed(&tags.disc_title))?;
    item("Label", &tags.labels)?;
    item("CatalogNumber", &listed(&tags.catalog_number))?;
    item("Media", &tags.media)?;
    item("ReleaseType", &tags.release_types)?;
    item("ArtistSort", &listed(&tags.sort.artist))?;
    item("AlbumArtistSort", &listed(&tags.sort.album_artist))?;
    item("AlbumSort", &listed(&tags.sort.album))?;
    item("TitleSort", &listed(&tags.sort.title))?;
    if let Some(picture) = picture {
        let item = ApeItem::new(
            "Cover Art (Front)".into(),
            ItemValue::Binary(picture.as_ape_bytes()),
        )
        .map_err(|error| Error::Invalid(format!("Its cover couldn't be written: {error}")))?;
        tag.insert(item);
    }
    Ok(tag)
}

/// A date as precise as it's known, `2006`, `2006-01` or `2006-01-10`, as
/// ID3v2.4's timestamp.
fn timestamp(date: &str) -> Option<Timestamp> {
    let mut parts = date.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next().and_then(|part| part.parse().ok());
    let day = parts.next().and_then(|part| part.parse().ok());
    Some(Timestamp {
        year,
        month,
        day,
        ..Timestamp::default()
    })
}

fn listed(value: &Option<String>) -> Vec<String> {
    value.iter().cloned().collect()
}

fn each(value: &Option<String>, mut write: impl FnMut(&str)) {
    if let Some(value) = value {
        write(value);
    }
}

fn number(value: i64) -> u32 {
    u32::try_from(value).unwrap_or(0)
}
