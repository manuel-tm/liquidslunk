# Seeking

> **Status: in design.**

The player shows how far a track has played, but can't go anywhere in it. Seeking was in the player's design (2_player.md §4.3), and was taken out while it was built (2_player.md Appendix B): WebKit seeks a FLAC file by estimate, so a seek could play from somewhere else than the time it showed (2_player.md Appendix A). The library is almost all FLAC. This design makes seeks in FLAC exact, then gives the player its seek bar back, and a way to skip back and forward.

It builds on [2_player.md](2_player.md) and [3_gapless.md](3_gapless.md), which are complete. Here, "§4" and "phase 0" mean this document's own sections and phases, and "2_player.md §4" means the player's. Other design docs cite this one as "6_seeking.md §4" and "phase 0 of 6_seeking.md".

It covers:
- what WebKit seeks exactly, as measured;
- serving FLAC as MP4, wrapped as it's served, which WebKit seeks exactly;
- which files the player seeks in, and which it doesn't;
- the seek bar, skipping back and forward, Now Playing's scrubber, and a remembered queue that comes back where it was;
- the phases to build it in.

## 1. Summary

- **WebKit seeks FLAC by estimate, and FLAC in MP4 exactly.** On 2026-11-12, in the app, a seek to 10 seconds before the end of a FLAC track played from 3:55 before the time it showed. The same track as FLAC in MP4 or in CAF, as Apple Lossless and as AAC, and a constant-bitrate MP3, all landed where WAV did, within 10 ms. WebKit seeks WAV by arithmetic (§2).
- **So the app serves FLAC as MP4, wrapped as it's served.** A new route, `slunk://localhost/audio/:id`, serves a FLAC file as an MP4 that holds the file's own frames, byte for byte, behind a header made from an index of them. Nothing is decoded or converted, and nothing is written: the index is made once and kept in memory (§4). Other formats are served as they are.
- **What plays doesn't change.** The frames are the file's, so WebKit decodes the same FLAC, at its own bit depth and sample rate, with the same decoder as now (§4.4). Gapless playback works as it does (§5.2).
- **The player seeks only where it's exact:** FLAC, WAV, AIFF, M4A and constant-bitrate MP3. A VBR MP3, AAC in ADTS, or a FLAC file that can't be indexed doesn't seek, and its bar only shows how far it's played, as every track's does now (§4.7, §5.4).
- **The player seeks.** The progress bar is a seek bar again, dragged, clicked or moved with its keys (§5.1). ⇧⌘← and ⇧⌘→ skip back and forward by 10 seconds, as do two new items in the Controls menu (§5.3). Now Playing gets its scrubber (§5.5), and a remembered queue comes back at the time it was left at (§5.6).
- **The file page's preview** plays from the same route, so it seeks exactly in FLAC too (§5.7).
- **No new dependency, and nothing changes in the database.** Reading FLAC's frames and writing an MP4's header are a few hundred lines of Rust in the media context. One new command says whether a file seeks (§4.7).
- **Two phases:** **0** Exact seeks → **1** Seeking in the player.

## 2. What was measured

On 2026-11-12, in the app's webview, driven through WebDriver, with a scratch library.

- **The files:** a 7:07 track from `dev-library/music/`, 16-bit at 44.1 kHz, as its own FLAC file, and made from it with `afconvert`: FLAC in MP4, FLAC in CAF, Apple Lossless, AAC at 256 kbps, and WAV. Then the FLAC file's own frames, wrapped in an MP4 by a prototype of §4. And a 2:54 MP3 at a constant 192 kbps, the only long MP3 there.
- **How:** each file in its own `<audio>` element, at a volume of 0.0001 (3_gapless.md Appendix A), was seeked to some time before its end, then played to it. The settled clock, `performance.now()` minus `currentTime` (3_gapless.md Appendix A), says when the element expects the track to end, and `pause` says when it did. A seek that lands early plays for longer, and ends late by as much.
- **WAV is the reference.** Its frames all have one size, so WebKit seeks it by arithmetic. Every file ended some 80 ms after the clock said, WAV too: that's the method's own offset. What counts is how far each is from WAV.

