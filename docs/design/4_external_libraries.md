# External libraries

> **Status: being built.**

A liquidslunk library holds more music than an iPod or a phone has room for. This design adds external libraries: a selection of the library's releases, artists, tracks and tag values, converted to a format the destination plays, tagged and given covers from the catalog, and kept in step with it. There are two kinds from the start:
- **a folder library:** files laid out by a path template, for anything that reads a folder of music;
- **an iPod library:** the iPod with video (5th and 5.5th generations), written as iTunes wrote it.

This document builds on the base design, [1_introduction.md](1_introduction.md). Here, "§4" and "phase 0" mean this document's own sections and phases, and "design §5.5" means the base design's, as it does everywhere else. Code and docs cite this document as "4_external_libraries.md §4" and "phase 0 of 4_external_libraries.md".

It covers:
- what an external library is: its destination, its selection and its profile;
- how the catalog becomes tags, covers and file names;
- converting audio, with macOS's own encoders;
- folder libraries and their path templates;
- the iPod: its database, its artwork, and what it writes back;
- syncing: what's on a destination, what a sync changes, and in what order;
- the tables it adds, the commands and the pages;
- the phases to build it in.

## 1. Summary

- **An external library is a destination, a selection and a profile.**
  - The destination is a folder or an iPod.
  - The selection is a list of rules: these releases, these artists' releases, these tracks, the releases or tracks with this tag value, and exclusions.
  - The profile says what the files are: their format, and how their tags, covers and names are written.
- **The database stays the source of truth.** Tags, file names and covers are worked out from the catalog at each sync, and never read back from the destination. A change to a track, a release or an artist reaches every external library it's in at the next sync, which the user starts.
- **The user's files are still only read.** A sync writes only inside its destination, and only replaces or deletes files it wrote there itself (§8.4).
- **Conversion uses Apple's encoders,** through Core Audio's `afconvert`, which every Mac has: AAC, Apple Lossless and FLAC. Everything liquidslunk encodes goes through them, since an iPod can break on files from other encoders. Files the destination already plays, like MP3 and AAC, are copied rather than converted again. There's no MP3 or Opus encoding (§5).
- **Tags come from one mapping** (§4), the same for every format: Vorbis comments, ID3v2, MP4 atoms, and the iPod's database. It writes only what the catalog's own structure says: titles, artists, album, date, numbers, labels, catalog number, formats, media and the cover. Tag values and credit lines aren't written, since what they hold is up to the user (§4.3). The app shows a track's tags on its page before anything is written (phase 0).
- **The iPod is written as iTunes wrote it:** its `iTunesDB`, its `ArtworkDB` and thumbnails, and its music folders, by liquidslunk's own Rust code. The 5th and 5.5th generations need no checksum on their database, which is what makes that possible without iTunes (§7).
- **Each destination keeps a manifest** of what liquidslunk wrote on it (§8.1). A sync compares it with what the selection and the catalog say now, shows the plan, then carries it out: add, convert again, retag, move, remove.
- **Two new tables, in one migration:** external libraries and their rules (§9). It's the first migration since the port.
- **Four phases:** **0** The mapping → **1** External libraries → **2** Folder libraries → **3** The iPod.

## 2. Scope

In scope:
- creating, editing and deleting external libraries, and choosing what's in each;
- folder libraries, on any disk macOS mounts: an SD card, a music player in USB mass storage mode, a drive, a folder another app syncs;
- the iPod with video, 5th and 5.5th generations, with Apple's firmware;
- converting to AAC, Apple Lossless and FLAC, and copying what the destination already plays;
- tags and covers from the catalog, in each format;
- syncing changes: to tags, covers, file names, the profile and the selection;
- previewing a sync, following its progress, cancelling it, and ejecting the destination.

Out of scope:
- **Phones as devices.** An iPhone's Music app can only be filled by Finder. An Android phone is reached through MTP, which macOS doesn't mount. A phone gets its music through a folder library: an SD card, or a folder that an app on the phone syncs (§15).
- **Tag values and credit lines in exported files** (§4.3).
- **Other iPods.**
  - The iPod Classic (6th and 7th generations) and the nano from its 3rd generation sign their database with a hash worked out from the device. The iPod touch and the later nanos use schemes that only Apple writes reliably.
  - Earlier iPods (photo, mini, 4th generation and before) have other artwork formats and older databases.
  - Nothing here rules the Classic out. It would get a design of its own.
- **Reading back from a destination.** Play counts, ratings and edits made on a device or by another app aren't imported (§7.7).
- **Playlists.** liquidslunk has none yet (2_player.md §2). The iPod gets only the master playlist it needs.
- **MP3 and Opus output, and decoding APE, WavPack, Musepack, DSF, DFF and WMA.** Core Audio does none of them. A track that can't be converted is left out of the sync, with its reason (§5.2). ffmpeg could add them later, for folder libraries only (§14).
- **Loudness analysis by liquidslunk.** Core Audio adds Sound Check data to the files it makes, which the iPod uses (§7.6). Nothing else is analyzed.
- **Video, podcasts and audiobooks.**
- **Sharing a destination.** A destination belongs to one external library, and so to one liquidslunk library (§8.1).

## 3. What an external library is

### 3.1 Its destination

- **A folder library** is a folder, like `/Volumes/WALKMAN/MUSIC` or `~/Music/Car`. Its files are laid out by a path template (§6). It's kept as an absolute path. A folder that moved, or a card that mounted under another name, is found again with **Locate…**, and recognized by its manifest (§8.1).
- **An iPod library** is an iPod. It's chosen from the iPods mounted when it's created (§7.2), and recognized by its manifest wherever it mounts.
- **A destination is connected** when it's there and its manifest names this external library, or it has no manifest yet. Everything but syncing works while it isn't: editing the selection and the profile, and seeing what's in it.
- **It's never inside the library.** A folder library can't be the library's folder (the one `slunk.db` is in), be inside it, or contain it, and can't contain a registered file. liquidslunk doesn't write where the user's own files are, and the library check would list the copies as unregistered files.

### 3.2 Its selection

The selection is a list of rules. A track is in the external library when a rule includes it and no rule excludes it.

| Rule | Includes |
|---|---|
| A release | Its tracks |
| An artist | The tracks of every release the artist is a release artist of, now and later |
| A track | That track |
| A tag value | The releases with that value, whole, and the tracks with it: "Genre is Jazz", "Score ≥ 8". It's a tag filter, as the release and track lists take them ([backend.md](../development/backend.md#the-tagging-context)) |

