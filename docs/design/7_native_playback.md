# Native playback

> **Status: being built.**

The player plays in the webview, through two `<audio>` elements (2_player.md, 3_gapless.md). At best that's nearly gapless: tracks that run into each other, like The Wall's, still click where they meet. And WebKit seeks FLAC by estimate, so the player doesn't seek at all (6_seeking.md, canceled). This design moves playback into the app. Rust reads each file, decodes it and plays it, joining a track's last sample to the next one's first, and seeking to the exact sample. The player in the sidebar stays as it is, and gets its seek bar.

It builds on [2_player.md](2_player.md) and [3_gapless.md](3_gapless.md), which are complete, and takes the seeking that [6_seeking.md](6_seeking.md) planned for the player, which was canceled before it was built. Here, "§4" and "phase 0" mean this document's own sections and phases, and "2_player.md §4" means the player's. Other design docs cite this one as "7_native_playback.md §4" and "phase 0 of 7_native_playback.md".

It covers:
- what was measured in the webview, and why it can't join two tracks exactly;
- the engine: decoding, joining tracks, the output, the volume;
- the queue, which moves to Rust with its rules;
- the commands and events between the player and the webview;
- Now Playing, the media keys and the Controls menu, natively;
- seeking;
- the phases to build it in.

## 1. Summary

- **The webview can't join two tracks to the sample** (§2). With every fix tried, The Happiest Days of Our Lives still went into Another Brick in the Wall, Part 2 with 9 to 12 ms of silence, and WebKit can only time a start to within a few milliseconds. The two songs meet mid-waveform, so any gap or overlap clicks.
- **So the app plays audio itself** (§4). symphonia decodes, a decoder thread writes each track's samples right after the last one's into a ring buffer, and cpal plays them to the system's output. A track's last sample is followed by the next one's first, as on the iPod, so the join is exact by construction.
- **The engine is a crate of its own,** `crates/slunk-player/`, with no Tauri dependency, tested with a fake output that keeps the samples it's given.
- **The queue moves to Rust** (§5), with its rules and their tests, ported from `lib/queue.ts`. Once the page plays nothing, macOS can suspend it for seconds while the window is hidden. With the queue in Rust, the media keys, Now Playing, the Controls menu and the next track don't wait for it. The webview shows the player and sends it commands, and Rust says what changed through events.
- **Now Playing and the media keys are native** (§6), through `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter`. The Controls menu reads the player's state directly.
- **The player seeks** (§7), exactly, in every format it plays, with 6_seeking.md's seek bar, skips, Now Playing scrubber and remembered time.
- **Formats:** FLAC, WAV, AIFF, Apple Lossless, AAC-LC in M4A and in ADTS, MP3 and Ogg Vorbis. HE-AAC, Opus, APE, WavPack, DSF, DFF, WMA and Musepack are skipped with a toast, as WebKit's failures are now (§4.2).
- **What stays:** the sidebar's player and its controls, the queue sheet, the shortcuts and `usePlayer()`. The file page's preview stays WebKit's own player.
- **What goes:** the two `<audio>` elements, the handoff and its worker, Media Session, `set_controls_menu` and `PlayerControl`.
- **Nothing changes in the database, and no new file is written.** The page still remembers the player in `localStorage`, and hands it to Rust as the library opens (§5.5).
- **Three phases:** **0** The engine → **1** The player plays natively → **2** Seeking.

## 2. What was measured

From 2026-11-13 to 2026-11-16, in the app, driven through WebDriver, with scratch libraries in `dev-library/`.

- **How:** the settled clock (3_gapless.md Appendix A), and a recording of what the app played. A Core Audio process tap on WebKit's media process, `com.apple.WebKit.GPU`, which plays the elements' audio, recorded what it sent to the output at a volume of 0.0001. Lined up with the decoded files by cross-correlation (1.000 on music), the recording says where each source sample played, to the sample.
- **WebKit pads a FLAC file's length.** Once a FLAC file plays, its `duration` counts its last frame as a whole one, which is up to 93 ms more than its samples at 44.1 kHz with frames of 4,096 samples. The element's clock runs on through that silence. So a handoff timed by `duration` started the next track late: by 23 and 37 ms on the formats library's live tracks.
- **With only media elements playing, a start waits for a grid of 93 ms.** An element took 220 to 300 ms to start, and its sound started only on a boundary of 4,096 frames, in step with the track already playing. While a Web Audio context ran, at a `baseLatency` of 128 frames, an element started in 23 to 47 ms, on 128-frame ticks. The context cost about one percent of a core.
- **A start's time varies with the file.** The element loaded ahead holds only the file's first 64 KB, and at `play()` WebKit asks for about ten more 64 KB ranges, one at a time, before it starts. Starts took 23 to 28 ms in small test files, 29 to 40 ms in eight 10-second excerpts of the library's FLAC, and up to 47 ms in full-length tracks.
- **With all of it fixed,** by the catalog's length, a running Web Audio context and a 35 ms lead, the excerpts joined within 5 ms either way. The Wall's two songs, in `dev-library/two-songs/`, still left 404, 532 and 532 samples of silence (9 to 12 ms), since the second song started on 128-frame ticks, more slowly than the lead allowed for.
- **The two songs meet mid-waveform.** The first ends at −12,153 and the second starts at −12,265, about 37% of full scale, continuing the same wave. A gap drops it to zero and back, and an overlap doubles it. Either is a click, however short: only a join to the sample is silent. The iPod joins them so, since it decodes both itself (4_external_libraries.md §7.6).
- **That fix was measured, and not kept.** It narrowed the gap, but the click stayed.