How long after the clock said each track ended, in ms:

| File | Seeked to 30 s before its end | 60 s before | 150 s before |
|---|---|---|---|
| WAV | 76, and 86 in a second run | 81 | 76, and 89 in a second run |
| FLAC in MP4, by `afconvert` | 83 | 80 | 79 |
| FLAC in CAF, by `afconvert` | 83 | 84 | 80 |
| Apple Lossless in MP4 | 79 | 81 | 86 |
| AAC in MP4 | 81 | 79 | 79 |
| MP3, at a constant bitrate | 81 | | 80 |
| The FLAC file's frames in MP4, by the prototype (second runs) | 84 | | 88 |

- **The FLAC file** never ended on time. Seeked to 6:57, 10 seconds before its end, it played for 4:05 more, from about 3:02, so 3:55 off. Its time reached 7:07 and stayed there, with `ended` set but no event, while the rest played. Then WebKit stretched its length to 11:02, and fired `pause` and `ended`. Seeked to 6:07, 60 seconds before its end, it played for 4:27 more, from about 2:40, so 3:26 off, and its length became 10:33. In two earlier runs, seeked 10 and 60 seconds before its end, it was still playing two minutes after its time had reached the end, when the runs gave up. 2_player.md Appendix A saw a seek 10 seconds off: how far off depends on the file and the point.
- **Every other file** ended within 10 ms of WAV, at every point. At 10 seconds before the end, in two runs, the readings scattered from 108 to 301 ms, WAV's among them, so that point says nothing either way.
- **`seeked`** came 5 to 157 ms after each seek.
- **The prototype,** in Python, read the FLAC file's metadata, found its 4,599 frames, and wrote an MP4: a 20 KB header, then the file's frames as they are. Indexing took 32 ms. `afinfo` reads it as FLAC in MP4, and `afconvert` decodes it to the same PCM as the FLAC file, byte for byte. Its length first read as 427.106 s instead of 427.1066…: its movie and track headers counted time in milliseconds, and WebKit takes the length from them, not from the media's header. With the sample rate as its timescale, the length was exact (§4.4).
- **The largest file,** a 34-minute track of 143 MB, has 22,098 frames, and the prototype indexed it in 132 ms, with a 97 KB header.
- **The library's FLAC:** 431 files in `dev-library/music/`, 411 at 16 bits and 44.1 kHz and 20 at 24 bits and 48 kHz, all with blocks of 4,096 samples. Every one has a seek table, a point every 10 seconds or so, which WebKit doesn't use.

Not measured: differences under about 10 ms; what the speakers play; AIFF; a VBR MP3; AAC in ADTS (`.aac`); sample rates above 48 kHz; seeks near a track's start, which this method would take a whole track to measure.

## 3. Scope

In scope:
- exact seeks in FLAC files, wherever the app plays audio: the player, and the file page's preview;
- which files the player seeks in (§4.7);
- the seek bar: dragging it, clicking it, and its keys while it has focus;
- skipping back and forward by 10 seconds, with shortcuts and Controls menu items;
- seeking from Now Playing's scrubber;
- a remembered queue that comes back at its time;
- measuring how WebKit seeks the formats it plays (phase 0).

Out of scope:
- **Wrapping other formats.** WAV, AIFF, M4A and constant-bitrate MP3 seek exactly as they are (§2). A VBR MP3 and AAC in ADTS may not, so the player doesn't seek in them (§4.7). Wrapping them in MP4 too would take a design of its own.
- **Formats WebKit can't play.** They're still skipped (2_player.md §7.1).
- **Buttons for skipping in the sidebar.** Its row of buttons is full, and the bar is right under it.
- **Skips from Now Playing, the media keys or headphones.** Now Playing shows skips in place of previous and next (§5.5).
- **Hearing the track while the bar is dragged,** a waveform on the bar, and markers like a cue sheet's.
- **Native playback** (2_player.md §3, B). The player stays in the webview.
- **Anything written.** No cache of converted files, and no index on disk.

## 4. Making seeks exact

### 4.1 How

