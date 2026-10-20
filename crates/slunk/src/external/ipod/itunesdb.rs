//! The iTunesDB (ipod.md, "The iTunesDB"): every track on the iPod, and its
//! master playlist, which the iPod reads to show its music. It never looks at
//! the music folders themselves.
//!
//! It's little-endian chunks, each a four-letter tag, its header's length
//! and its whole length; strings are UTF-16LE, and times are seconds since
//! 1904 in local time. It's written in the layout libgpod writes for the
//! iPod with video, which gtkpod, Rhythmbox and Strawberry used on it for
//! years. `docs/reference/ipod.md` has the layout as it's written here.

use std::collections::HashMap;

use super::bytes::{Reader, Writer, from_utf16, utf16};
use crate::Result;

/// The database's version: iTunes 9.2's, as libgpod writes it
const VERSION: u32 = 0x30;
/// The first track id written, as libgpod numbers them
const FIRST_ID: u32 = 52;

// Header lengths, as written
const MHBD: usize = 244;
const MHSD: usize = 96;
const MHL: usize = 92;
const MHIT: usize = 0x248;
const MHOD: usize = 24;
const MHYP: usize = 108;
const MHIP: usize = 76;
const MHIA: usize = 88;
const MHII: usize = 80;

// String types (`mhod`)
const TITLE: u32 = 1;
const LOCATION: u32 = 2;
const ALBUM: u32 = 3;
const ARTIST: u32 = 4;
const FILE_TYPE: u32 = 6;
const ALBUM_ARTIST: u32 = 22;
const SORT_ARTIST: u32 = 23;
const SORT_TITLE: u32 = 27;
const SORT_ALBUM: u32 = 28;
const SORT_ALBUM_ARTIST: u32 = 29;
const INDEX: u32 = 52;
const JUMP_TABLE: u32 = 53;
const POSITION: u32 = 100;
const ALBUM_LIST_ALBUM: u32 = 200;
const ALBUM_LIST_ARTIST: u32 = 201;
const ALBUM_LIST_SORT_ARTIST: u32 = 202;
const ARTIST_LIST_ARTIST: u32 = 300;

/// What the database keeps from one write to the next: its ids, and what
/// iTunes or Finder set in its header, which liquidslunk doesn't know the
/// meaning of and passes on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Header {
    pub id: u64,
    /// 1 for a Mac, 2 for Windows
    pub platform: u16,
    pub unknown_0x22: u16,
    pub id_0x24: u64,
    /// Two letters, like "en"
    pub language: u16,
    pub library_id: u64,
    pub unknown_0x50: u32,
    pub unknown_0x54: u32,
    pub timezone_offset: i32,
    /// 0xa0 to 0xaa: the audio and subtitle languages, and unknowns
    pub languages: [u8; 10],
}

/// A database, as it's read and written.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Database {
    pub header: Header,
    /// The master playlist's id
    pub master_id: u64,
    /// In the order they're listed, which Play Counts' entries follow
    pub tracks: Vec<DbTrack>,
}

/// What a track's file is, as the database marks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Aac,
    Alac,
    Mp3,
}

impl FileKind {
    /// The marker in its record: its four letters, read as a number
    fn marker(self) -> u32 {
        match self {
            FileKind::Aac | FileKind::Alac => u32::from_be_bytes(*b"M4A "),
            FileKind::Mp3 => u32::from_be_bytes(*b"MP3 "),
        }
    }

    /// What the iPod shows as its kind
    fn name(self) -> &'static str {
        match self {
            FileKind::Aac => "AAC audio file",
            FileKind::Alac => "Apple Lossless audio file",
            FileKind::Mp3 => "MPEG audio file",
        }
    }

    /// Two fields iTunes sets by kind, of unknown meaning: 0xffff and 0x33
    /// for AAC, 0xffff and 0x0c for MP3, none for Apple Lossless
    fn unknowns(self) -> (u16, u16) {
        match self {
            FileKind::Aac => (0xffff, 0x33),
            FileKind::Mp3 => (0xffff, 0x0c),
            FileKind::Alac => (0, 0),
        }
    }

    fn of(marker: u32, name: Option<&str>) -> Option<FileKind> {
        if marker == FileKind::Mp3.marker() {
            Some(FileKind::Mp3)
        } else if marker == FileKind::Aac.marker() {
            Some(match name {
                Some(name) if name.contains("Lossless") => FileKind::Alac,
                _ => FileKind::Aac,
            })
        } else {
            None
        }
    }
}