- **Exclusions** are a release or a track, and win over every rule: "Pink Floyd, but not The Wall".
- **Rules stay live.** A release added later for an included artist, or tagged later with an included value, is in the next sync.
- **Deletes.** A deleted release, track or artist takes its rules with it, and its tracks leave the destination at the next sync.
- **An artist's appearances,** their tracks on other artists' releases, aren't included by their rule. A track rule adds one.
- **The pages say why** a track is in an external library: "On iPod, through Ryo Fukui".

### 3.3 Its profile

| Setting | Folder library | iPod library |
|---|---|---|
| Format, for lossless sources | Keep the original, FLAC, Apple Lossless or AAC | Apple Lossless or AAC |
| AAC's bitrate | 128 to 320 kbps, 256 by default | The same |
| Lossless at CD resolution | Optional: at most 16-bit and 48 kHz | Always (§7.6) |
| Path template | §6 | None: the iPod names its own files |
| Covers | Embedded, at most 600 × 600 px by default. Optionally also a `cover.jpg` in each release's folder | In the iPod's artwork database (§7.5) |

What each source becomes is in §5.2: lossy sources are copied, not converted again. How names and titles are written isn't a setting (§4.2).

## 4. From the catalog to tags

One function, `external::tags(track)`, gives a track's tags, worked out from the catalog. Everything that writes uses it: each format's tags, the iPod's database, and the path template (§6). So a file's name and its tags always agree. It's the same for every external library: the profile changes the files, not their tags.

### 4.1 A track's tags