| | A. FLAC as MP4, wrapped as it's served | B. FLAC as CAF, wrapped as it's served | C. FLAC decoded to WAV as it's served | D. FLAC converted ahead, to Apple Lossless |
|---|---|---|---|---|
| How | An MP4 header, made from an index of the file's frames, then the frames as they are | The same, with CAF's chunks | A FLAC decoder makes the PCM of each range asked for | `afconvert` makes an `.m4a` of each track, kept in a cache |
| Seeks | Exactly (§2) | Exactly (§2) | Exactly (§2: WAV) | Exactly (§2: Apple Lossless) |
| What plays | The file's frames, through WebKit's FLAC decoder, as now | The same | PCM the app decoded | Apple Lossless, converted from the FLAC: the same samples, through another decoder |
| What it reads | The whole file once, to index it | The same | The whole file once, to index it, then what plays, to decode it | The whole file once, to convert it: seconds per track |
| What it writes | Nothing | Nothing | Nothing | A lossless copy of every track played, somewhere the app doesn't write now |
| What's served | The file, and a header of 20 to 100 KB | The same | About twice the file | About the file |
| Elsewhere | WebView2, on Windows, plays FLAC in MP4 too | Only Apple's engines play CAF | Every engine plays WAV | |
| New code | Reading FLAC's frames, writing an MP4's header | Reading FLAC's frames, writing a CAF's header | Reading FLAC's frames, a FLAC decoder (a dependency), a WAV header | A cache, keeping it in step with the library, and cleaning it up |

**Decision: A.** It's the least that's exact: no decoding, no copy, nothing written, and the prototype already does it (§2).
- B is as good, and its header is a little simpler. CAF's magic cookie for FLAC is the same `dfLa` box that MP4 has: Core Audio writes the same bytes in both. But FLAC in MP4 has a published specification, and is FLAC's standard container besides its own, where CAF is Apple's.
- C needs the same index, plus a decoder, to serve twice the bytes.
- D adds a place the app writes, which AGENTS.md lists, for no better seek than A's.

### 4.2 The audio route

- **`protocol.rs` serves `slunk://localhost/audio/:id`,** or `http://slunk.localhost/audio/:id` on Windows, beside `/files/:id`. The id is the file's.
- **A FLAC file is served as an MP4** (`audio/mp4`), as §4.3 to §4.5 describe. It's known by its first bytes, `fLaC`, after an ID3v2 tag if it has one, not by its extension, as converting reads a file's headers, not its name.
- **Any other file** is served as `/files/:id` serves it.
- **A FLAC file that can't be indexed** (§4.3) is served as it is. It plays as it does now, the player doesn't seek in it (§4.7), and the reason is logged.
- **`/files/:id` doesn't change.** It's still a file's bytes, for images, and for whatever wants the file as it is.
- **The frontend** gets `audioUrl(id)` beside `fileUrl(id)`. The player's elements and the file page's preview play from it.
- **The CSP** already allows the scheme in `media-src`.

### 4.3 Indexing a FLAC file

A FLAC file is its metadata blocks, then its frames: each frame a header, then a block of samples, then a CRC. The MP4 needs each frame's size and number of samples, so the file is indexed once, in one pass.

- **The metadata.** An ID3v2 tag before it is skipped. STREAMINFO is kept, for the MP4's header (§4.4). The rest, tags, pictures, the seek table and padding, isn't needed.
- **The frames** start where the metadata ends. A frame starts with a sync code, `0xFFF8`, or `0xFFF9` for variable block sizes. The same two bytes turn up inside frames too: in the prototype's file, 916 of its 5,515 sync codes did (§2). So a sync code starts the next frame only when:
  - its header is valid, and its CRC-8 matches;
  - its number follows the last frame's: the next frame number, or with variable block sizes, the sample after the last frame's last;
  - the bytes before it, from the last frame's start, end with a matching CRC-16.

  The prototype didn't check CRC-16s, and found the right frames: Core Audio decoded them to the FLAC file's PCM. With them, a false frame would take a false header and a false CRC-16 at once. They cost one more pass of arithmetic over the bytes, measured in phase 0 (§8).