/// What the iPod counts for a track as it's played: kept from one sync to
/// the next (ipod.md, "Play Counts").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub play_count: u32,
    /// Mac time, as the iPod wrote it
    pub last_played: u32,
    pub skip_count: u32,
    pub last_skipped: u32,
    /// Stars times 20
    pub rating: u8,
}

/// What gapless playback needs (ipod.md, "Gapless and Sound Check"), in
/// samples, and for an MP3 the bytes from its first frame to the 8th before
/// its last.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gapless {
    pub pregap: u32,
    pub postgap: u32,
    pub sample_count: u64,
    pub data: u32,
}

/// A track's record.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DbTrack {
    /// Numbered again at each write
    pub id: u32,
    /// Persistent: a track keeps it from one sync to the next
    pub dbid: u64,
    /// Where its file is: `:iPod_Control:Music:F12:LSKQ2MX.m4a`
    pub location: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub sort_artist: Option<String>,
    pub sort_album_artist: Option<String>,
    pub sort_album: Option<String>,
    pub sort_title: Option<String>,
    /// `None` for a kind liquidslunk doesn't write
    pub kind: Option<FileKind>,
    pub compilation: bool,
    pub size: u32,
    pub length_ms: u32,
    pub track_number: u32,
    pub track_total: u32,
    pub disc_number: u32,
    pub disc_total: u32,
    pub year: u32,
    /// In kbps
    pub bitrate: u32,
    /// In Hz
    pub sample_rate: u32,
    /// Sound Check: 1000 × 10^(−gain/10), 0 for none
    pub sound_check: u32,
    /// Mac times
    pub added: u32,
    pub modified: u32,
    pub stats: Stats,
    pub gapless: Gapless,
    /// Its image in the artwork database, and its cover's size in bytes
    pub artwork: Option<(u32, u32)>,
}

/// Reads a database. Only what liquidslunk uses is read: the tracks, with
/// their strings and counts, and the ids it keeps.
pub fn read(bytes: &[u8]) -> Result<Database> {
    let r = Reader::new(bytes, "iTunesDB");
    r.expect(0, b"mhbd")?;
    let header_length = r.header_length(0)?;
    let at = |offset: usize| offset + 2 <= header_length;
    let header = Header {
        id: r.u64(0x18)?,
        platform: if at(0x22) { r.u16(0x20)? } else { 0 },
        unknown_0x22: if at(0x24) { r.u16(0x22)? } else { 0 },
        id_0x24: if at(0x2c) { r.u64(0x24)? } else { 0 },
        language: if at(0x48) { r.u16(0x46)? } else { 0 },
        library_id: if at(0x50) { r.u64(0x48)? } else { 0 },
        unknown_0x50: if at(0x54) { r.u32(0x50)? } else { 0 },
        unknown_0x54: if at(0x58) { r.u32(0x54)? } else { 0 },
        timezone_offset: if at(0x70) { r.i32(0x6c)? } else { 0 },
        languages: if at(0xaa) {
            r.bytes(0xa0, 10)?.try_into().expect("ten bytes")
        } else {
            [0; 10]
        },
    };
    let mut database = Database {
        header,
        ..Database::default()
    };
    let end = r.total_length(0)?;
    let mut offset = header_length;
    while offset < end {
        r.expect(offset, b"mhsd")?;
        let total = r.total_length(offset)?;
        match r.u32(offset + 12)? {
            1 => database.tracks = read_tracks(&r, offset + r.header_length(offset)?)?,
            2 => {
                database.master_id =
                    read_master_id(&r, offset + r.header_length(offset)?)?.unwrap_or(0)
            }
            _ => {}
        }
        offset += total;
    }
    Ok(database)
}

fn read_tracks(r: &Reader, mhlt: usize) -> Result<Vec<DbTrack>> {
    r.expect(mhlt, b"mhlt")?;
    let count = r.u32(mhlt + 8)? as usize;
    let mut offset = mhlt + r.header_length(mhlt)?;
    let mut tracks = Vec::with_capacity(count.min(100_000));
    for _ in 0..count {
        r.expect(offset, b"mhit")?;
        let header_length = r.header_length(offset)?;
        let total = r.total_length(offset)?;
        tracks.push(read_track(r, offset, header_length)?);
        offset += total;
    }
    Ok(tracks)
}