> Changed during implementation, see [Appendix B](#appendix-b-totals-are-the-last-numbers).

| Field | From the catalog | Vorbis comment | ID3v2.4 | MP4 | iPod |
|---|---|---|---|---|---|
| Title | The track's title, with its features: "Bridges (feat. Mari, Sander)" (§4.2) | `TITLE` | `TIT2` | `©nam` | Title |
| Artist | Who performs the track, as credited, joined with ", " | `ARTIST` | `TPE1` | `©ART` | Artist |
| Artists | Each performer, then each feature | `ARTISTS`, one per name | `TXXX:ARTISTS` | `ARTISTS` | |
| Album | The release's title | `ALBUM` | `TALB` | `©alb` | Album |
| Album artist | The release's artists, as credited, joined with ", " | `ALBUMARTIST` | `TPE2` | `aART` | Album artist |
| Compilation | A various-artists release | `COMPILATION=1` | `TCMP` | `cpil` | Compilation |
| Date | The release date, as precise as it's known: `2006`, `2006-01` or `2006-01-10` | `DATE` | `TDRC` | `©day` | Year |
| Track | Its number, and the tracks on its disc | `TRACKNUMBER`, `TRACKTOTAL` | `TRCK` | `trkn` | Track |
| Disc | Its disc, and the release's last disc | `DISCNUMBER`, `DISCTOTAL` | `TPOS` | `disk` | Disc |
| Disc title | Its disc's title | `DISCSUBTITLE` | `TSST` | `DISCSUBTITLE` | |
| Label | The release's labels, in order | `LABEL`, one per label | `TPUB` | `LABEL` | |
| Catalog number | The release's | `CATALOGNUMBER` | `TXXX:CATALOGNUMBER` | `CATALOGNUMBER` | |
| Media | The release's media: CD, Vinyl | `MEDIA` | `TMED` | `MEDIA` | |
| Release type | The release's formats: Album, EP, Live | `RELEASETYPE` | `TXXX:RELEASETYPE` | `RELEASETYPE` | |
| Sort fields | Transliterations (§4.2) | `ARTISTSORT`, `ALBUMARTISTSORT`, `ALBUMSORT`, `TITLESORT` | `TSOP`, `TSO2`, `TSOA`, `TSOT` | `soar`, `soaa`, `soal`, `sonm` | The same four |
| Cover | The release's cover (§4.4) | `PICTURE` block, or `METADATA_BLOCK_PICTURE` in Ogg | `APIC`, front cover | `covr` | Artwork database |

- **Who performs a track** is `performers()`, as the track page counts them ([backend.md](../development/backend.md#tracks)): its own artists; on a regular release with none, the release's artists; on a various-artists release, only its own. Names are joined with ", ", as the app joins them: an "&" is only ever part of a name.
- **A compilation's album artist** is its release artist, which the release form says is an artist named "Various Artists".
- **Several values** are written as each format holds them: one Vorbis comment per value; ID3v2.4's null separator; one MP4 data atom per value. The iPod takes one string, so a list is joined with ", ".
- **MP4's freeform atoms** are `----:com.apple.iTunes:<NAME>`, which is how MusicBrainz Picard writes them, so other apps read them.
- **A copied file's tags are all replaced** by these, whatever it had.
- **Nothing is written for a field with no value,** like a release with no date.

### 4.2 Names and titles

> Changed during implementation, see [Appendix A](#appendix-a-sort-fields-when-a-language-isnt-known).

- **Artists are named as credited:** the name printed on the release, "ALIKA" for Alika, as the release page shows it. So an artist credited two ways shows up twice in the iPod's Artists menu.
- **Featured artists are in the title:** "Bridges (feat. Mari, Sander)", as the release page shows them. The artist field is only the performers, so the iPod's Artists menu doesn't get an entry for each combination. `ARTISTS` lists everyone.
- **Neither is a setting.** Every external library writes them the same way (§15).
- **Titles** are the main titles. Alternate titles aren't written, except as sort fields.
- **Sort fields come from transliterations.** When a name or title isn't in Latin script, and it has a Latin-script form in its own language, that form is its sort field. A form in its own language is one whose language is the name's own language with `-Latn`, like `ja-Latn` for a `ja` title. For an artist, the main name counts too.
  - So "初夏", with the alternate title "Shoka" in `ja-Latn`, sorts under S on the iPod, rather than after Z. A link that credits Ryo Fukui as "福居良" sorts by the artist's main name, "Ryo Fukui".
  - A translation, like "Early Summer" in `en`, isn't a sort field.
  - Otherwise, no sort field is written, and players sort by the name itself.
  - Scripts are told apart with ICU's script property, which `languages.rs` already uses.
- **Languages aren't written.** No tag field holds a title's language: Vorbis's `LANGUAGE` is the language sung in.

### 4.3 What isn't written

The mapping writes only what the catalog's own structure says, the same in every library. Two things are the user's own vocabulary, so they aren't written:
- **Tag values.** Which tags exist, what they're called and what they hold is up to the user: a library may have a Genre tag or not, and a Score or not. A rule from a tag's name to a file's field ("Genre is the genre") would guess. So the genre, grouping, comment and rating fields stay empty, and so do the iPod's Genres menu and its stars.
- **Credit lines.** Their roles are free text, as printed ("Mixed By", "Written-By"), so a rule from a role's spelling to a field would guess too. So the composer and the other credit fields stay empty, and so does the iPod's Composers menu.

Formats and media are lists the user manages too, but they're part of the schema, and each is written as it's named, without a rule: `RELEASETYPE=Album`, `MEDIA=CD`.

### 4.4 Covers

- **A track's cover is its release's cover.** Tracks don't have their own.
- **Folder libraries embed it** as the front cover, at most the profile's size (600 × 600 px by default, which is plenty for a player's screen), never enlarged, as a JPEG. A JPEG already within the size is embedded as it is. The profile can also write a `cover.jpg` in each folder that holds one release's tracks.
- **The iPod** gets 100 × 100 and 200 × 200 thumbnails in its artwork database (§7.5). Nothing is embedded, since its firmware doesn't read embedded covers.
- **Images are read and resized with the `image` crate:** JPEG, PNG, GIF, BMP, TIFF and WebP. A cover in AVIF, HEIC or SVG isn't exported, and the plan says so, since those need decoders outside Rust.

### 4.5 Checking it

Phase 0 builds the mapping, before anything is written. The track page shows a track's tags as a folder library would write them, with the file name the default template gives, so the mapping can be checked against real releases (§11). Its rules are tested in Rust (§10).

## 5. Converting audio

### 5.1 Choosing the encoder

| | A. Core Audio, through `afconvert` | B. ffmpeg | C. In Rust |
|---|---|---|---|
| Where it comes from | Every Mac | Installed by the user, or bundled at about 80 MB | Crates |
| Reads | FLAC, ALAC, AAC, MP3, WAV, AIFF | Everything, including APE, WavPack, DSF and WMA | symphonia: FLAC, ALAC, AAC, MP3, WAV, AIFF, Vorbis |
| Writes | AAC, with Apple's encoder; ALAC; FLAC | AAC, with its own weaker encoder; ALAC; FLAC; MP3 (LAME); Opus | FLAC and ALAC, and MP3 through LAME's bindings. No AAC encoder that can ship: fdk-aac's license doesn't allow it |
| Sound Check | `--soundcheck-generate` | No | No |
| Platforms | macOS | All | All |

On 2026-10-20, on this Mac (macOS 27), `afconvert` made AAC, ALAC and FLAC from the FLAC fixture, and ALAC at 16-bit and 44.1 kHz from a 24-bit 96 kHz file (`-d alac@44100/1`). It refused to encode MP3, Ogg Vorbis and Opus.

**Decision: A.** Every Mac has it, its AAC encoder is the best there is, and it covers both destinations, since the iPod plays AAC and ALAC. liquidslunk is macOS-only for now (design §12).

**Apple's encoders only.** Every file liquidslunk encodes, for any destination, is encoded by Core Audio, and so is every change to its sample rate or bit depth. Files from other encoders have broken music on an iPod before (§15). Converting sits behind one function, so B could be added later for MP3, Opus and the formats Core Audio can't read, but only for folder libraries: nothing another encoder makes goes to an iPod (§14).

How it runs:
- Rust starts `/usr/bin/afconvert` with its arguments, never through a shell. The webview never starts it, so it needs no shell plugin.
- It reads the source file by its absolute path, and only reads it. It writes to a temporary file in the system's temporary folder. liquidslunk then writes the tags into that file with lofty, and copies it to the destination (§8.3).
- Several files convert at once, one per core but one.
- **AAC** is Apple's constrained VBR at the profile's bitrate (`-d aac -b <bitrate> -s 2`), in an M4A.
- **Apple Lossless** is `-d alac`, and **FLAC** `-d flac`.
- **CD resolution** is 16-bit, at 44.1 kHz for sources at 44.1, 88.2 or 176.4 kHz, and at 48 kHz for the rest, converted by `afconvert` as it encodes. Phase 2 checks whether it dithers when it cuts 24 bits to 16 (§13).

### 5.2 What each source becomes

| Source | Keep the original | FLAC | Apple Lossless | AAC | On an iPod |
|---|---|---|---|---|---|
| FLAC, ALAC, WAV, AIFF | Copied | FLAC | ALAC | AAC | The profile's: ALAC or AAC |
| MP3, AAC | Copied | Copied | Copied | Copied | Copied |
| Ogg Vorbis, Opus | Copied | Copied | Copied | Copied | AAC |
| APE, WavPack, Musepack | Copied | Left out | Left out | Left out | Left out |
| DSF, DFF, WMA | Left out | Left out | Left out | Left out | Left out |

- **Lossy sources are copied,** not converted again: another lossy pass loses quality for little space. On the iPod, which can't play Ogg, they're converted to AAC. Phase 3 checks that copied MP3 and AAC files from other encoders play on the iPod. If they don't, the iPod converts every lossy source with Apple's encoder too (§13).
- **A file in the format it would become is copied:** a FLAC to a FLAC library, an ALAC to an iPod's ALAC.
- **CD resolution** converts a lossless file above it even when its format stays the same.
- **Left out:** Core Audio can't decode APE, WavPack and Musepack, and lofty can't write DSF, DFF or WMA tags (design Appendix D). The plan lists each one left out, with its reason.
- Whether Core Audio decodes Ogg Vorbis is checked in phase 2 (§13). If it doesn't, Ogg is left out on the iPod.

## 6. Folder libraries

### 6.1 Path templates

> Changed during implementation, see [Appendix B](#appendix-b-totals-are-the-last-numbers) and [Appendix C](#appendix-c-the-default-template-leaves-out-a-missing-year).

A folder library's profile has a path template. The default:

```
${album_artist}/${year} - ${album}/[${disc}-]${track_number:2} ${track_title}.${format}
```

which gives `Ryo Fukui/1976 - Scenery/01 It Could Happen to You.flac`, and on a release with two discs, `…/2-01 ….flac`.

| Variable | Its value |
|---|---|
| `${album_artist}` | The album artist (§4.1) |
| `${artist}` | The track's artist (§4.1) |
| `${album}` | The release's title |
| `${year}` | The release's year |
| `${date}` | The release's date, as precise as it's known: `2006`, `2006-01` or `2006-01-10` |
| `${cd_number}` | The track's disc |
| `${disc}` | The track's disc on a release with more than one, and empty on a release with one |
| `${cd_count}` | The release's last disc |
| `${disc_title}` | Its disc's title |
| `${track_number}` | The track's number |
| `${track_count}` | The tracks on its disc |
| `${track_title}` | The track's title, as tagged |
| `${label}` | The release's first label |
| `${catalog_number}` | The release's catalog number |
| `${format}` | The file's extension: `flac`, `m4a`, `mp3` |

- **`${name:N}`** pads a number with zeros to N digits: `${track_number:2}` is `03`.
- **`[…]` is left out** when a variable in it is empty: `[${disc}-]` only shows on a release with several discs, and `[ (${catalog_number})]` only with a catalog number.
- **`/` separates folders.** A value's own `/` is replaced (§6.2), so a title can't make a folder.
- **A template is checked when it's saved:** a variable it doesn't know, an unclosed `[`, or one that doesn't end in `.${format}` is refused. One without `${track_number}` or `${track_title}` is refused too, since it would give every track of a release the same name.
- **The album artist or the artist.** A template that starts with `${artist}`, like `${artist}/${year} - ${album}/${cd_number}_${track_number}-${track_title}.${format}`, spreads a compilation's tracks over one folder per performer. `${album_artist}` keeps each release in one folder.

### 6.2 Names on disk

Folder libraries are often FAT32 or exFAT cards, whose names are more limited than macOS's. Each folder and file name is made safe for them:
- `/ \ : * ? " < > |` and control characters become `_`;
- leading dots are dropped, since a leading dot hides a file on macOS and Linux ("...And Justice for All"), and so are trailing dots and spaces, which Windows drops;
- a name is at most 255 UTF-16 code units, FAT32's and exFAT's limit, cut at a character before its extension;
- names are in NFC, as the catalog's text is;
- a name left empty is `_`.

**Two tracks with the same path** are told apart before the extension: " (2)", " (3)", in the order of their ids. Paths are compared ignoring case and Unicode normalization, as FAT32, exFAT and APFS compare them. A track keeps the name it was given for as long as its template's result stays the same, so names don't trade places between syncs.

### 6.3 Files and folders

- **Folders** are made as they're needed. A folder is removed when a sync leaves it empty after moving or removing liquidslunk's files from it, and only then: a folder with anything else in it stays.
- **`cover.jpg`**, when the profile asks for it, is written in each folder that holds one release's tracks, and kept in the manifest like the music.
- **No AppleDouble files.** On FAT32 and exFAT, macOS stores a file's extended attributes in a `._` file beside it, which some players list as a broken track. liquidslunk writes each file's bytes itself, without copying attributes, and phase 2 checks that no `._` files appear (§13).

## 7. The iPod

### 7.1 Which iPods

- **The iPod with video**, with Apple's firmware:
  - 5th generation (2005): A002, A146, A003, A147, A452;
  - 5.5th generation (2006): A444, A446, A664, A448, A450.

  The model is read from `ModelNumStr` in `iPod_Control/Device/SysInfo`. Both generations have the same database, artwork formats and no checksum.
- **Formatted for Mac (HFS+) or for Windows (FAT32):** macOS writes both.
- **A replaced disk,** like a flash card adapter, changes nothing: nothing here depends on the disk's size.
- **Other iPods are refused:** "liquidslunk syncs only the iPod with video (5th and 5.5th generations)." An iPod with no `SysInfo` is accepted, with a warning.
- **An iPod with Rockbox** doesn't need any of this: Rockbox plays a folder library, FLAC included.

### 7.2 Finding it

- **It's a volume** in `/Volumes/` with an `iPod_Control/iTunes/` folder. The iPod mounts as one when its disk use is on. Phase 3 checks first that the user's iPod mounts that way on current macOS (§13).
- **Creating an iPod library** lists the iPods mounted then, with their model and free space.
- **Its page** looks for it when it opens, when the window comes to the front, and on Refresh. There's no watching for volumes as they mount.
- **It's recognized by its manifest** (§8.1), wherever it mounts. A new iPod has none, and gets one at its first sync.
- **Eject** unmounts it after a sync, with `diskutil eject`, so it can be unplugged.

### 7.3 What's on it

```
iPod_Control/
  Device/SysInfo             the model (read only)
  iTunes/iTunesDB            every track, and the master playlist (§7.4)
  iTunes/Play Counts         what the iPod played since the last sync (§7.7)
  iTunes/liquidslunk.json    liquidslunk's manifest (§8.1)
  Music/F00/ … Music/F49/    the music files
  Artwork/ArtworkDB          the artwork database (§7.5)
  Artwork/F1028_1.ithmb      100 × 100 thumbnails
  Artwork/F1029_1.ithmb      200 × 200 thumbnails
```

- **Music files** go into the `F` folders already there, chosen at random, under unique names like `LSKQ2M.m4a`, with lowercase extensions, which some iPods need. The iPod never shows file names: it reads only its database.
- **The manifest** sits next to `iTunesDB`, as gtkpod's own `iTunesDB.ext` did. The iPod ignores files it doesn't know.

### 7.4 The iTunesDB

- **Its shape:** little-endian chunks, each with a four-letter tag (`mhbd`, `mhit`, …), its header's length and its whole length. Strings (`mhod`) are UTF-16LE. Times are seconds since 1904, in local time.
- **What's written,** following the layout libgpod writes for these iPods, which gtkpod, Rhythmbox and Strawberry used on them for years:
  - `mhbd`, the database, version `0x30`;
  - `mhsd` type 1, the tracks: an `mhit` for each, with its strings: title, location (`:iPod_Control:Music:F12:LSKQ2M.m4a`), album, artist, album artist, file type and the four sort fields;
  - `mhsd` types 2 and 3, the playlists: only the master playlist, hidden, with every track, named after the external library, and the sorted indexes (`mhod` 52 and 53) the iPod browses by;
  - `mhsd` types 4 and 8, albums and artists, and the empty types 6 and 10, as libgpod writes them.
- **Each track's numbers:** track and disc, with their totals, year, length, size, bitrate, sample rate, file type, compilation, the gapless data and Sound Check (§7.6), and its artwork (§7.5).
- **Its ids:**
  - `dbid`, 64 bits, is persistent: the manifest keeps it, so a track stays the same track on the iPod from one sync to the next, play count and all;
  - `id` is numbered again at each write, as iTunes did.
- **No checksum.** The 5th and 5.5th generations don't check one.
- **The old database is kept** as `iTunesDB.liquidslunk-previous` before it's replaced, so an iPod that refuses the new one can be put back.
- **It's read too:** to check what's on the iPod against the manifest, to keep play counts (§7.7), and to find tracks liquidslunk didn't write (§7.8).
- **It's liquidslunk's own code,** in Rust, in `crates/slunk`. libgpod and the iPodLinux wiki are the references for the format, and libgpod's code isn't copied: it's LGPL, and liquidslunk is MIT. What the code needs of the format is written into `docs/reference/ipod.md` as it's built, so nothing depends on those references staying online (§14).

### 7.5 Artwork

- **`ArtworkDB`** has the same kind of chunks: an `mhii` for each image, with an `mhni` for each of its sizes, pointing at its place in an `.ithmb` file. Each `mhit` names its image.
- **Two sizes, for cover art,** on both generations, both RGB565 little-endian:
  - format 1028, 100 × 100: 20,000 bytes an image;
  - format 1029, 200 × 200: 80,000 bytes an image.

  Each is made from the release's cover, scaled to fit the square.
- **An image per track.** These iPods don't share an `mhii` between tracks ("sparse artwork"), so each track has its own. Phase 3 checks whether the `mhii`s of a release's tracks may still point at the same pixels in the `.ithmb` files (§13):
  - if so, each release's cover is stored once;
  - if not, it's stored once per track: about 100 KB a track, 1.2 GB for 12,000 tracks.

### 7.6 Gapless, Sound Check and resolution

- **Gapless.** The 5.5th generation plays gapless, and the 5th from firmware 1.2. It needs data in each `mhit`: the samples of silence the encoder added at the start (`pregap`) and at the end (`postgap`), the samples in the track (`samplecount`), and a flag. iTunes worked them out when it copied a file. With them missing, an iPod only played gapless once iTunes had gone through it.
  - liquidslunk reads them from the file it writes: an AAC file's encoder delay and padding from its MP4 headers, and an ALAC file's sample count, with no delay or padding.
  - A copied MP3 needs its LAME header's delay and padding, and `gapless_data`: the bytes from its first frame to the 8th before its last.
  - The live albums 3_gapless.md was tuned on are the check (§13).
- **Sound Check.** `afconvert --soundcheck-generate` adds Sound Check data to the AAC and ALAC files it makes, and liquidslunk copies it into the `mhit`, so the iPod's Sound Check setting works. A copied file has none.
- **Resolution.** Apple Lossless on the iPod is at most 16-bit and 48 kHz. Reports on 24-bit ALAC on the 5.5th generation disagree, so phase 3 checks it (§13). If it plays, the iPod's profile can offer it.

### 7.7 What the iPod writes back

- **The iPod counts** plays, skips and ratings in `iTunes/Play Counts`, an entry per track, in the database's order. iTunes added them to its library and removed the file.
- **liquidslunk keeps them on the iPod,** without importing them (§2). At each sync, it adds Play Counts' numbers to the database's, carries them over to the tracks it writes again, by `dbid`, and removes the file, so each count stays with its track. Ratings made on the iPod stay on the iPod.
- Bringing play counts into the library belongs to a design about play counts (2_player.md §2).

### 7.8 An iPod with other music

- **The first sync** of an iPod that iTunes or Finder filled finds tracks that no manifest lists. Like iTunes with an iPod from another library, liquidslunk erases them before it syncs, once the user agrees: "This iPod has 1,234 tracks from somewhere else. Erase them, then sync?" Erasing removes their files from `iPod_Control/Music/` and their entries in the database. Nothing else on the iPod is touched: notes, photos, or files kept in disk use.
- **They aren't kept** beside liquidslunk's tracks (§15). Keeping them would mean carrying over records liquidslunk doesn't fully read, and the playlists that name them.
- **Finder can undo a sync.** If Finder or Music syncs the iPod later, it replaces the database with its own. The next liquidslunk sync then finds tracks it didn't write, and offers to erase them, as above. Finder shouldn't be set to sync an iPod that liquidslunk fills.

## 8. Syncing

### 8.1 The manifest

- **Each destination keeps a manifest** of what liquidslunk wrote there: `.liquidslunk/manifest.json` at a folder library's root, and `iPod_Control/iTunes/liquidslunk.json` on an iPod.
- **It names its external library** by the UUID kept in its row (§9), and has its own format version.
- **It lists every file liquidslunk wrote** there: its path, its size and mtime as written, and its track's id, with three keys:
  - **audio:** the source file's SHA-256, the format and settings it was converted with, and the encoder's version;
  - **tags:** the tags it was given, as written (§4), so the plan can say what changed;
  - **cover:** the cover's SHA-256, and the cover settings.

  On an iPod, each track's `dbid` too.
- **It's the destination's own record,** so it stays true when the library is restored from a backup, or opened from another copy of it: the database says what should be there, and the manifest what is.
- **It's written atomically,** to a temporary file and then renamed, at the end of a sync, and at checkpoints along the way, so a sync that's interrupted carries on from there next time.
- **A destination whose manifest names another external library is refused:** "This folder belongs to another external library, Car." A folder with files and no manifest is accepted: its files are the user's, and they're never touched.

### 8.2 The plan

A sync starts by working out what it would do, and shows it before it does anything.

1. **What should be there:** the selection's tracks (§3.2), each with its path (§6, or the iPod's), its tags (§4), its cover and its audio key (§8.1). They're read in one read transaction, so the plan is one consistent picture of the library. Then the connection is let go.
2. **What is there:** the manifest, checked against the disk by each file's size and mtime, without hashing. On an iPod, against its database too.
3. **The difference:**

| Action | When | What it does |
|---|---|---|
| Add | A track should be there and isn't | Converts or copies it, tags it, and writes it |
| Convert again | Its audio key changed: a new audio file for the track, or a new format or bitrate | The same |
| Restore | Its file is missing on the destination, or not as liquidslunk wrote it | The same |
| Retag | Its tags or cover changed | In a folder library, writes a tagged copy and puts it in place of the old one. On an iPod, changes only the database |
| Move | In a folder library, its path changed: an artist was renamed | Renames it, then retags it if it needs it |
| Remove | It's there, but has left the selection or been deleted | Deletes it |

- **The preview** says it in short, "Add 42 tracks (1.8 GB), update 7, remove 3", with the tracks under each. A retag says what changed: "Title: Shoka → 初夏".
- **It lists what's left out,** and why: a format that can't be converted (§5.2), a cover that can't be read (§4.4), or a file at the track's path that liquidslunk didn't write, which is never replaced.
- **A source file missing from the library** leaves its track out of the plan, and its copy on the destination stays. The library check finds the missing file.
- **Space.** The plan estimates what the sync needs, from durations and bitrates for AAC, and from the sources' sizes for copies and lossless files. A sync that doesn't fit is refused: "It needs 3.2 GB more than the iPod has free."

### 8.3 Running it

- **Converting** runs several files at once (§5.1). **Writing to the destination** is one file at a time: an iPod's disk is slow, and on USB 2.0.
- **A file reaches its final path whole.** It's written under a temporary name in the destination, then renamed, which is atomic on the same volume. Converted files are made in the system's temporary folder first, and removed once they're copied. Whatever an earlier run left there is removed at launch.
- **In a folder library,** removals and moves go first, to make room, then additions, conversions and retags.
- **On an iPod, its database is the last thing written,** so an iPod unplugged midway still plays what it played before, and the next sync cleans up after it:
  1. the new files are copied in;
  2. the artwork database and thumbnails are written under temporary names, then the iTunesDB, then each is renamed over the old one;
  3. the files of removed tracks are deleted, and Play Counts;
  4. the manifest is written.

  When the new files wouldn't fit before the removed ones are gone, the sync runs twice: once without the removed tracks, then once with the new ones.
- **Progress** goes through a `Channel` (design §5.4): the step, the track, files done and left, and bytes written. **Cancel** stops after the file in hand, and writes the manifest for what's done.
- **One sync at a time.** The library can be browsed and edited while one runs. It works from the plan's snapshot, so an edit made meanwhile reaches the next sync.

### 8.4 What liquidslunk may write

The base design says that liquidslunk writes only next to its database and in its settings (design §2). This design adds, and AGENTS.md's rule changes with it, in phase 2:
- **Inside an external library's destination, and only there:** the files it converts or copies, `cover.jpg`s, its manifest and its temporary files, and on an iPod, the iTunesDB, the artwork database and its thumbnails.
- **It replaces or deletes only what its manifest lists,** and on an iPod, its database files, Play Counts once they're counted, and, once the user agrees, the tracks of §7.8.
- **Temporary files in the system's temporary folder,** while converting.

It still never writes to the user's own files, and never inside the library's folder (§3.1).

## 9. The database

Phase 1 adds one migration, `add_external_libraries`, with two tables.

**`external_libraries`:**

| Column | What it holds |
|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` |
| `uuid` | The id its destination's manifest names (§8.1): unique, 36 lowercase characters |
| `name` | Unique ignoring case, non-empty and trimmed |
| `kind` | `folder` or `ipod` |
| `folder` | A folder library's absolute path. NULL for an iPod |
| `format` | `original`, `flac`, `alac` or `aac`; on an iPod, `alac` or `aac` |
| `aac_kbps` | 128 to 320, 256 by default |
| `cd_resolution` | 0 or 1: lossless at most 16-bit and 48 kHz. Always 1 on an iPod |
| `path_template` | A folder library's template (§6.1). NULL for an iPod |
| `embed_covers`, `cover_px`, `cover_file` | Covers (§4.4): whether to embed them, their size (100 to 3000, 600 by default), and whether to write `cover.jpg` |
| `last_synced_at` | When its last sync ended, or NULL |
| `inserted_at`, `updated_at` | As the other main tables |

The kinds are checked by the database: a folder library has `folder` and `path_template`, an iPod has neither, and an iPod's `format` and `cd_resolution` are what it plays.

**`external_library_rules`:**

| Column | What it holds |
|---|---|
| `id` | `INTEGER PRIMARY KEY` |
| `external_library_id` | Its external library, `ON DELETE CASCADE` |
| `exclude` | 0 or 1 |
| `release_id`, `artist_id`, `track_id` | What it names, `ON DELETE CASCADE` |
| `text_tag_id`, `real_number_tag_id`, `date_tag_id` | A tag value rule's tag, one column per type, since each type has its own table, `ON DELETE CASCADE` |
| `tag_equals`, `tag_min`, `tag_max` | Its filter, as the lists' tag filters take it, checked by the app when it's saved |

- A CHECK makes each rule name exactly one thing, and a tag value rule have its filter.
- Exclusions are a release or a track only.
- A partial unique index keeps each thing named at most once per external library.
- **What's on each destination isn't a table:** it's the manifest (§8.1).
- **They're in the database, not in `settings.json`,** since rules name catalog ids, which belong to the library.
- **The one-way door.** This is the first migration since slunk's eight. slunk can't open the library after it, apart from its backups. The port is complete, so nothing else still needs slunk.
- `reference/database.md` describes both tables.

Deleting an external library deletes its rules, and leaves its destination's files where they are. With the destination connected, the delete dialog also offers to remove what liquidslunk wrote there.

## 10. The backend

A new context, the external libraries context, in `crates/slunk/src/external/`:

```
external/
  mod.rs          External: external libraries, their rules, what's in each,
                  plans and syncs
  tags.rs         a track's tags from the catalog (§4)
  template.rs     path templates and names on disk (§6)
  convert.rs      afconvert, and what each source becomes (§5)
  covers.rs       covers read, resized, as JPEG or RGB565
  write_tags.rs   tags into files, with lofty
  manifest.rs     the manifest (§8.1)
  plan.rs         the plan, from what should be there and what is (§8.2)
  sync.rs         running a plan (§8.3)
  folder.rs       a folder destination
  ipod/           finding an iPod and reading SysInfo; the iTunesDB, the
                  ArtworkDB and thumbnails, Play Counts, gapless data
  repository.rs   the records the context stores
```

- **It's wired with the others,** `External::new(store, catalog, tagging)`, as the library check joins the media context and the catalog.
- **`plan.rs` is a plain function** of what should be there and what is, so its rules are tested without files.
- **New crates:** `image` (JPEG, PNG, GIF, BMP, TIFF and WebP), and `serde_json` in `crates/slunk` for the manifest. UUIDs come from the system's random numbers.
- **Commands,** in `src-tauri/src/commands/external.rs`:
  - `external_libraries`, `external_library`, `create_external_library`, `update_external_library` and `delete_external_library`;
  - `external_library_rules` and `set_external_library_rules`, and `add_to_external_library`, for the pages' menus (§11);
  - `external_memberships`: which external libraries a release or track is in, and through which rule;
  - `external_library_tracks`: what's in one, paged, with its estimated size;
  - `exported_tags`: a track's tags as they're written, and the file name a template gives (phase 0);
  - `mounted_ipods` and `locate_external_library`;
  - `sync_plan`, `sync`, with a progress `Channel`, `cancel_sync` and `eject_external_library`.
- **Tests,** with the others in `crates/slunk/tests/`: the mapping's rules; templates and names; plans; a folder sync from start to end, in a temporary folder, with the audio fixtures; and the iTunesDB, read back after it's written and compared with a small database written by hand. The ones that run `afconvert` run on macOS only.

## 11. The frontend

- **The sidebar** gets an External libraries section, listing each one, with whether it's connected. Each opens its page.
- **`/external-libraries`** lists them, with New folder library… and New iPod library….
- **`/external-libraries/:id`** has three tabs:
  - **Overview:** its destination and whether it's connected, what's in it (releases, tracks and their size), when it was last synced, and Sync and Eject;
  - **Contents:** its rules, added with the release, artist and track pickers and a tag filter like the lists' `TagFilters`, and removed; and below them, the tracks they come to, paged like the track list;
  - **Settings:** its profile (§3.3), with the template's result for a sample track.

  **Sync** opens the plan (§8.2) in a sheet, and then its progress and its result.
- **Adding from the catalog.** "Add to external library", with a submenu of them, on the release page, the artist page and the track page, and in the release list's bulk edits, for the selected releases.
- **Where a release or track is in one,** its page says so: "On iPod", or "On iPod, through Ryo Fukui". Removing it removes its own rule, or adds an exclusion when it comes from a broader rule.
- **Tags as exported** (phase 0): on the track page, a table of the track's fields as a folder library writes them (§4.1), and the file name the default template gives. It's closed by default, and it's how the mapping is checked.

## 12. Phases

Each phase ends with something usable and documented, and every check in AGENTS.md's "Before you finish" passes.

**Phase 0: The mapping**
- `external/tags.rs` (§4), and its tests: performers on regular and various-artists releases, features in the title, credited names, sort fields from transliterations, totals, dates by precision, and nothing from tag values or credit lines.
- `external/template.rs` (§6), with its tests: variables, padding, `[…]`, names made safe, and two tracks with one path.
- `exported_tags`, and Tags as exported on the track page.
- No migration, and nothing written anywhere.
- Docs: the mapping and templates in `backend.md`, the panel in `frontend.md`.
- *Exit:* the user checks the mapping on the releases in `dev-library/music/`, the Japanese and Estonian ones among them, and what it needs changed is changed before phase 1.

**Phase 1: External libraries**
- The migration (§9), and `reference/database.md`.
- The context's external libraries, rules and memberships, with their tests.
- The commands, the sidebar's section, and the pages without Sync: the list, Overview, Contents and Settings. "Add to external library" on the release, artist and track pages and in the bulk edits, and memberships on those pages. Size estimates.
- AGENTS.md: "Not designed yet: external libraries" points to this design instead.
- *Exit:*
  - A folder library and an iPod library are created and filled from an artist, releases, tracks and a tag value, with exclusions, and each shows its tracks and their size.
  - A release deleted from the catalog leaves them.
  - The migration applies, rolls back and applies again through the CLI.

**Phase 2: Folder libraries**
- Converting (§5), covers (§4.4), writing tags (§4.1), the manifest, the plan, syncing, its progress, cancelling, and ejecting a volume.
- Phase 2's checks (§13), each recorded as an appendix where it changes this design.
- Docs: §8.4's rule in AGENTS.md and `docs/README.md`; the context in `backend.md`; the pages in `frontend.md`.
- *Exit:*
  - A folder library of 50 releases in AAC, on an SD card in FAT32 or exFAT, plays on a player with its tags and covers.
  - Renaming an artist moves and retags their files. Changing the format converts them again. Removing a release removes its files and the folders left empty. Nothing else in the folder is touched.
  - A sync that's cancelled, or whose card is pulled out, carries on at the next one.

**Phase 3: The iPod**
- Before anything else: the iPod mounts on this Mac, and liquidslunk reads its database as it is.
- The iTunesDB, read and written; the artwork database and thumbnails; gapless data and Sound Check; Play Counts; erasing other music; finding iPods.
- `docs/reference/ipod.md`: the format as liquidslunk writes it.
- Phase 3's checks (§13), each recorded as an appendix where it changes this design. The first sync is one album.
- Mark this design complete.
- *Exit:*
  - The user's iPod is synced with releases in AAC and in ALAC. It browses them by artist and album, with covers, in Japanese too, sorted by their transliterations.
  - The live albums play without gaps.
  - A second sync after some edits changes only what was edited, and the play counts made on the iPod in between are still there.

## 13. Risks and checks

Each is checked early in the phase that needs it. Anything that changes this design is recorded as an appendix.

**Phase 2:**
- **Dither.** Whether `afconvert` dithers when it cuts 24 bits to 16. If it truncates, converting moves to Core Audio's API, whose converter can be told to dither (§14), so the audio still goes only through Apple's code.
- **Ogg Vorbis.** Whether Core Audio decodes it (§5.2).
- **AppleDouble files.** No `._` files on FAT32 and exFAT after a sync (§6.3).
- **Tags as players read them.** Each format's tags, multiple values and freeform fields, as Music, Finder's Get Info and the user's player show them. Some players only read ID3v2.3: if the user's does, the profile gets an ID3 version.
- **Time.** How fast `afconvert` converts the dev library, so a first sync of 1,000 releases can be estimated, and how fast an SD card and an iPod are written.

**Phase 3:**
- **Mounting.** The user's iPod, formatted for Mac on its original disk, mounts as a volume on macOS 27 with its disk use on. Finder's own iPod sync has been unreliable since macOS 15, which is part of what this design is for, but the volume is all liquidslunk needs.
- **Copied files.** MP3 and AAC files that other encoders made, copied as they are, play on the iPod (§5.2). The check includes MP3s from LAME and an AAC file from another encoder. If any of them breaks, the iPod converts every lossy source with Apple's encoder.
- **The database.** The iPod takes the database liquidslunk writes. The first write is one album. If the iPod shows no music, the previous database is put back (§7.4), and the two are compared with one libgpod writes for the same tracks.
- **Gapless.** The live albums play through without gaps, in AAC and in ALAC (§7.6).
- **24-bit ALAC.** Whether the 5.5th generation plays it (§7.6).
- **Shared artwork.** Whether a release's tracks can share their thumbnails' pixels (§7.5).
- **Sort fields.** Whether the firmware sorts by them, and shows Japanese names where they sort.

**Throughout:**
- **Finder replacing the iPod's database** if it syncs it (§7.8).
- **Temporary space.** Converted files wait in the system's temporary folder, a few at a time, before they're copied: a few hundred MB at most.

## 14. Alternatives considered

- **libgpod, through FFI.** It's the library gtkpod, Rhythmbox and Strawberry use for iPods. It's C, with GLib, gdk-pixbuf and libplist, LGPL, so it would be bundled as a dynamic library, with its own build for macOS. These two iPods need a small part of it: a reader and writer for one database and one artwork format is less than binding and shipping it.
- **ffmpeg** (§5.1, B). It adds MP3 and Opus, and reads APE, WavPack, DSF and WMA, but it's 80 MB, or a separate install, and its AAC encoder is worse than Apple's. It's the way to those formats, behind the same function as `afconvert`, for folder libraries only: what it encodes never goes to an iPod (§5.1).
- **Core Audio's API** (ExtAudioFile and AudioConverter, through objc2's bindings) instead of `afconvert`. It's the same encoders, in the app's process, with progress within a file, and its converter's dithering can be set. `afconvert` needs nothing bound. The API is the fallback if starting a process per file costs too much, or if `afconvert` doesn't dither (§13).
- **Keeping what's on each destination in the database.** It would show what's out of date while a destination isn't connected. But it goes stale when the library is restored from a backup or opened from another copy, or the destination is changed by hand. The manifest is the destination's own record (§8.1). If a status while disconnected is wanted later, the last manifest can be cached.
- **Reading the destination's tags back** to compare. It's slow, since it opens every file, and can't tell whether a converted file still matches its source.
- **Rockbox on the iPod.** It plays FLAC and a folder library, with nothing to write but files. But the design is for the iPod as Apple made it.
- **Embedding covers for the iPod.** Its firmware doesn't read them, so they'd only take space.
- **Mapping tags and credit roles to fields.** A table from tag names (Genre, Grouping, Score) and role spellings (Composed By, Mixed By) to fields would fill the iPod's Genres and Composers menus and its stars. But which tags exist and how roles are spelled is up to each user, so the table would guess (§4.3).
- **Each tag as a field of its own name** (`SCORE=8`, `TXXX:Score`). It needs no table, but it still writes data whose meaning only the user knows, under names each format restricts differently, and the iPod has no place for it.
- **Settings for featured artists and names.** Each could be a profile setting (features in the artist, main names instead of credited ones). One way for every external library is simpler, and is how the app shows them (§4.2).
- **The profile as a JSON column.** A new setting wouldn't need a migration, but the database couldn't check any of it.

## 15. Settled questions

The first draft left these open. They were settled on 2026-10-20:
1. **Phones:** a folder library is enough (§2). There's no MTP and no iPhone sync.
2. **Other music on the iPod:** it's erased before the first sync, once the user agrees (§7.8).
3. **Formats:** AAC, Apple Lossless and FLAC, with MP3 and AAC sources copied (§5.2). Everything liquidslunk encodes goes through Apple's encoders: the user has seen music from other encoders break on an iPod (§5.1). Copied files from other encoders are checked on the iPod (§13).
4. **Featured artists:** in the title, always. It isn't a setting (§4.2).
5. **Artists' names:** as credited, always. It isn't a setting (§4.2).
6. **The user's iPod** is formatted for Mac, on its original disk, and mounts on this Mac with its disk use on. Phase 3 checks it first (§13).
7. **No denormalizing the user's own vocabulary.** Whether a library has genres or scores at all is up to its user, so no logic depends on them: tag values aren't written, nor is a Score turned into stars. By the same rule, neither are credit lines, whose roles are free text (§4.3).

## Appendix A: Sort fields when a language isn't known

- **Date:** 2026-10-20
- **Affects:** §4.2 Names and titles
- **Was:** a form in a name's own language is one whose language is the name's own language with `-Latn`, like `ja-Latn` for a `ja` title. For an artist, the main name counts too.
- **Now:**
  - A Latin-script form of a name or title is one of its alternate names or titles that's in Latin script, and whose language is tagged with the Latin script: in the same language, like `ja-Latn` for `ja`, or in any language when the name's own language isn't known. So "新しい光", with no language, sorts as "Atarashii Hikari", in `ja-Latn`, and a translation in `en` still isn't a form.
  - A name an artist is credited with sorts as the artist's main name when that's in Latin script, whatever its language. Otherwise, it sorts as the first of the artist's alternate names that's a form of it.
  - A real name isn't a form of the artist's name, so it's never a sort field.
- **Why:** in the dev libraries, track and release titles have no language, and neither have most main names, like "Ryo Fukui". As written, "新しい光" and a link crediting Ryo Fukui as "福居良" got no sort field, so the Japanese releases would sort after Z.

## Appendix B: Totals are the last numbers

- **Date:** 2026-10-20
- **Affects:** §4.1 A track's tags, §6.1 Path templates
- **Was:** a track's total, `TRACKTOTAL` and `${track_count}`, is the tracks on its disc.
- **Now:** it's the last track number on its disc, as the disc's total is the release's last disc.
- **Why:** a release cataloged in part, like one with only its tracks 1 and 4, gave "track 4 of 2". The last number is never less than the track's own.

## Appendix C: The default template leaves out a missing year

- **Date:** 2026-10-20
- **Affects:** §6.1 Path templates
- **Was:** `${album_artist}/${year} - ${album}/[${disc}-]${track_number:2} ${track_title}.${format}`
- **Now:** `${album_artist}/[${year} - ]${album}/[${disc}-]${track_number:2} ${track_title}.${format}`
- **Why:** a release with no date got a folder named " - Scenery", starting with a space and a dash.