- **The last frame** ends at the end of the file, or before an ID3v1 tag (`TAG` and 125 bytes), or before whatever follows it that isn't a frame.
- **What's kept:** where the first frame starts, and each frame's size and number of samples. For the 7-minute track, 4,599 frames.
- **It fails** when there's no STREAMINFO, when there's no frame where the metadata ends, or when a frame isn't followed by the next one, or by the end. The file is then served as it is (§4.2).

### 4.4 The MP4

The MP4 follows Xiph's specification for FLAC in the ISO base media file format, as Core Audio writes it: one audio track, each FLAC frame one sample.

```
ftyp                   M4A, compatible with mp42 and isom
moov
  mvhd                 timescale: the sample rate; duration: the samples
  trak
    tkhd               duration: the samples
    mdia
      mdhd             timescale: the sample rate; duration: the samples
      hdlr             soun
      minf
        smhd
        dinf > dref > url
        stbl
          stsd > fLaC  channels, bits per sample, sample rate
                 dfLa  STREAMINFO, as the file has it
          stts         samples per frame, in runs
          stsc, stco   frames in chunks of about a second
          stsz         each frame's size
mdat                   the file's frames, byte for byte
```

- **Lossless, as now.** The frames in `mdat` are the file's, untouched, and `dfLa` holds its STREAMINFO as it is. So WebKit decodes the same FLAC, at the file's bit depth and sample rate, with the same decoder it uses for the FLAC file now. Only the container around the frames is new: Core Audio decoded the prototype's MP4 to the same PCM as the FLAC file, byte for byte (§2).
- **The header comes first,** so the element reads it with its first range.
- **Every timescale is the sample rate.** WebKit takes the length from the movie and track headers (`mvhd`, `tkhd`), so in milliseconds, the length would be cut to the millisecond (§2). With the sample rate, it's exact, and the same as the FLAC file's.
- **A sample rate above 65,535 Hz** doesn't fit the sample entry's field, which is 16.16 fixed point. It's written as the specification says, and phase 0 plays a 96 kHz and a 192 kHz file (§8).
- **A file over 4 GB** needs 64-bit chunk offsets (`co64`) and a 64-bit `mdat` size. They're written only then.
- **No tags and no cover.** The player and Now Playing take them from the catalog.
- **No edit list.** FLAC has no encoder delay or padding, so every sample in the frames plays, and a track starts and ends on the same samples as now.

### 4.5 Serving it

- **The MP4 is never whole.** It's the header, made from the index, then the file's frames, read from the file. A range is cut in two where the header ends: what's before comes from the header, and what's after is read from the file, at the same distance from its first frame.
- **Ranges work as they do now:** a 206 with `Content-Range` over the MP4's length, and 1 MB for a range with no end (backend.md, "Serving files").
- **A request with no range** gets the whole MP4, as `/files/:id` gets the whole file.

### 4.6 The index, in memory

- **Kept, never written.** The app keeps the indexes of the last 16 files it indexed, each with its header: 20 KB for a 7-minute track, 100 KB for the 34-minute one. They're never written to disk.
- **Made once.** The element asks for many ranges of each file, and `audio_seekable` asks about it too (§4.7). Whichever comes first indexes it, and every other, even one that comes at the same time, waits for that index and uses it.
- **Kept only while it's right.** An index is kept under its file's id, path, size and modification time, so a file that's changed or relinked is indexed again. Switching libraries drops them all.
- **What it costs:** one read of the whole file before its first byte is served. Most of it is the read: the prototype, in Python, took 32 ms for 40 MB and 132 ms for 143 MB, from the system's cache. The next track is loaded ahead (3_gapless.md §3), so it's indexed while the current one plays. The first track waits for its index. Phase 0 measures that from a cold disk (§8). If it's too long, a fragmented MP4 could be served before the whole file is indexed (§9).

### 4.7 Which files seek

The player seeks only where WebKit seeks exactly, as `/audio/` serves the file. A seek anywhere else could play from minutes away (§2).