fn read_track(r: &Reader, at: usize, header_length: usize) -> Result<DbTrack> {
    let field = |offset: usize, size: usize| offset + size <= header_length;
    let u32_at = |offset: usize| -> Result<u32> {
        if field(offset, 4) {
            r.u32(at + offset)
        } else {
            Ok(0)
        }
    };
    let mut track = DbTrack {
        id: u32_at(0x10)?,
        compilation: r.u8(at + 0x1e)? != 0,
        modified: u32_at(0x20)?,
        size: u32_at(0x24)?,
        length_ms: u32_at(0x28)?,
        track_number: u32_at(0x2c)?,
        track_total: u32_at(0x30)?,
        year: u32_at(0x34)?,
        bitrate: u32_at(0x38)?,
        sample_rate: u32_at(0x3c)? >> 16,
        sound_check: u32_at(0x4c)?,
        disc_number: u32_at(0x5c)?,
        disc_total: u32_at(0x60)?,
        added: u32_at(0x68)?,
        dbid: if field(0x70, 8) { r.u64(at + 0x70)? } else { 0 },
        stats: Stats {
            play_count: u32_at(0x50)?,
            last_played: u32_at(0x58)?,
            skip_count: u32_at(0x9c)?,
            last_skipped: u32_at(0xa0)?,
            rating: r.u8(at + 0x1f)?,
        },
        gapless: Gapless {
            pregap: u32_at(0xb8)?,
            sample_count: if field(0xbc, 8) { r.u64(at + 0xbc)? } else { 0 },
            postgap: u32_at(0xc8)?,
            data: u32_at(0xf8)?,
        },
        ..DbTrack::default()
    };
    if field(0xa4, 1) && r.u8(at + 0xa4)? == 1 {
        track.artwork = Some((u32_at(0x160)?, u32_at(0x80)?));
    }
    let marker = u32_at(0x18)?;
    let mut file_type = None;
    let count = r.u32(at + 12)?;
    let mut offset = at + header_length;
    for _ in 0..count {
        r.expect(offset, b"mhod")?;
        let total = r.total_length(offset)?;
        let kind = r.u32(offset + 12)?;
        // Only the strings liquidslunk uses: others, like a podcast's URL
        // or chapters, aren't laid out as strings are
        let text = || read_string(r, offset);
        match kind {
            TITLE => track.title = text()?,
            LOCATION => track.location = text()?,
            ALBUM => track.album = Some(text()?),
            ARTIST => track.artist = Some(text()?),
            FILE_TYPE => file_type = Some(text()?),
            ALBUM_ARTIST => track.album_artist = Some(text()?),
            SORT_ARTIST => track.sort_artist = Some(text()?),
            SORT_TITLE => track.sort_title = Some(text()?),
            SORT_ALBUM => track.sort_album = Some(text()?),
            SORT_ALBUM_ARTIST => track.sort_album_artist = Some(text()?),
            _ => {}
        }
        offset += total;
    }
    track.kind = FileKind::of(marker, file_type.as_deref());
    Ok(track)
}

/// A string `mhod`'s text: UTF-16LE, or UTF-8 when its encoding says 2.
fn read_string(r: &Reader, at: usize) -> Result<String> {
    let body = at + r.header_length(at)?;
    let encoding = r.u32(body)?;
    let length = r.u32(body + 4)? as usize;
    let bytes = r.bytes(body + 16, length)?;
    Ok(if encoding == 2 {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        from_utf16(bytes)
    })
}

/// The master playlist's id, from the playlists' `mhlp`.
fn read_master_id(r: &Reader, mhlp: usize) -> Result<Option<u64>> {
    r.expect(mhlp, b"mhlp")?;
    let count = r.u32(mhlp + 8)?;
    let mut offset = mhlp + r.header_length(mhlp)?;
    for _ in 0..count {
        r.expect(offset, b"mhyp")?;
        if r.u8(offset + 20)? == 1 {
            return Ok(Some(r.u64(offset + 28)?));
        }
        offset += r.total_length(offset)?;
    }
    Ok(None)
}