## 3. Scope

In scope:
- playing the queue in Rust, with every join exact to the sample;
- the formats symphonia decodes (§4.2);
- the output: the system's default device, following it as it changes;
- the queue's rules, moved to Rust, and what the player remembers;
- the volume, mute, and short fades where the app cuts the audio;
- Now Playing, the media keys and headphones' buttons, natively, and the Controls menu;
- seeking: the seek bar, skipping back and forward, Now Playing's scrubber, and a remembered queue that comes back at its time.

Out of scope:
- **The file page's preview.** It's WebKit's own player, for any file, and it stays so.
- **Choosing an output device,** exclusive (hog) mode, and bit-perfect output by switching the device's sample rate (§4.4).
- **Formats symphonia doesn't decode** (§4.2), through other decoders.
- **Crossfades, an equalizer, ReplayGain and Sound Check.**
- **Play counts, history and playlists,** which are the library's data (2_player.md §2).
- **Now Playing and media keys outside macOS.** The engine itself isn't macOS-only, but only macOS is built and checked.
- **Writing to the user's files.** The engine only reads them.

## 4. The engine

### 4.1 Its crate

> Changed during implementation, see [Appendix A](#appendix-a-the-engine-as-built-and-what-phase-0s-checks-found) and [Appendix B](#appendix-b-the-player-as-built-and-what-phase-1s-checks-found).

`crates/slunk-player/` holds the engine and the queue, with no Tauri dependency, as `crates/slunk` has none. The app holds a player for the open library, and wires it to its commands, events, menu and Now Playing (§9).

It runs on three threads:
- **The decoder thread** opens files, decodes them, trims them and resamples them, and writes interleaved 32-bit float samples into a lock-free single-producer, single-consumer ring buffer (rtrb), a few seconds ahead of what plays. It's the only one that reads files.
- **The audio callback,** on Core Audio's real-time thread, through cpal, takes samples from the ring buffer, applies the volume and the fades (§4.5), and counts what it has played. It never allocates, locks, or reads a file.
- **The player's thread** takes the commands, keeps the queue, tells the decoder what to decode, and turns what the callback has played into events (§5.3).

Where a track starts in the stream is a marker beside the samples. The callback passes the marker the moment it plays the track's first sample, and the player then says the next track plays. So the player shows a track when it's heard, as now.

### 4.2 Decoding

- **symphonia 0.6,** with its default formats (FLAC, WAV, PCM, Ogg Vorbis) and `aac`, `alac`, `isomp4`, `mp3` and `aiff` added.
- **A track plays every sample of its music and nothing else:**
  - FLAC, WAV, AIFF and Apple Lossless count their samples exactly.
  - MP3: symphonia trims the encoder's delay and padding by the LAME header. A file without one plays as decoded.
  - AAC in M4A: symphonia doesn't trim it. The engine trims it by `----:com.apple.iTunes:iTunSMPB`, as an iPod sync reads it (reference/ipod.md, "Gapless and Sound Check"). A file without it plays as decoded.
  - AAC in ADTS has no delay recorded, and plays as decoded.
  - symphonia 0.6.1 reports the first and last packets' lengths untrimmed, though it trims them, so the engine counts the frames it decodes.
- **What it can't decode** is a failure, like a missing file (§5.4): HE-AAC, which symphonia refuses, Opus, APE, WavPack, DSF, DFF, WMA and Musepack. The library has none of them but the test fixtures.
- **Channels:** mono plays on both channels. More than two are mixed down to stereo.
- **Files are read from disk,** read-only, by the path the media context resolves. Nothing goes through the file protocol.

### 4.3 Joining tracks

> Changed during implementation, see [Appendix B](#appendix-b-the-player-as-built-and-what-phase-1s-checks-found).

- **The decoder writes the next track's first sample right after the current one's last.** It opens the next track while the current one still has a few seconds to decode, so its first samples are ready in time.
- **When what's up next changes** before its marker has played (Play next, a move, a removal, shuffle, repeat), the decoder decodes the new one. The callback drops the old one's samples, unheard, and the new one follows at the join. A change in a track's last moments, closer to its end than the new track takes to start decoding, leaves a short gap.
- **Repeat one** joins a track to its own start.
- **Next, Previous, a jump and a new queue** drop what's in the buffer. The track that plays fades out (§4.5), and the new one starts from its first sample.
- **Sample rates.** The output runs at the device's rate (§4.4). A file at another rate is resampled to it with rubato. One resampler runs across consecutive tracks at the same rate, so nothing is flushed at a join between them, and a join across rates is exact in time.

### 4.4 The output

> Changed during implementation, see [Appendix A](#appendix-a-the-engine-as-built-and-what-phase-0s-checks-found) and [Appendix B](#appendix-b-the-player-as-built-and-what-phase-1s-checks-found).

- **cpal 0.18, on the default output device.** Its stream follows the system's default device as it changes: headphones unplugged, AirPods connected. cpal says so, and the stream plays on. With no output device left, it pauses, and so does the player.
- **At the device's own sample rate.** Asked for another rate, cpal would switch the device's rate, for every app, and a switch drops audio, so a join across it couldn't be gapless. So the stream takes the device's current rate, and the engine resamples what differs (§4.3). A 44.1 kHz file on a 44.1 kHz device reaches the output as it is, but for the volume.
- **The buffer** is the device's own size. The time shown is what has been played, less the output's latency, from cpal's timestamps.

### 4.5 The volume, and fades

- **The volume and mute** are a gain in the callback. A change moves over 10 ms, never in one step, so it doesn't click. The system's volume is the system's.
- **Where the app cuts the audio, it fades.** Pausing fades out, then stops, and playing fades back in. Next, Previous, a jump and a seek fade the old position out. A seek fades the new position in.
- **The fade's length is a setting:** Off, 10 ms, 50 ms or 200 ms, and 10 ms until it's changed. It's chosen in the Controls menu (§6.4), and remembered for every library, with the volume (§5.5). Off cuts the audio where it is, which can click.
- **A track's own start isn't faded.** It plays from its first sample, as the album has it.

### 4.6 In the background

The callback runs on Core Audio's real-time thread, and the decoder works seconds ahead. App Nap shouldn't slow either one while audio plays. Phase 0 checks it, with the window hidden and minimized (§12). If the decoder falls behind, the engine holds a `NSProcessInfo` activity while it plays.

## 5. The queue

### 5.1 Why it moves to Rust

- With the audio in Rust, the page plays nothing. macOS can suspend a page that plays nothing audible while its window is hidden: up to 13 seconds at a time, as 3_gapless.md Appendix A found.
- With the queue in the page, a media key, Now Playing's buttons and the Controls menu would wait for it. So would choosing the next track.
- With the queue in Rust, the player works without the page. The page shows it and sends it commands, and catches up from the player's state when it runs again.

### 5.2 Its rules

- **`lib/queue.ts`'s rules, ported as they are,** to `crates/slunk-player/src/queue.rs`:
  - entries keyed, so a track can be queued twice;
  - Previous's 3 seconds;
  - repeat off, all and one;
  - shuffle, with the order before it;
  - what's up next;
  - stopping once every entry has failed in a row;
  - restoring a remembered queue.
- **The tests are ported too,** from `lib/queue.test.ts`. Shuffle's seed comes from the player, and each test gives its own.
- **What's the engine's now:** a track that ends, a join, and a start are the engine's events, not an element's. The reducer's `ended`, `handoff` and `started` are the player's own steps.

### 5.3 Commands and events

> Changed during implementation, see [Appendix B](#appendix-b-the-player-as-built-and-what-phase-1s-checks-found).

The commands, in `commands/player.rs`:

| Command | Does |
|---|---|
| `player_state` | The player's state, for the page as it mounts |
| `player_restore({ queue, volume, muted, shuffle, repeat, fade })` | What the page remembers, as the library opens (§5.5) |
| `player_play({ trackIds, start? })` | A new queue, playing from `start`, or shuffled from a random track |
| `player_play_next({ trackIds })`, `player_add_to_queue({ trackIds })` | Right after the current track, or at the end; with nothing queued, a queue, paused |
| `player_toggle`, `player_set_playing({ playing })` | Play and pause |
| `player_next`, `player_previous`, `player_jump({ key })` | Moving in the queue |
| `player_move({ key, by })`, `player_remove({ key })`, `player_clear` | Changing it |
| `player_set_shuffle({ shuffle })`, `player_set_repeat({ repeat })` | Shuffle and repeat |
| `player_set_volume({ volume, muted })` | The volume, in hundredths, as now |
| `player_set_fade({ fade })` | The fade's length (§4.5): `off`, `10`, `50` or `200` |
| `player_seek({ seconds })` | Phase 2 (§7) |

The events, typed by tauri-specta like `PlayerControl` is now:
- **`PlayerState`,** whenever anything in it changes: the entries in the order they play, the current one, whether it plays, whether it waits for data, shuffle, repeat, the volume, mute and the fade's length.
- **`PlayerTime`,** four times a second while a track plays, and at once after a change of track, a pause or a seek: the time played, and the track's length.
- **`PlayerFailed`,** when a track can't play: its id, its file's path, and why.

A command returns once the player has taken it. What it changed comes back through `PlayerState`, so the page has one source of truth. What the page shows of each track still comes from `player_tracks`.

### 5.4 Tracks that can't play, and tracks deleted

- **A track that can't play,** because its file is missing or its format can't be decoded, is skipped, and `PlayerFailed` shows its toast, as now (2_player.md §7.1).
- **Tracks deleted from the catalog** leave the queue. The commands that delete tracks tell the player, and it drops them. When the current one goes, the next one plays.

### 5.5 What it remembers

> Changed during implementation, see [Appendix B](#appendix-b-the-player-as-built-and-what-phase-1s-checks-found).

- **The page remembers the player in `localStorage`, as now** (2_player.md §5.7), under the same keys:
  - **for each library:** the queue, its tracks in the order they play, the current one, and while shuffled, their order before; and from phase 2, the time played;
  - **for every library:** the volume, mute, shuffle, repeat, and the fade's length, which is new.
- **The page writes it from the player's events:** the queue at each `PlayerState`, and from phase 2, the time as 6_seeking.md §5.6 has it.
- **As the library opens,** the player starts empty. The provider mounts, reads `localStorage`, and hands it to the player with `player_restore`. The player comes back paused at the current track's start, or from phase 2, at its time. A queue an older version remembered comes back as it does now.
- **Changes made while the page is suspended** are written when it runs again: a track that ends, a media key, the menu, the time played. If the app quits or crashes before then, the queue comes back as it was when the page last ran. That's accepted, since it's only a convenience.
- **Not a file beside the settings,** and not the database: it's not the library's data, and changing the schema is a one-way door (2_player.md §5.7).

### 5.6 The library

The player belongs to the open library. Switching or closing it stops the player, and the page has already remembered it. Opening a library starts an empty player, which the page restores (§5.5). At Welcome, nothing plays, and the Controls menu's items are off.

## 6. Now Playing, the media keys and the Controls menu

### 6.1 Now Playing

> Changed during implementation, see [Appendix B](#appendix-b-the-player-as-built-and-what-phase-1s-checks-found).

- **Set from Rust,** through objc2-media-player, on the main thread.
- **What it shows:**
  - the title;
  - who performs it, as the player names them, joined with commas;
  - the release;
  - the cover, from its file;
  - the length, the time played and the rate, 1 or 0, which macOS uses to move the time itself.
- **When it's set:** at each change of track, when the track's data changes (a save), at play and pause, and after a seek.
- **Its state:** `playbackState` is Playing, Paused or Stopped. macOS needs it set whenever playback starts or stops, to show the app as playing. It's cleared when nothing is queued.

### 6.2 Its buttons and the media keys

- **`MPRemoteCommandCenter`:** play, pause, toggle, next and previous call the player.
- **Off:** the skips and seeks forward and back, so Now Playing shows previous and next.
- **From phase 2:** `changePlaybackPositionCommand`, the scrubber, seeks (§7).
- **The keyboard's media keys and headphones' buttons** reach the same handlers while liquidslunk played last.

### 6.3 WebKit's Now Playing

The page sets no Media Session, and plays no element of the player, so WebKit shouldn't register with Now Playing for it. The file page's preview still does while it plays. One sound at a time pauses the player then (§8), and when the player plays again, it sets its info and state again. Phase 1 checks both (§12).

### 6.4 The Controls menu

> Changed during implementation, see [Appendix B](#appendix-b-the-player-as-built-and-what-phase-1s-checks-found).

- **The menu reads the player's state directly,** and its items call the player. `set_controls_menu` and the `PlayerControl` event go.
- **Its items and key equivalents stay as they are.** Phase 1 adds Fade, a submenu after Repeat, like it: Off, 10 ms, 50 ms and 200 ms, the current one checked (§4.5). Phase 2 adds Skip Back and Skip Forward (§7).
- **The page's shortcuts** call the commands. WebKit still gives the page a key first (2_player.md Appendix E).

## 7. Seeking

As 6_seeking.md §5 planned it, with what changes:
- **Every format seeks exactly,** so there are no tracks that don't seek (6_seeking.md §5.4 goes), and `seekable` doesn't join `set_controls_menu`:
  - FLAC by its frame headers, narrowed by its seek table when it has one, then decoded from the frame and dropped up to the sample;
  - MP3 by scanning its frames, which is exact, but slower the further it goes. Phase 2 measures it on the longest MP3 it can make;
  - M4A by its sample tables, from one packet earlier for AAC, whose first packet after a seek lacks its overlap;
  - WAV and AIFF by arithmetic;
  - ADTS by scanning;
  - Ogg Vorbis by its pages.
- **A seek** drops the buffer, and the decoder starts again at the sample. The old position fades out and the new one fades in (§4.5).
- **The join with the next track doesn't change.** The decoder reaches it again, and joins it as before.
- **The seek bar, the skips and their shortcuts** (⇧⌘← and ⇧⌘→, by 10 seconds) are as 6_seeking.md §5.1 and §5.3 have them. So are the Controls menu's Skip Back and Skip Forward, and Now Playing's scrubber (§6.2).
- **The remembered time** is as 6_seeking.md §5.6 has it, in `localStorage` (§5.5): written on pause, after a seek, and every 5 seconds while playing.
- **Previous** goes by the time played, as now.

## 8. The frontend

> Changed during implementation, see [Appendix B](#appendix-b-the-player-as-built-and-what-phase-1s-checks-found).

- **`PlayerProvider` keeps `usePlayer()` and `usePlayerTime()`,** so the components don't change, but the seek bar (phase 2). It restores the player as it mounts (§5.5), listens for `PlayerState` and `PlayerTime`, and calls the commands. It still fetches `player_tracks` for what the player shows.
- **Remembering** stays in the page: `localStorage`, written from the events (§5.5).
- **`waiting`** is the engine's: true while the decoder hasn't the data to play. It shows as now.
- **Toasts:** `PlayerFailed` shows "Can't play …" with the file's path, as now.
- **One sound at a time:** the provider still listens for `play` on the document. A preview that starts pauses the player, and when the player starts, it pauses any element playing in the page.
- **Shortcuts** work as now, through the commands.
- **What goes:**
  - the two `<audio>` elements, `elements.ts` and `ticks.worker.ts`;
  - the queue's reducer and its tests, which move to Rust (§5.2);
  - Media Session;
  - `setControlsMenu` and the `PlayerControl` listener.

## 9. The backend

- **`crates/slunk-player/`:**
  - the queue (`queue.rs`);
  - the decoder, with its trimming and resampling;
  - the ring buffer and the callback;
  - the output;
  - the player, which ties them together;
  - their tests.

  It knows a track by its id, and asks the app for its file's path through a function it's given, so it doesn't depend on `crates/slunk`.
- **`src-tauri/`:**
  - the player, created for each library as it opens and kept in Tauri's state;
  - `commands/player.rs`;
  - the events;
  - Now Playing and the remote commands (`now_playing.rs`);
  - the Controls menu, from the player's state;
  - the delete commands telling the player what's gone.
- **New dependencies:**
  - symphonia 0.6, with the features in §4.2;
  - cpal 0.18;
  - rtrb, for the ring buffer;
  - rubato, pinned exactly, since its API changes from one major version to the next;
  - objc2-media-player, and objc2-app-kit for the cover's `NSImage`.

## 10. Testing

> Changed during implementation, see [Appendix A](#appendix-a-the-engine-as-built-and-what-phase-0s-checks-found) and [Appendix B](#appendix-b-the-player-as-built-and-what-phase-1s-checks-found).

- **Rust,** in `crates/slunk-player/tests/`, with a fake output that keeps every sample it's given:
  - **the queue:** `lib/queue.test.ts`'s cases, ported;
  - **joins:** two tracks played in order come out as one stream, equal to their decoded samples end to end, sample for sample, in WAV, FLAC, AIFF and Apple Lossless. A FLAC file whose last frame is short is among them, the case WebKit padded;
  - **trimming:** an MP3 with a LAME header and an AAC file with `iTunSMPB` come out at their sample counts, delay and padding trimmed;
  - **changes to what's up next:** before the decoder starts the next track, after it has, and in its last moments;
  - repeat one;
  - a track that fails between two that play;
  - a 48 kHz track between two at 44.1 kHz, resampled, with no gap;
  - **the volume and fades:** no step between two samples larger than the music's own; each fade as long as its setting; and with fades off, a cut where it's made;
  - **seeking** (phase 2): a seek to any sample plays what decoding from the start plays from there, in every format.
- **Fixtures with sound.** The audio fixtures are silence, and a join can't be checked on silence. `generate.ts` makes a few seconds of a tone and of noise, in each format, cut into consecutive tracks, with afconvert, as it makes the other fixtures. The tests that only need PCM write their own WAV files.
- **The app's tests:** the commands, as the other commands are tested; `player_restore`, with a queue the page saved and one an older version saved; and tracks deleted.
- **In the app:** the checks (§12). The join of the two songs in `dev-library/two-songs/` is recorded with a process tap on the app's own process, and lined up with the decoded files as in §2.

## 11. Phases

> Changed during implementation, see [Appendix A](#appendix-a-the-engine-as-built-and-what-phase-0s-checks-found).

Each phase ends with something usable and documented, and every check in AGENTS.md's "Before you finish" passes.

**Phase 0: The engine**
- `crates/slunk-player/`: decoding and trimming, the ring buffer and the callback, joins, resampling, the volume and fades, the output and its device changes, with their tests (§10). The queue isn't in it yet.
- An example that plays files in order, for the checks: `cargo run -p slunk-player --example play -- a.flac b.flac`.
- Phase 0's checks (§12), each recorded as an appendix where it changes this design.
- Nothing changes in the app yet.
- Docs: `backend.md`'s Stack and Structure, and how the engine plays, in a section of its own.
- *Exit:*
  - The two songs play through the example with no click heard, and a recording of the output joins them to the sample.
  - A 44.1 kHz track into a 48 kHz one joins with no gap.
  - Playback goes on with the headphones unplugged and AirPods connected.
  - Every file in `dev-library/music/` decodes to the sample count the catalog has.

**Phase 1: The player plays natively**
- The queue in Rust, with its tests; the player for each library; the commands and events; remembering in `localStorage`, and `player_restore`; tracks deleted.
- Now Playing and the remote commands; the Controls menu from the player, with its Fade submenu.
- The provider on the commands and events, and what goes (§8).
- Phase 1's checks (§12).
- Docs:
  - `frontend.md`'s player: how it plays, and Now Playing. "Nearly gapless" goes;
  - `backend.md`: the commands, the events, the player, the menu bar, and what it reads.
- *Exit:*
  - Everything 2_player.md's player does, played natively: the queue, shuffle, repeat, failures, the remembered queue, the volume, the shortcuts and the Controls menu.
  - Now Playing shows the track and its cover, and its buttons and the media keys work, with the window hidden for minutes too.
  - The two songs join with no click heard.
  - An Ogg Vorbis track plays.

**Phase 2: Seeking**
- `player_seek`, the seek bar, the skips and their menu items, Now Playing's scrubber, and the remembered time, with their tests (§7).
- Docs: `frontend.md`'s player, and `backend.md`'s commands and menu bar.
- Mark this design complete.
- *Exit:*
  - The bar seeks by dragging, clicking and its keys, playing and paused, in every format.
  - ⇧⌘← and ⇧⌘→ and the menu's items skip by 10 seconds, with the sidebar hidden too.
  - Control Center's scrubber seeks, and Now Playing still shows previous and next.
  - A seek into a track's last seconds still joins the next one exactly.
  - After a restart, the queue is where it was, paused at its time.

## 12. Risks and checks

> Changed during implementation, see [Appendix A](#appendix-a-the-engine-as-built-and-what-phase-0s-checks-found) and [Appendix B](#appendix-b-the-player-as-built-and-what-phase-1s-checks-found).

Each is checked early in the phase that needs it. Anything that changes this design is recorded as an appendix.

Phase 0:
- **The join,** recorded (§10), and by ear, on the two songs and the formats library's live tracks.
- **The library's files.** Every file in `dev-library/music/` opens and decodes to its sample count. Any that doesn't is looked into before phase 1.
- **Device changes:** unplugging the headphones, connecting AirPods, and another app changing the device's sample rate. Note what cpal's default-output stream does with each.
- **CPU and energy,** in Activity Monitor, against the webview's: 3_gapless.md Appendix C measured 2 to 6% of a core while it handed off.
- **In the background:** with the window hidden and minimized, the decoder stays ahead (§4.6).
- **Latency:** how long Play takes to be heard, and how far the time shown is from the sound.

Phase 1:
- **Now Playing from a debug build,** whose Info.plist has no bundle identifier, and from a release build: the name, the icon and the cover it shows. Then the media keys, after another app has played, and after the file page's preview has.
- **WebKit leaves Now Playing alone** once no element of the player plays (§6.3).
- **The page suspended.** With the window hidden for minutes, keys and the menu act at once and the queue goes on. When the window shows again, the page catches up from `PlayerState`.
- **A library switched, and the app quit, while it plays:** the player comes back paused, where the page last remembered it.

Phase 2:
- **Seek times,** in the longest MP3 and FLAC files there are. An MP3 scans its frames, so a seek near its end takes longest.
- **Now Playing's scrubber,** with the time it shows after a seek made in the app.
- **A seek into a track's last second,** and one made while the next track's samples are already in the buffer.

## 13. Alternatives considered

- **Staying in the webview, with §2's fix.** The excerpts joined within 5 ms either way, and the two songs at 9 to 12 ms, and the click stayed.
- **A splicer in Web Audio.** Both elements would play through `MediaElementAudioSourceNode`s, which needs CORS on the file protocol, into an AudioWorklet that joins the first one's last sample to the second one's first, a few milliseconds behind. It could be exact, but WebKit's media pipeline would still decide when samples arrive. It's another workaround inside WebKit, as 6_seeking.md's was, and seeking would stay by estimate.
- **AVAudioEngine,** with an `AVAudioPlayerNode` scheduling files back to back. That joins to the sample, with Apple's decoders, but it has no Ogg Vorbis. Its FLAC goes through Core Audio, whose handling of raw FLAC is what WebKit seeks by estimate. And it ties the engine to macOS.
- **rodio.** Its queue joins sources without silence. But it decodes inside the audio callback, its time isn't corrected for the output's latency, and it's on symphonia 0.5, which trims only MP3.
- **Decoding in Rust and playing through Web Audio.** Every sample would cross the IPC, and the page could still be suspended.
- **The queue staying in the page,** with Rust only playing what it's told (§5.1).
- **Remembering the player in a file beside the settings,** written by Rust, so that changes made while the page is suspended are saved too (§5.5). It's another file the app writes, for a convenience.
- **Switching the device's rate to each file's,** for bit-perfect output. It changes the output for every app, and a switch drops audio, so a join across it couldn't be gapless (§4.4).
- **souvlaki, for Now Playing.** It's built on the older `objc` crate. objc2-media-player is generated from Apple's headers, like the objc2 crates Tauri already uses.

## 14. Settled questions

The first draft left these open. They were settled on 2026-11-16:
1. **Formats symphonia doesn't decode** (§4.2) are skipped, with a toast. The library has none of them.
2. **Fades** where the app cuts the audio: yes, and their length is a setting, in the Controls menu, 10 ms until it's changed (§4.5, §6.4).
3. **Where the player is remembered:** in the page's `localStorage`, as now, handed to Rust as the library opens (§5.5). A file beside the settings isn't wanted. Changes made while the page is suspended can be lost if the app quits before it runs again.
4. **Carrying the remembered queue over** to a new place isn't needed: it stays where it is.

## Appendix A: The engine as built, and what phase 0's checks found

- **Date:** 2026-11-16
- **Affects:** §4.1 Its crate, §4.4 The output, §10 Testing, §11 Phases, §12 Risks and checks
- **Was:**
  - A player's thread takes the commands, keeps the queue, and turns what the callback has played into events (§4.1).
  - `generate.ts` makes a few seconds of a tone and of noise, in each format, cut into consecutive tracks (§10).
  - One example plays files in order, for the checks (§11).
- **Now:**
  - **The engine's commands go to the decoder thread:** `play` for a new queue, `set_upcoming` for what follows the entry playing, play and pause, the volume and the fade. That thread also turns what the callback reports into events: an entry started, an entry failed, or everything ended. Phase 1's queue sits above it, and gives it what follows the entry playing.
  - **The fixtures are tracks of about a tenth of a second,** cut from one triangle wave in integer arithmetic, which the tests work out too:
    - FLAC and Apple Lossless, made by afconvert, whose last FLAC frame is short;
    - AAC, for its delay;
    - an MP3 written by hand, with a LAME tag that says its delay and padding, since this Mac has no MP3 encoder.

    The tests write WAV and AIFF themselves.
  - **Two examples.** `play` has `--start`, which begins the first file that many seconds in by decoding its start and leaving it out, so a join can be reached without playing a whole track. It prints its events with the wall clock's time, to line a recording up with. `decode` counts files' frames, for the library's check.
  - **What the checks found:**
    - **The join.** The two songs in `dev-library/two-songs/` were played through `play`, and recorded with a process tap on its own process. The first song's last sample was followed by the second's first, with nothing between them. Around the join, the recording differed from the two songs by 1 to 2%, as it did everywhere: that's the recording's own resampling. The webview's recording of the same join was 17% off there, which was the click.
    - **The library.** Every file in `dev-library/music/` decodes. All 431 FLAC files decode to the samples their STREAMINFO states, at 0.5 ms per second of audio. The AAC decodes to the frames Core Audio counts after its delay. The MP3 has no LAME tag, and plays as decoded.
    - **CPU:** 0.7 to 0.8% of a core while playing, and 7 MB.
    - **Latency.** This Mac's wired headphones report 233 ms of device latency, so what's rendered is heard 245 ms later. The engine reports it, from cpal's timestamps. Phase 1's time shown, and Now Playing's, take it off (§4.4).
    - **Checked by the user, the same day:** through `play`, the two songs joined with no click heard, and playback went on as Bluetooth headphones were connected and disconnected. Switching the system's output from a script would have moved any other audio playing too, so it wasn't done that way.
- **Why:** building phase 0 settled these. The queue isn't in phase 0, so the engine took its own commands. A tenth of a second covers a short last frame and a join, and keeps the fixtures small. This Mac encodes no MP3.

## Appendix B: The player as built, and what phase 1's checks found

- **Date:** 2026-11-16
- **Affects:** §4.1 Its crate, §4.3 Joining tracks, §4.4 The output, §5.3 Commands and events, §5.5 What it remembers, §6.1 Now Playing, §6.4 The Controls menu, §8 The frontend, §10 Testing, §12 Risks and checks
- **Was:**
  - The callback passes a track's marker as it plays its first sample, and the player then says the track plays (§4.1).
  - When what's up next changes, the decoder decodes the new one (§4.3); a track that can't play is skipped (§5.4).
  - The engine plays to the default device (§4.4), with no word on when the output opens.
  - `PlayerTime` carries the time played and the track's length; `player_set_volume` the volume in hundredths, as the page had it, from 0 to 1 (§5.3).
  - The page writes the queue at each `PlayerState`, and hands it to the player with `player_restore` (§5.5).
  - Now Playing is set at each change of track, when its data changes, at play and pause (§6.1).
  - The Controls menu's Fade is like Repeat (§6.4), which is off with nothing queued.
  - `waiting` is true while the decoder hasn't the data to play (§8).
- **Now:**
  - **What's heard.** The renderer keeps where the output and the stream were at each of its last 512 renders, and works out what's heard, an output's latency behind what's rendered. The engine says a track started once its first sample is heard, and its position, and so the time shown, is what's heard. While paused, what's heard catches up with where it paused.
  - **Epochs.** Each `play()` and `stop()` starts an epoch, and events are stamped with theirs, so the player drops one about what no longer plays, like a track heard just before Next.
  - **What follows is anchored.** `set_upcoming(after, tracks)` names the entry it follows, and the engine ignores it once another entry plays. So a change that crosses a join is sent again, after the player hears the join, rather than taken as following the wrong entry.
  - **A failure stops the engine.** The engine says `Failed` and drops what was to follow, and the player says what follows, by the queue's rules. A track that fails as it's decoded ahead is set aside: the engine is given what follows it, so the next one joins where it would have started, and the failure is said, with its toast, when the queue reaches it. It's never shown as current.
  - **The player gives the engine up to 3 entries** to follow the current one, so short tracks join too, and nothing is loaded for a paused queue until it plays.
  - **The output** opens as something first plays, and pauses once nothing has played for 2 seconds, so the device rests. A lost device pauses the player, which makes the engine and the output again at the next play.
  - **`PlayerTime` carries only the milliseconds heard.** The length shown is the catalog's, which the page and Now Playing already have.
  - **Whole numbers in the events.** The volume is in hundredths, from 0 to 100, in `PlayerState` and in `player_set_volume` and `player_restore`, and the time is in milliseconds: specta types a float that's sent as `number | null`.
  - **Rust writes what's remembered.** `PlayerState.remembered` is the queue as JSON, in the format the page wrote before, so a queue remembered before comes back. The page stores it as it is, and hands it back to `player_restore`, which only the first call for a library takes: a page that mounts again keeps the player as it is.
  - **Now Playing** is left alone until the player has played since the library opened, so opening a library doesn't take the media keys from the app that played last. Its time is set again when it moves more than a second from where macOS would have it. A save is caught by SQLite's count of changes: any library command that wrote has Now Playing read the current track again.
  - **Fade is on whenever a library is open,** even with nothing queued, since it's a setting, not the queue's.
  - **`waiting`** is true once the engine has had nothing to play for a quarter of a second while it plays.
  - **Tests** (§10): the queue's cases are ported in `tests/queue.rs`, with `following()`'s. `tests/player.rs` plays the queue through the engine with a device rendering about six times as fast as it would play, and checks the output is the fixtures' signal, every sample, past a track that can't play. The app's commands are thin, and aren't tested on their own.
  - **What the checks found,** in the app, driven through WebDriver, muted:
    - **Formats.** The formats library's eight one-second tracks played in turn, FLAC, WAV, AIFF, Apple Lossless, AAC, AAC in ADTS, and CBR and VBR MP3, with no failure, and the player stopped, paused, after the last.
    - **Missing files.** A release whose eight files were missing failed track by track, a toast each with its path and why, and the player stopped once all had failed.
    - **Remembered.** A queue the webview's player had remembered came back, paused. Closing the window while it played quit the app in under a second, and the queue came back where it was, paused.
    - **A library switched while it plays.** The old player stopped, its threads gone, and the new library's queue came back paused, with the settings.
    - **Resting.** Two seconds after a pause, Core Audio's IO thread is gone.
    - **Not checked yet:** Now Playing and the media keys, in a debug and a release build, after another app and the file page's preview have played, and with the window hidden for minutes; WebKit leaving Now Playing alone; the two songs by ear in the app; an Ogg Vorbis track, since this Mac has neither an Ogg file nor an encoder.
- **Why:** building phase 1 settled these. The design's player took the engine's events as they came, and a race between a command and a join could have the queue and the engine disagree; epochs and anchored changes make them agree. The time a track is heard is what the player shows. The rest keeps the device and the media keys free while nothing plays, and the events' types plain.