| The file, by its headers | Seeks | Why |
|---|---|---|
| FLAC that's indexed (§4.3) | Yes | It's served as MP4 (§2) |
| FLAC that can't be indexed | No | It's served as it is, and WebKit seeks it by estimate |
| WAV | Yes | §2 |
| AIFF | Yes | Its frames all have one size, as WAV's do. Phase 0 checks it |
| M4A, AAC or Apple Lossless | Yes | §2 |
| MP3 whose frames all have the same bitrate | Yes | §2 |
| MP3 whose bitrate varies | No | It has at best a coarse table of contents, so WebKit can only estimate. There's no long one to measure |
| AAC in ADTS (`.aac`) | No | It has no index at all |
| Anything else, a missing file, or one that can't be read | No | WebKit can't play it, or there's nothing to play |

- **`audio_seekable({ id })`,** a new command in `commands/media.rs`, says it for a file: `true` or `false`. The media context reads the format from the file's headers with lofty, as converting does, not from its extension, since an `.m4a` may hold AAC or Apple Lossless.
  - A FLAC file is indexed, through the same indexes in memory as the route (§4.6). So asking also indexes the current track ahead of its first range, and a FLAC file seeks exactly when, and only when, the route will serve it as MP4.
  - An MP3's frames are read through, as durations already reads them (backend.md, "Audio"), to see that every one has the same bitrate.
  - It writes nothing, and a file it can't read is `false`.
- **The player asks it for the current track only,** with `useCommand()`, when the track becomes current. A queue can hold hundreds of tracks, and only the current one seeks. `player_tracks` doesn't change.

## 5. Seeking in the player

### 5.1 The seek bar

- **The bar is a slider,** shadcn's `slider`, as the volume is, in place of the `Progress` it is now. It has the time played on its left and the track's length on its right, as now.
- **Dragging** moves the bar and the time played to where it'll land, and seeks when it's let go, with Base UI's `onValueCommitted`. While it's dragged, the bar doesn't follow the element.
- **Clicking** the bar seeks to that point.
- **Its step is a second,** so a drag lands on whole seconds: for a 4-minute track, a second is about a pixel of the bar.
- **Its keys,** while it has focus, are Base UI's: ← and → move a second, ⇧ with an arrow, Page Up and Page Down move 10 seconds (its `largeStep`), and Home goes to the start. End goes to the end, so the queue goes on, as at a track's end. Each key seeks.
- **Until the element has read the file,** the bar's length is the catalog's `durationMs`, as the length shown is now. A seek then waits for the element (§5.2).
- **For assistive tech,** the thumb is labeled "Seek", and reads as "1:23 of 4:05", as the bar does now.
- **For a track that doesn't seek,** the bar is as it is now (§5.4).
- **With nothing queued,** the player is still the one line "Nothing playing", with no bar.

### 5.2 Seeking

- **`usePlayer()` gets `seek(seconds)`,** which sets the current element's `currentTime`. A paused track stays paused, and a playing one plays on from there. It does nothing for a track that doesn't seek (§5.4).
- **Before the element has read the file's header,** a seek is kept, and made at `loadedmetadata`.
- **The time** comes from `usePlayerTime()`, which already follows `seeked`.
- **While the element seeks,** the play button shows its spinner, as while it waits for data. In §2, seeks took 5 to 157 ms.
- **Previous** goes by the time played, as now: after a seek to 0:02, it goes to the previous track. Going back to a track's start is a seek to 0, which is exact in every file, so Previous works in tracks that don't seek too, as it does now.
- **Gapless playback doesn't change** (3_gapless.md and its appendices): the two elements, the 240 ms lead, and the worker that times the handoff stay as they are. What's new is only what a seek does near a handoff:
  - a seek into a track's last second starts the watch, as playing into it does: `seeked` checks it, as `timeupdate` does;
  - a seek out of the last second stops the watch;
  - a seek during a handoff, while the next track has started and the current one plays its last moments, stops the next track, back at its start, as Pause and Previous do, then seeks;
  - a seek to the end ends the track, and the queue goes on, as at `ended`.