/// Writes a database, with its master playlist named `name`. Tracks are
/// numbered again from 52 in the order they're given, which is the order
/// Play Counts will follow.
pub fn write(database: &Database, name: &str) -> Vec<u8> {
    let mut tracks = database.tracks.clone();
    for (index, track) in tracks.iter_mut().enumerate() {
        track.id = FIRST_ID + index as u32;
    }
    let albums = Groups::of(&tracks, |track| {
        track.album.as_ref().map(|album| {
            (
                album.clone(),
                track.album_artist.clone().or_else(|| track.artist.clone()),
            )
        })
    });
    let artists = Groups::of(&tracks, |track| track.artist.clone());

    let mut w = Writer::new();
    header(&mut w, &database.header, 8);
    // Tracks
    let mhsd = section(&mut w, 1);
    list(&mut w, b"mhlt", tracks.len());
    for (index, track) in tracks.iter().enumerate() {
        write_track(
            &mut w,
            track,
            &database.header,
            albums.ids[index],
            artists.ids[index],
        );
    }
    w.close(mhsd);
    // The playlists, twice, as iTunes writes them: for podcasts, then all
    for kind in [3, 2] {
        let mhsd = section(&mut w, kind);
        list(&mut w, b"mhlp", 1);
        master(&mut w, database.master_id, name, &tracks);
        w.close(mhsd);
    }
    // Albums
    let mhsd = section(&mut w, 4);
    list(&mut w, b"mhla", albums.first.len());
    for (id, &index) in albums.first.iter().enumerate() {
        let track = &tracks[index];
        let start = w.len();
        w.tag(b"mhia");
        w.u32(MHIA as u32);
        w.u32(0);
        w.u32(0);
        w.u32(id as u32 + 1);
        w.u64(sql_id(database.header.id, 4, id));
        w.u32(2);
        w.pad_to(start, MHIA);
        let artist = track.album_artist.as_ref().or(track.artist.as_ref());
        let sort = track
            .sort_album_artist
            .as_ref()
            .or(track.sort_artist.as_ref());
        let mut count = 0;
        for (kind, text) in [
            (ALBUM_LIST_ALBUM, track.album.as_ref()),
            (ALBUM_LIST_ARTIST, artist),
            (ALBUM_LIST_SORT_ARTIST, sort),
        ] {
            if let Some(text) = text.filter(|text| !text.is_empty()) {
                string(&mut w, kind, text);
                count += 1;
            }
        }
        w.close(start);
        w.set_u32(start + 12, count);
    }
    w.close(mhsd);
    // Artists
    let mhsd = section(&mut w, 8);
    list(&mut w, b"mhli", artists.first.len());
    for (id, &index) in artists.first.iter().enumerate() {
        let start = w.len();
        w.tag(b"mhii");
        w.u32(MHII as u32);
        w.u32(0);
        w.u32(1);
        w.u32(id as u32 + 1);
        w.u64(sql_id(database.header.id, 8, id));
        w.u32(2);
        w.pad_to(start, MHII);
        string(
            &mut w,
            ARTIST_LIST_ARTIST,
            tracks[index].artist.as_deref().unwrap_or_default(),
        );
        w.close(start);
    }
    w.close(mhsd);
    // Two empty lists, and no smart playlists, as libgpod writes them
    for kind in [6, 10] {
        let mhsd = section(&mut w, kind);
        list(&mut w, b"mhlt", 0);
        w.close(mhsd);
    }
    let mhsd = section(&mut w, 5);
    list(&mut w, b"mhlp", 0);
    w.close(mhsd);
    w.close(0);
    w.bytes
}

/// The `mhbd`, with `sections` sections to follow.
fn header(w: &mut Writer, header: &Header, sections: u32) {
    w.tag(b"mhbd");
    w.u32(MHBD as u32);
    w.u32(0);
    w.u32(1);
    w.u32(VERSION);
    w.u32(sections);
    w.u64(header.id);
    w.u16(header.platform);
    w.u16(header.unknown_0x22);
    w.u64(header.id_0x24);
    w.u32(0);
    // 0x30: no hashing scheme, which these iPods don't check
    w.u16(0);
    w.zeros(20);
    w.u16(header.language);
    w.u64(header.library_id);
    w.u32(header.unknown_0x50);
    w.u32(header.unknown_0x54);
    // 0x58: no hash
    w.zeros(20);
    w.i32(header.timezone_offset);
    // 0x70: no checksum's type, and no hash72
    w.zeros(0xa0 - 0x70);
    w.raw(&header.languages);
    w.pad_to(0, MHBD);
}

