# The iPod

> Keep this page up to date: any change to what liquidslunk reads or writes on an iPod updates it in the same change.

This is what liquidslunk reads and writes on an iPod with video, 5th and 5.5th generations, as it's built in `crates/slunk/src/external/ipod/`. It's written down here so nothing depends on libgpod's source or the iPodLinux wiki staying online, which were the references for the formats. liquidslunk's code is its own: libgpod's isn't copied.

How a sync uses all this: [development/backend.md](../development/backend.md#syncing-an-ipod).

## The files

An iPod mounts as a volume, with its disk use on. liquidslunk recognizes it by its `iPod_Control/iTunes` folder.

```
iPod_Control/
  Device/SysInfo                    which iPod it is: read only
  iTunes/iTunesDB                   every track, and the master playlist
  iTunes/iTunesDB.liquidslunk-previous   the database before the last sync
  iTunes/Play Counts                what the iPod counted since the last sync
  iTunes/liquidslunk.json           liquidslunk's manifest
  iTunes/liquidslunk-journal.jsonl  its journal, during a sync
  iTunes/liquidslunk-tmp/           files being written, during a sync
  Music/F00/ … Music/F49/           the music files
  Artwork/ArtworkDB                 where each track's thumbnails are
  Artwork/F1028_1.ithmb             100 × 100 thumbnails
  Artwork/F1029_1.ithmb             200 × 200 thumbnails
```

- **Music files** go into the `F` folders already on the iPod, chosen at random, named `LSK` and four random letters and digits, with their extension in lowercase: `LSKQ2MX.m4a`. When there are no `F` folders, it's `F00`. The iPod never shows file names: it reads only the iTunesDB.
- **Finder** writes the database again when an iPod it synced connects, taking in its counts. It leaves one liquidslunk wrote as it is, as long as it isn't told to sync the iPod, which would replace it.
- **A playlist made on the iPod** (On-The-Go) names tracks by their place in the database, which a sync changes. liquidslunk doesn't carry it over.
- **liquidslunk writes** the music files, the iTunesDB, the ArtworkDB and its `.ithmb` files, the manifest and its journal, and the copy of the old database. It deletes the music files its manifest lists, `Play Counts` once it's counted, `.ithmb` files of formats 1028 and 1029 that the new artwork doesn't use, and, once the user agrees, the files of tracks it didn't write, in `iPod_Control/Music` only. Nothing else on the iPod is touched: notes, photos, or files kept in disk use.

## Which iPod

`iPod_Control/Device/SysInfo` is text, a `Key: value` per line. `ModelNumStr` is the model number after a letter for the region: `xA446` is model A446.

| Model | Generation | Capacity | Finish |
|---|---|---|---|
| A002, A146 | 5th | 30 GB | White, black |
| A003, A147 | 5th | 60 GB | White, black |
| A452 | 5th | 30 GB | U2 edition |
| A444, A446 | 5.5th | 30 GB | White, black |
| A664 | 5.5th | 30 GB | U2 edition |
| A448, A450 | 5.5th | 80 GB | White, black |

Any other model number is refused. An iPod without a SysInfo, or without a model number in it, is synced with a warning.

## Chunks

The iTunesDB and the ArtworkDB are both little-endian chunks. Each starts with a four-letter tag (`mhbd`, `mhit`, …), then a u32 header length. Most then have a u32 whole length, of the header and everything in the chunk; lists (`mhlt`, `mhlp`, `mhla`, `mhli`, `mhlf`) have the number of items instead, and the items follow the list's header. A header is zeros past the fields below, to its length. Offsets are in bytes, in hex, from the chunk's start.

Times are u32 seconds since 1904-01-01, in local time: liquidslunk adds the Mac's offset from UTC at that moment. 0 is a time that isn't known.

## The iTunesDB

```
mhbd                      the database
  mhsd type 1             tracks
    mhlt                  an mhit per track
      mhit                a track, with its strings (mhod)
  mhsd type 3             playlists, as for podcasts: the same as type 2
  mhsd type 2             playlists
    mhlp                  only the master playlist
      mhyp                the master playlist: its name, settings,
                          sorted indexes, and an mhip per track
  mhsd type 4             albums: mhla, with an mhia each
  mhsd type 8             artists: mhli, with an mhii each
  mhsd types 6 and 10     empty: an mhlt of 0
  mhsd type 5             smart playlists: an empty mhlp
```

This is the layout libgpod writes, as iTunes 9.2 wrote it. Reading it, liquidslunk takes the header's ids, each track's record and strings, and the master playlist's id; the rest is written again from what liquidslunk knows.

### `mhbd`, the database: 244 bytes

| Offset | Size | Value |
|---|---|---|
| 0x0c | 4 | 1 |
| 0x10 | 4 | The version: 0x30 |
| 0x14 | 4 | Its sections: 8 |
| 0x18 | 8 | The database's id |
| 0x20 | 2 | Platform: 1 for a Mac, 2 for Windows |
| 0x22 | 2 | Unknown, kept |
| 0x24 | 8 | Another id, kept, and written into every `mhit` too |
| 0x30 | 2 | The hashing scheme: 0, since these iPods check none |
| 0x46 | 2 | The language, two letters: `en` |
| 0x48 | 8 | The library's persistent id, kept |
| 0x50, 0x54 | 4 each | Unknown, kept |
| 0x58 | 20 | No hash: zeros |
| 0x6c | 4 | The time zone's offset, in seconds, kept |
| 0xa0 | 10 | The audio and subtitle languages, and unknowns, kept |

What it keeps comes from the database liquidslunk replaces. An iPod without one gets random ids, a Mac's platform, and `en`.

### `mhsd`, a section: 96 bytes

Its type at 0x0c, then its list.

### `mhit`, a track: 0x248 bytes

| Offset | Size | Value |
|---|---|---|
| 0x0c | 4 | Its strings, the `mhod`s that follow |
| 0x10 | 4 | Its id: numbered from 52 at each write, in the database's order |
| 0x14 | 4 | Visible: 1 |
| 0x18 | 4 | Its kind: `M4A ` for AAC and Apple Lossless, `MP3 ` for MP3, as four letters read big-endian, so the file has them reversed |
| 0x1c | 1, 1 | Whether an MP3 is VBR, twice: 1, 1 for MP3, 0, 0 otherwise |
| 0x1e | 1 | Compilation: 1 for a various-artists release |
| 0x1f | 1 | Rating, stars × 20: kept from the iPod |
| 0x20 | 4 | When the file was modified |
| 0x24 | 4 | Its size, in bytes |
| 0x28 | 4 | Its length, in ms |
| 0x2c, 0x30 | 4 each | Track number, and the disc's last |
| 0x34 | 4 | Year |
| 0x38 | 4 | Bitrate, in kbps |
| 0x3c | 4 | Sample rate × 0x10000 |
| 0x4c | 4 | Sound Check (below) |
| 0x50, 0x54 | 4 each | Play count, twice |
| 0x58 | 4 | Last played |
| 0x5c, 0x60 | 4 each | Disc number, and the release's last |
| 0x68 | 4 | When it was added |
| 0x70 | 8 | `dbid`: the track's persistent id (below) |
| 0x7c | 2 | Its images: 1 with artwork, 0 without |
| 0x7e | 2 | Unknown, by kind: 0xffff for AAC and MP3, 0 for Apple Lossless |
| 0x80 | 4 | Its artwork's size: the thumbnails' bytes, 100,000 |
| 0x88 | 4 | Sample rate, as a 32-bit float |
| 0x90 | 2 | Unknown, by kind: 0x33 for AAC, 0x0c for MP3, 0 for Apple Lossless |
| 0x9c | 4 | Skip count |
| 0xa0 | 4 | Last skipped |
| 0xa4 | 1 | Artwork: 1 with, 2 without |
| 0xa8 | 8 | `dbid` again |
| 0xb2 | 1 | 1: not marked unplayed, as for music |
| 0xb8 | 4 | Gapless: samples of the encoder's delay |
| 0xbc | 8 | Gapless: the samples played |
| 0xc8 | 4 | Gapless: samples of the encoder's padding |
| 0xd0 | 4 | Media type: 1, audio |
| 0xf8 | 4 | Gapless: for an MP3, the bytes from its first frame to the 8th before its last |
| 0x100 | 2 | 1 when it has gapless data |
| 0x120 | 4 | Its album's id in the albums' list |
| 0x124 | 8 | The `mhbd`'s id at 0x24 |
| 0x12c | 4 | Its size again |
| 0x134 | 8 | 0x808080808080, as libgpod writes it |
| 0x160 | 4 | Its image's id in the ArtworkDB, or 0 |
| 0x168 | 4 | 1 |
| 0x1e0 | 4 | Its artist's id in the artists' list |

The counts (play count, last played, skips, last skipped, rating) are the iPod's, carried over from the database liquidslunk replaces, with `Play Counts` added in. Everything else is written from the manifest.

### `mhod`, a string: 24 bytes

Its type at 0x0c. Then, after the header: its encoding (1 for UTF-16, 2 for UTF-8, which liquidslunk only reads), its length in bytes, 1, 0, and the string, in UTF-16LE. A track's strings:

| Type | Holds |
|---|---|
| 1 | Title |
| 2 | Location: the file's path with `:` for `/`, `:iPod_Control:Music:F12:LSKQ2MX.m4a` |
| 3 | Album |
| 4 | Artist |
| 6 | Kind: "AAC audio file", "Apple Lossless audio file", "MPEG audio file" |
| 22 | Album artist |
| 23 | Artist's sort form |
| 27 | Title's sort form |
| 28 | Album's sort form |
| 29 | Album artist's sort form |

They're the mapping every external library writes ([A track's tags](../development/backend.md#a-tracks-tags)): Title, Artist, Album, Album artist, and the sort fields, with the year from Date, Compilation, and the track and disc numbers with their totals. No genre, composer or comment is written, since tag values and credit lines aren't read.

### The master playlist

- **`mhyp`, 108 bytes:** at 0x0c its strings, at 0x10 its tracks, at 0x14 1 (the master playlist, which is hidden), at 0x1c its 64-bit id, kept from one write to the next, at 0x28 1 (one string) and at 0x2c 1 (sorted by hand).
- **Its name**, an `mhod` of type 1: the external library's name.
- **Its column settings**, an `mhod` of type 100, 0x288 bytes long, as iTunes writes it for every playlist. The iPod ignores it.
- **Five sorted indexes**, each an `mhod` of type 52 and its jump table, of type 53, that the iPod browses by:
  - by title (3), album (4), artist (5), genre (7) and composer (0x12), with their tracks' positions in the playlist, sorted;
  - each field sorts by its sort form when it has one, so a Japanese title sorts by its transliteration. They're compared as ICU's root collation compares them, with numbers by their value, then by disc, track number and title;
  - with no genres or composers, those two indexes sort as artist and album do;
  - type 52: the sort type, the count, 40 bytes of zeros, then a u32 position each;
  - type 53: the sort type, the count, 8 bytes of zeros, then for each first letter: the letter as UTF-16, a u16 0, where it starts, and how many tracks have it. The letter is the first letter or digit, in capitals and without accents, or `0` for a digit or none.
- **An `mhip` per track, 76 bytes:** at 0x0c 1, at 0x18 the track's id, then an `mhod` of type 100, 44 bytes long, with its position.

### Albums and artists

- **`mhia`, 88 bytes,** an album: at 0x10 its id, from 1, at 0x14 a 64-bit id, at 0x1c 2, then `mhod`s of type 200 (album), 201 (album artist, or else artist) and 202 (their sort form).
- **`mhii`, 80 bytes,** an artist: at 0x10 its id, from 1, at 0x14 a 64-bit id, at 0x1c 2, then an `mhod` of type 300, the artist.
- The 64-bit ids are made from the database's id, so they're the same at each write.

### Ids

- **`dbid`** is a random 64-bit number, which the manifest keeps for each file. A track keeps it from one sync to the next, even when it's converted again into a new file, so the iPod keeps its counts.
- **The track's id** is numbered again at each write, as iTunes did.

## Gapless and Sound Check

liquidslunk reads them from the file it writes, and the manifest keeps them.

- **AAC:** `----:com.apple.iTunes:iTunSMPB` holds, in hex, a 0, the encoder's delay, its padding, and the samples played: ` 00000000 00000840 0000026C 0000000000035D54`. 2112 samples of delay is Apple's encoder's.
- **Apple Lossless:** the samples played, from the same atom. No delay or padding.
- **MP3, copied:** from the LAME header in the first frame: the delay is 12 bits at 0x15 of the `LAME` tag, the padding the next 12. The samples played are the frames' samples, less the delay and padding; a CBR file counts a frame more, as iTunes did. The bytes at 0xf8 are from the first frame to the 8th before the last. An MP3 without a LAME header has no gapless data.
- **The length** is the samples played, at the sample rate, when they're known: lofty's counts the encoder's delay and padding in.
- **Sound Check:** `afconvert --soundcheck-generate` adds `----:com.apple.iTunes:iTunNORM`, ten numbers in hex. The first two are the left and right channels' Sound Check, 1000 × 10^(−gain/10), and the louder is written. A copied file without the atom has none, 0.

## The ArtworkDB and thumbnails

```
mhfd                    the database: 0x84 bytes
  mhsd 1                images: 0x60 bytes, its type a u16 at 0x0c
    mhli                an mhii each
      mhii              an image: 0x98 bytes
        mhod 2          a thumbnail: 24 bytes
          mhni          where its pixels are: 0x4c bytes
            mhod 3      the .ithmb file's name
  mhsd 2                photo albums: an empty mhla
  mhsd 3                thumbnail files
    mhlf                an mhif per format: 0x7c bytes
```

- **`mhfd`:** at 0x10 2 (iTunes 7 erases an ArtworkDB with 1), at 0x14 3 sections, at 0x1c the highest image id, at 0x30 2.
- **`mhii`:** at 0x0c its thumbnails, at 0x10 its id, from 0x64, at 0x14 its track's `dbid`, at 0x30 its size: the thumbnails' bytes.
- **`mhod` in the ArtworkDB:** its type is a u16 at 0x0c. Type 2 holds an `mhni`; type 3 is a string: its padding's length as a u8 at 0x0f, its length at 0x18, its encoding as a u8 at 0x1c (2, UTF-16LE), and the string at 0x24, padded to 4 bytes.
- **`mhni`:** at 0x0c 1, at 0x10 the format, at 0x14 the offset in its file, at 0x18 the size, at 0x1c the vertical padding, at 0x1e the horizontal padding, at 0x20 the height and at 0x22 the width, each with the padding in, as u16s. Its `mhod` 3 names its file: `:F1029_1.ithmb`.
- **`mhif`:** at 0x10 the format, at 0x14 an image's bytes.

**The formats** of the iPod with video's covers, both RGB565 little-endian: 5 bits of red, 6 of green, 5 of blue.

| Format | Size | Bytes |
|---|---|---|
| 1028 | 100 × 100 | 20,000 |
| 1029 | 200 × 200 | 80,000 |

- **A thumbnail** is the release's cover scaled to fit the square, with Lanczos3, centred on white.
- **The `.ithmb` files** hold thumbnails one after another, a format's each: `F1028_1.ithmb`, then `F1028_2.ithmb` past 256 MB.
- **A release's tracks share** their thumbnails' pixels: each track has its own `mhii`, but they point at the same place, so each cover is stored once.
- **At each write,** the files are made again: a cover that's the same as before is copied from the old files, and a new one is made from the library's.

## Play Counts

What the iPod counted since the database was written: an `mhdp` header, 0x60 bytes, with the entries' length at 0x08 and their count at 0x0c. Then an entry per track, in the database's order: plays, last played, the bookmark, and from 16 bytes long the rating, and from 28 bytes the skips at 0x14 and last skipped at 0x18. When liquidslunk writes the database again, it adds the plays and skips to each track's, takes the times and the rating, and removes the file.

## The manifest

`iPod_Control/iTunes/liquidslunk.json`, as a folder library's ([backend.md](../development/backend.md#syncing-a-folder-library)), with an `ipod` object for each track's file: its `dbid`, its kind (`aac`, `alac` or `mp3`), length, bitrate, sample rate, Sound Check, gapless data, and when it was added. The database is written again from the manifest, without reading the files.