- **The element loaded ahead** doesn't change. It holds what's up next, from its start.
- **The lead was tuned on WebKit's start-up** for each format (3_gapless.md Appendix A), and FLAC now starts from an MP4. Phase 0 measures the transitions again, by the settled clock and by ear (§8). If they've moved, the lead is tuned again, in an appendix.

### 5.3 Skipping back and forward

- **Skip Back** goes 10 seconds back, or to the track's start. It never goes to the previous track: that's Previous.
- **Skip Forward** goes 10 seconds on. In the last 10 seconds, it goes to the end, and the queue goes on, as at a track's end.
- **Both keep playing or paused** as they were, and are seeks like any other (§5.2), so they do nothing for a track that doesn't seek (§5.4).
- **10 seconds** is ⇧ with an arrow on the bar too (§5.1), so a skip moves as far wherever it's made.
- **Shortcuts:** ⇧⌘← skips back and ⇧⌘→ skips forward, beside ⌘← and ⌘→ for Previous and Next. Held down, they skip again as the keys repeat. `usePlayerShortcuts()` handles them, as it does the other ⌘ arrows: they work while the sidebar is hidden, and they're ignored in a field, where ⇧⌘← and ⇧⌘→ select to the line's start and end. Neither is taken in the app.
- **With the bar focused,** its own keys apply. Its thumb is an `input`, which the shortcuts already leave alone as a field, and Base UI moves it by 10 seconds for ⇧ with an arrow, whether ⌘ is down or not. So ⇧⌘→ there moves the bar 10 seconds once, not twice.
- **The Controls menu** gets **Skip Forward** and **Skip Back,** after Next and Previous, with ⇧⌘→ and ⇧⌘← as their key equivalents:
  - `PlayerControl` gets `skipForward` and `skipBack`, and the provider calls its own actions for them, as for the other items;
  - `set_controls_menu` gets `seekable`, and the two items are off while nothing is queued, or while the current track doesn't seek;
  - the provider still handles the keys itself, and calls `preventDefault()`, so the menu doesn't skip a second time. A field gets them first, so they select there, as the ⌘ arrows move the caret (2_player.md Appendix E). Phase 1 checks both (§8).
- **Not from Now Playing.** Its 15-second skips would take previous and next's places (§5.5).

### 5.4 Tracks that don't seek

For a track that doesn't seek (§4.7), and while `audio_seekable` hasn't answered yet:
- **The bar** is as it is now: it shows how far the track has played, has no thumb, and can't be dragged, clicked or focused. Its tooltip says "Can't seek in this file's format".
- **`seek()`, the skips and the bar's keys** do nothing, and the Controls menu's skips are off (§5.3).
- **Now Playing** has no scrubber (§5.5).
- **A remembered queue** comes back at the start of the track (§5.6).
- **Previous** still goes back to its start (§5.2).

The file page's preview is the browser's own player, which still seeks every format, by estimate where WebKit estimates (§5.7).

### 5.5 Now Playing

- **The provider sets a `seekto` handler** with the others, each time an element starts playing (2_player.md Appendix C), and when the current track's answer from `audio_seekable` comes. Control Center's scrubber then seeks, through `seek()`. For a track that doesn't seek, `seekto` is set to `null`, so there's no scrubber.
- **It still sets none for `seekbackward` and `seekforward`.** With them, Now Playing would show 15-second skips in place of previous and next (2_player.md Appendix C). Phase 1 checks that `seekto` alone doesn't bring them back.
- **The time.** Now Playing takes it from the element, so it should follow a seek. If it doesn't, the provider sets it with `setPositionState()` after each seek.

### 5.6 Where a remembered queue comes back

As 2_player.md §5.7 first planned it:
- **The remembered queue** (`SavedQueue`) gets the time played in its current track, in seconds.
- **It's written** when the queue changes, as now, and also on pause, after a seek, and every 5 seconds while playing. Each is one `localStorage` write.
- **It comes back paused at that time,** with a seek made once the element has read the file's header (§5.2) and `audio_seekable` says the track seeks. A track that doesn't seek comes back at its start, and so does a queue remembered without a time, by an older version.
- **It's still a convenience.** When `localStorage` fails, it's ignored.