fn section(w: &mut Writer, kind: u32) -> usize {
    let start = w.len();
    w.tag(b"mhsd");
    w.u32(MHSD as u32);
    w.u32(0);
    w.u32(kind);
    w.pad_to(start, MHSD);
    start
}

/// A list's header: `mhlt`, `mhlp`, `mhla` or `mhli`, with its count.
fn list(w: &mut Writer, tag: &[u8; 4], count: usize) {
    let start = w.len();
    w.tag(tag);
    w.u32(MHL as u32);
    w.u32(count as u32);
    w.pad_to(start, MHL);
}

fn write_track(w: &mut Writer, track: &DbTrack, header: &Header, album: u32, artist: u32) {
    let start = w.len();
    let kind = track.kind.unwrap_or(FileKind::Aac);
    let (unknown_0x7e, unknown_0x90) = kind.unknowns();
    w.tag(b"mhit");
    w.u32(MHIT as u32);
    w.u32(0);
    w.u32(0);
    w.u32(track.id);
    // Visible
    w.u32(1);
    w.u32(kind.marker());
    // 0x1c: whether an MP3 is VBR, twice, as iTunes marks it
    let vbr = u8::from(kind == FileKind::Mp3);
    w.u8(vbr);
    w.u8(vbr);
    w.u8(u8::from(track.compilation));
    w.u8(track.stats.rating);
    w.u32(track.modified);
    w.u32(track.size);
    w.u32(track.length_ms);
    w.u32(track.track_number);
    w.u32(track.track_total);
    w.u32(track.year);
    w.u32(track.bitrate);
    w.u32(track.sample_rate << 16);
    // 0x40: volume, start and stop time
    w.zeros(12);
    w.u32(track.sound_check);
    w.u32(track.stats.play_count);
    w.u32(track.stats.play_count);
    w.u32(track.stats.last_played);
    w.u32(track.disc_number);
    w.u32(track.disc_total);
    // 0x64: the iTunes Store's user id
    w.u32(0);
    w.u32(track.added);
    // 0x6c: bookmark time
    w.u32(0);
    w.u64(track.dbid);
    // 0x78: checked, application rating, BPM
    w.u8(0);
    w.u8(0);
    w.u16(0);
    w.u16(u16::from(track.artwork.is_some()));
    w.u16(unknown_0x7e);
    w.u32(track.artwork.map_or(0, |(_, size)| size));
    w.u32(0);
    w.f32(track.sample_rate as f32);
    // 0x8c: date released
    w.u32(0);
    w.u16(unknown_0x90);
    // 0x92: explicit
    w.u16(0);
    w.zeros(8);
    w.u32(track.stats.skip_count);
    w.u32(track.stats.last_skipped);
    // 0xa4: has artwork, 1, or not, 2
    w.u8(if track.artwork.is_some() { 1 } else { 2 });
    // 0xa5: skip when shuffling, remember the position, podcast
    w.zeros(3);
    w.u64(track.dbid);
    // 0xb0: lyrics, movie, and "played", as a music track is marked
    w.u8(0);
    w.u8(0);
    w.u8(1);
    w.u8(0);
    w.u32(0);
    w.u32(track.gapless.pregap);
    w.u64(track.gapless.sample_count);
    w.u32(0);
    w.u32(track.gapless.postgap);
    w.u32(0);
    // 0xd0: media type, audio
    w.u32(1);
    // 0xd4: season and episode, and unknowns
    w.zeros(0xf8 - 0xd4);
    w.u32(track.gapless.data);
    w.u32(0);
    let gapless = track.gapless != Gapless::default();
    w.u16(u16::from(gapless));
    // 0x102: part of a gapless album, which only stops crossfading
    w.u16(0);
    w.zeros(0x120 - 0x104);
    w.u32(album);
    w.u64(header.id_0x24);
    w.u32(track.size);
    w.u32(0);
    // 0x134: as libgpod writes it, whatever it means
    w.u64(0x8080_8080_8080);
    w.u32(0);
    w.zeros(0x160 - 0x140);
    w.u32(track.artwork.map_or(0, |(image, _)| image));
    w.u32(0);
    w.u32(1);
    w.u32(0);
    w.zeros(0x1e0 - 0x170);
    w.u32(artist);
    w.pad_to(start, MHIT);

    let mut count = 0;
    let file_type = kind.name();
    for (kind, text) in [
        (TITLE, Some(track.title.as_str())),
        (ARTIST, track.artist.as_deref()),
        (ALBUM, track.album.as_deref()),
        (FILE_TYPE, Some(file_type)),
        (LOCATION, Some(track.location.as_str())),
        (ALBUM_ARTIST, track.album_artist.as_deref()),
        (SORT_ARTIST, track.sort_artist.as_deref()),
        (SORT_TITLE, track.sort_title.as_deref()),
        (SORT_ALBUM, track.sort_album.as_deref()),
        (SORT_ALBUM_ARTIST, track.sort_album_artist.as_deref()),
    ] {
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            string(w, kind, text);
            count += 1;
        }
    }
    w.close(start);
    w.set_u32(start + 12, count);
}