### 5.7 The file page's preview

The preview, the browser's own `<audio controls>`, plays from `audioUrl()` (§4.2), so its own seek bar is exact in a FLAC file. Nothing else about it changes: it seeks the formats that seek by estimate as it does now, by estimate, since it's WebKit's player, not the app's.

## 6. Testing

- **Rust,** in `crates/slunk/tests/`:
  - indexing: each frame's offset, size and samples, against what `afinfo` counts; a file with an ID3v2 tag before it, and one with an ID3v1 tag after it; a sync code inside a frame; a file cut off inside a frame; a stream with variable block sizes, built in the test; and a file that isn't FLAC;
  - the MP4: its boxes read back; its samples, sizes and chunk offsets against the index; and `mdat` against the file's frames;
  - ranges: inside the header, inside the frames, across where the header ends, at the end, and with no end;
  - which files seek (§4.7), with the audio fixtures: FLAC, WAV, AIFF, AAC and Apple Lossless in M4A, and the constant-bitrate MP3 do; the VBR MP3, AAC in ADTS, a FLAC file that can't be indexed, a file that isn't audio, and a missing file don't;
  - on macOS, `afconvert` decodes the MP4 to the same PCM as the FLAC file, byte for byte. The external libraries' tests already run `afconvert`.
- **A fixture with sound.** The FLAC fixtures are silence, whose frames are too small to have a sync code inside them. A few seconds of noise, as FLAC, is made by `generate.ts`, with its expected values added by hand (backend.md, "Audio").
- **The app's tests:** `/audio/:id` serves a FLAC file as MP4, another file as it is, and a FLAC file that can't be indexed as it is; and `audio_seekable`, with a test like the other commands'.
- **Vitest:** the remembered time, written and read back, and a queue remembered without one (`lib/queue.test.ts`).
- **In the app:** the checks (§8).

## 7. Phases

Each phase ends with something usable and documented, and every check in AGENTS.md's "Before you finish" passes.

**Phase 0: Exact seeks**
- The backend: indexing FLAC, the MP4's header, ranges across it, the indexes in memory, and the `/audio/:id` route, with their tests (§4.1 to §4.6, §6).
- The frontend: `audioUrl()`, for the player's elements and the file page's preview.
- The checks for phase 0 (§8), each recorded as an appendix where it changes this design.
- Docs:
  - `backend.md`: the route in "Serving files", and how a FLAC file is indexed and wrapped, and why, in "Audio";
  - `frontend.md`: the player plays `audioUrl()`, and the file page's preview seeks exactly in FLAC.
- *Exit:*
  - A FLAC file from `/audio/` seeks as exactly as its WAV does, by §2's method, at several points, at 16 and 24 bits.
  - The player plays as it did: gapless transitions, errors, Now Playing.
  - The file page's preview of a FLAC file seeks exactly.

**Phase 1: Seeking in the player**
- The backend: `audio_seekable`, with its tests (§4.7); `seekable` in `set_controls_menu`, and Skip Forward and Skip Back in the Controls menu and `PlayerControl` (§5.3).
- The seek bar, `seek()`, and seeking around a handoff (§5.1, §5.2).
- Skipping back and forward, with its shortcuts and menu items (§5.3).
- Tracks that don't seek (§5.4).
- Now Playing's `seekto` (§5.5).
- The remembered time, with its tests (§5.6).
- Docs:
  - `frontend.md`'s player: the seek bar in place of "No seeking", which tracks seek, the skips and their shortcuts, Now Playing's scrubber, and the remembered time;
  - `backend.md`: `audio_seekable`, and the menu's new items, `seekable` and the two new controls, in "The menu bar".
- Mark this design complete.
- *Exit:*
  - The bar seeks by dragging, clicking and its keys, playing and paused, in FLAC, M4A, WAV and a constant-bitrate MP3.
  - ⇧⌘← and ⇧⌘→, and the Controls menu's Skip Back and Skip Forward, skip by 10 seconds, with the sidebar hidden too.
  - In the VBR MP3 fixture, and in AAC in ADTS, the bar only shows the time played, and the skips and the menu's items are off.
  - A seek into a track's last second still hands off with no gap heard, and a seek during a handoff stops the next track.
  - Control Center's scrubber seeks, and Now Playing still shows previous and next.
  - After a restart, the queue is where it was, paused at its time.