/// A string `mhod`, in UTF-16LE.
fn string(w: &mut Writer, kind: u32, text: &str) {
    let text = utf16(text);
    let start = w.len();
    w.tag(b"mhod");
    w.u32(MHOD as u32);
    w.u32(0);
    w.u32(kind);
    w.zeros(8);
    // Its encoding, UTF-16, its length, and an unknown 1
    w.u32(1);
    w.u32(text.len() as u32);
    w.u32(1);
    w.u32(0);
    w.raw(&text);
    w.close(start);
}

/// The master playlist: hidden, with every track, and the sorted indexes
/// the iPod browses by.
fn master(w: &mut Writer, id: u64, name: &str, tracks: &[DbTrack]) {
    let start = w.len();
    let indexed = !tracks.is_empty();
    w.tag(b"mhyp");
    w.u32(MHYP as u32);
    w.u32(0);
    w.u32(if indexed { 12 } else { 2 });
    w.u32(tracks.len() as u32);
    // 0x14: the master playlist, which is hidden
    w.u8(1);
    w.zeros(3);
    // 0x18: when it was made
    w.u32(0);
    w.u64(id);
    w.u32(0);
    // One string, and not a podcast
    w.u16(1);
    w.u16(0);
    // 0x2c: sorted by hand
    w.u32(1);
    w.pad_to(start, MHYP);
    string(w, TITLE, name);
    column_settings(w);
    if indexed {
        let keys: Vec<SortKeys> = tracks.iter().map(SortKeys::of).collect();
        for order in [
            Order::Title,
            Order::Artist,
            Order::Album,
            Order::Genre,
            Order::Composer,
        ] {
            index(w, order, &keys);
        }
    }
    for (position, track) in tracks.iter().enumerate() {
        let item = w.len();
        w.tag(b"mhip");
        w.u32(MHIP as u32);
        w.u32(0);
        // One child: its position
        w.u32(1);
        w.zeros(8);
        w.u32(track.id);
        w.pad_to(item, MHIP);
        let mhod = w.len();
        w.tag(b"mhod");
        w.u32(MHOD as u32);
        w.u32(0);
        w.u32(POSITION);
        w.zeros(8);
        w.u32(position as u32);
        w.zeros(16);
        w.close(mhod);
        w.close(item);
    }
    w.close(start);
}

/// The playlist's column settings, as iTunes writes them for each
/// playlist. iTunes doesn't show a playlist without them; the iPod ignores
/// them.
fn column_settings(w: &mut Writer) {
    let start = w.len();
    w.tag(b"mhod");
    w.u32(MHOD as u32);
    w.u32(0);
    w.u32(POSITION);
    w.zeros(24);
    for value in [0x01_0084, 5, 9, 3] {
        w.u32(value);
    }
    // Each column: its width and number, and a flag
    for (column, flag) in [
        (0x12_0001, 0),
        (0xc8_0002, 0),
        (0x3c_000d, 0),
        (0x7d_0004, 0),
        (0x7d_0003, 0),
        (0x64_0008, 0),
        (0x64_0017, 1),
        (0x50_0014, 1),
        (0x7d_0015, 1),
    ] {
        w.u32(column);
        w.u32(flag);
        w.zeros(8);
    }
    w.pad_to(start, 0x288);
    w.close(start);
}

/// The orders the iPod browses the master playlist in, with the number
/// each index is marked with.
#[derive(Debug, Clone, Copy)]
enum Order {
    Title = 3,
    Album = 4,
    Artist = 5,
    Genre = 7,
    Composer = 0x12,
}

/// What a track sorts by: each field's sort form, or else the field.
struct SortKeys {
    title: String,
    artist: String,
    album: String,
    disc: u32,
    number: u32,
}

impl SortKeys {
    fn of(track: &DbTrack) -> SortKeys {
        let key = |sort: &Option<String>, field: Option<&String>| {
            sort.as_ref().or(field).cloned().unwrap_or_default()
        };
        SortKeys {
            title: key(&track.sort_title, Some(&track.title)),
            artist: key(&track.sort_artist, track.artist.as_ref()),
            album: key(&track.sort_album, track.album.as_ref()),
            disc: track.disc_number,
            number: track.track_number,
        }
    }
}

/// An index of the tracks in one order (`mhod` 52), and its jump table
/// (`mhod` 53): where each first letter starts, and how many tracks have it.
fn index(w: &mut Writer, order: Order, keys: &[SortKeys]) {
    use crate::media::library::compare;
    use std::cmp::Ordering;
    let mut sorted: Vec<usize> = (0..keys.len()).collect();
    let within_album = |a: &SortKeys, b: &SortKeys| -> Ordering {
        a.disc
            .cmp(&b.disc)
            .then(a.number.cmp(&b.number))
            .then_with(|| compare(&a.title, &b.title))
    };
    sorted.sort_by(|&a, &b| {
        let (a, b) = (&keys[a], &keys[b]);
        match order {
            Order::Title => compare(&a.title, &b.title),
            // No composers are written, so they all have the same
            Order::Album | Order::Composer => {
                compare(&a.album, &b.album).then_with(|| within_album(a, b))
            }
            // No genres either
            Order::Artist | Order::Genre => compare(&a.artist, &b.artist)
                .then_with(|| compare(&a.album, &b.album))
                .then_with(|| within_album(a, b)),
        }
    });
    let letter = |index: usize| -> u16 {
        let key = &keys[index];
        match order {
            Order::Title => first_letter(&key.title),
            Order::Album => first_letter(&key.album),
            Order::Artist => first_letter(&key.artist),
            Order::Genre | Order::Composer => u16::from(b'0'),
        }
    };

    let start = w.len();
    w.tag(b"mhod");
    w.u32(MHOD as u32);
    w.u32(0);
    w.u32(INDEX);
    w.zeros(8);
    w.u32(order as u32);
    w.u32(sorted.len() as u32);
    w.zeros(40);
    for &position in &sorted {
        w.u32(position as u32);
    }
    w.close(start);

    let mut jumps: Vec<(u16, u32, u32)> = Vec::new();
    for (position, &track) in sorted.iter().enumerate() {
        let letter = letter(track);
        match jumps.last_mut() {
            Some((last, _, count)) if *last == letter => *count += 1,
            _ => jumps.push((letter, position as u32, 1)),
        }
    }
    let start = w.len();
    w.tag(b"mhod");
    w.u32(MHOD as u32);
    w.u32(0);
    w.u32(JUMP_TABLE);
    w.zeros(8);
    w.u32(order as u32);
    w.u32(jumps.len() as u32);
    w.zeros(8);
    for (letter, first, count) in jumps {
        w.u16(letter);
        w.u16(0);
        w.u32(first);
        w.u32(count);
    }
    w.close(start);
}

/// What a text is listed under in a jump table: its first letter or digit,
/// in capitals and without accents, or "0" for a digit or none.
fn first_letter(text: &str) -> u16 {
    use unicode_normalization::UnicodeNormalization;
    let Some(first) = text.nfd().find(|c| c.is_alphanumeric()) else {
        return u16::from(b'0');
    };
    if !first.is_alphabetic() {
        return u16::from(b'0');
    }
    let upper = first.to_uppercase().next().unwrap_or(first);
    let mut units = [0u16; 2];
    upper.encode_utf16(&mut units)[0]
}

/// Tracks grouped by a key, numbered from 1 in the order each key first
/// comes: each track's group, and each group's first track.
struct Groups {
    /// Each track's group, 0 for none
    ids: Vec<u32>,
    first: Vec<usize>,
}