## 8. Risks and checks

Each is checked early in the phase that needs it. Anything that changes this design is recorded as an appendix.

Phase 0:
- **The wrapped FLAC in WebKit.** §2 measured the prototype's MP4 at two points. Measure the Rust one at several, in a few tracks, at 16 and 24 bits.
- **Gapless.** Transitions between wrapped tracks, by the settled clock and by ear, as 3_gapless.md measured them. Its 240 ms lead is for WebKit's start-up, which may not be the same for MP4 (§5.2).
- **Indexing time,** for a typical track and for the 34-minute one, from the system's cache and from a cold disk, after `purge`. Check that the first track doesn't start noticeably later than it does now. If it does, see §9's fragmented MP4.
- **High sample rates.** A 96 kHz and a 192 kHz FLAC file, made with `afconvert`, play and seek (§4.4).
- **AIFF** seeks as WAV does (§4.7). If it doesn't, it doesn't seek, in an appendix.
- **Concurrent requests.** The element asks for ranges of a file while it's being indexed, and for the next track's while it plays. Each file is indexed once (§4.6).

Phase 1:
- **Now Playing,** with `seekto`: its scrubber seeks, its time follows a seek made in the app, it still shows previous and next, and it has no scrubber for a track that doesn't seek (§5.5).
- **The menu's key equivalents** (§5.3). In a field, ⇧⌘← and ⇧⌘→ select, and nothing skips. Outside one, a key skips once, not twice, and with the seek bar focused, it moves the bar 10 seconds once. As 2_player.md Appendix E found, a key equivalent has to be sent inside the app, the way AppKit routes it, since no tool can press keys without Accessibility access.
- **The handoff,** on the formats library's live tracks: seeks and skips into and out of the last second, and during a handoff (§5.2).
- **`audio_seekable`'s time** on a large FLAC file that isn't indexed yet, and that the bar doesn't wait on it visibly when a track starts.
- **Renders.** Dragging renders only the bar and its times again, as the time does now.

## 9. Alternatives considered

- **B, C and D** (§4.1).
- **A fragmented MP4,** with its frames described a few seconds at a time, so the first bytes could be served before the whole file is indexed. It's more to write, and WebKit's seeking in it would need measuring. It's the way forward if indexing takes too long (§4.6).
- **Media Source Extensions** (3_gapless.md §5): the same MP4, fragmented, fed to the element by the page, which would manage its buffer too. More for nothing more.
- **Native playback** (2_player.md §3, B): exact seeking and true gapless, at the cost of a decoder, an audio output and a native Now Playing. Exact seeking doesn't need it.
- **Correcting WebKit's estimate.** The page can't know where the audio really is: the element shows the time it was asked for.
- **Denser seek tables in the files.** WebKit doesn't use the ones they have (§2), and the app never writes the user's files.
- **Seeking FLAC by estimate, as WebKit does.** Seeking was taken out rather than have that (2_player.md Appendix B), and §2 found a seek 3:55 off.
- **Seeking every track, by estimate where WebKit estimates.** The bar would work the same everywhere, but a seek in a VBR MP3 could land seconds or minutes off (§10).
- **Whether a track seeks, in `player_tracks`.** It would read the headers, and index the FLAC, of every queued track, each time the queue's data is fetched again, which is after every save. Only the current track needs it (§4.7).

## 10. Settled questions

The first draft left these open. They were settled on 2026-11-12:
1. **Skipping back and forward:** yes, by 10 seconds, with ⇧⌘← and ⇧⌘→ and the Controls menu's Skip Back and Skip Forward (§5.3).
2. **A VBR MP3 or AAC in ADTS,** which may seek by estimate: the player doesn't seek in them (§4.7, §5.4).
3. **The remembered time:** yes, as 2_player.md first planned (§5.6).