impl Groups {
    fn of<K: std::hash::Hash + Eq>(
        tracks: &[DbTrack],
        key: impl Fn(&DbTrack) -> Option<K>,
    ) -> Groups {
        let mut seen: HashMap<K, u32> = HashMap::new();
        let mut groups = Groups {
            ids: Vec::with_capacity(tracks.len()),
            first: Vec::new(),
        };
        for (index, track) in tracks.iter().enumerate() {
            let id = match key(track) {
                Some(key) => *seen.entry(key).or_insert_with(|| {
                    groups.first.push(index);
                    groups.first.len() as u32
                }),
                None => 0,
            };
            groups.ids.push(id);
        }
        groups
    }
}

/// An album's or an artist's 64-bit id, which iTunes keeps for its own
/// database: made from the database's id, so it's the same at each write.
fn sql_id(database: u64, list: u64, index: usize) -> u64 {
    database ^ (list << 56) ^ (index as u64 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(dbid: u64, title: &str, album: &str, number: u32) -> DbTrack {
        DbTrack {
            dbid,
            location: format!(":iPod_Control:Music:F00:LSK{dbid}.m4a"),
            title: title.into(),
            artist: Some("Ryo Fukui".into()),
            album: Some(album.into()),
            album_artist: Some("Ryo Fukui".into()),
            kind: Some(FileKind::Aac),
            size: 1000,
            length_ms: 60_000,
            track_number: number,
            track_total: 6,
            disc_number: 1,
            disc_total: 1,
            year: 1976,
            bitrate: 256,
            sample_rate: 44_100,
            stats: Stats {
                play_count: 3,
                ..Stats::default()
            },
            gapless: Gapless {
                pregap: 2112,
                postgap: 620,
                sample_count: 2_646_000,
                data: 0,
            },
            ..DbTrack::default()
        }
    }

    #[test]
    fn a_database_is_read_back_as_it_was_written() {
        let database = Database {
            header: Header {
                id: 0x1234_5678_9abc_def0,
                platform: 1,
                language: u16::from_le_bytes(*b"en"),
                timezone_offset: 10_800,
                languages: *b"enen\x01\0\x02\0\0\0",
                ..Header::default()
            },
            master_id: 77,
            tracks: vec![
                track(11, "Early Summer", "Scenery", 2),
                DbTrack {
                    sort_title: Some("Shoka".into()),
                    kind: Some(FileKind::Alac),
                    artwork: Some((100, 5000)),
                    ..track(12, "初夏", "Scenery", 1)
                },
            ],
        };
        let bytes = write(&database, "iPod");
        let read = read(&bytes).unwrap();
        let mut expected = database.clone();
        expected.tracks[0].id = 52;
        expected.tracks[1].id = 53;
        assert_eq!(read, expected);
        // Its length is all of it
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize,
            bytes.len()
        );
    }

    #[test]
    fn indexes_sort_by_the_sort_fields() {
        let tracks = [
            DbTrack {
                sort_title: Some("Shoka".into()),
                ..track(1, "初夏", "Scenery", 2)
            },
            track(2, "Early Summer", "Scenery", 1),
            track(3, "10 Song", "Scenery", 3),
            track(4, "2 Song", "Scenery", 4),
        ];
        let keys: Vec<SortKeys> = tracks.iter().map(SortKeys::of).collect();
        let mut w = Writer::new();
        index(&mut w, Order::Title, &keys);
        let r = Reader::new(&w.bytes, "test");
        let order: Vec<u32> = (0..4).map(|i| r.u32(72 + 4 * i).unwrap()).collect();
        // 2 Song, 10 Song, Early Summer, Shoka
        assert_eq!(order, vec![3, 2, 1, 0]);
        // The jump table: 0 (2), E (1), S (1)
        let table = r.total_length(0).unwrap();
        assert_eq!(r.u32(table + 28).unwrap(), 3);
        assert_eq!(r.u16(table + 40).unwrap(), u16::from(b'0'));
        assert_eq!(r.u32(table + 48).unwrap(), 2);
        assert_eq!(r.u16(table + 64).unwrap(), u16::from(b'S'));
        assert_eq!(first_letter("Émile"), u16::from(b'E'));
        assert_eq!(first_letter("…"), u16::from(b'0'));
    }

    #[test]
    fn a_damaged_database_is_refused() {
        let bytes = write(&Database::default(), "iPod");
        assert!(read(&bytes[..100]).is_err());
        assert!(read(b"not a database").is_err());
    }
}
