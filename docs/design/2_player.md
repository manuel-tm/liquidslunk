# Music player

> **Status: complete.**

liquidslunk catalogs music, and plays a track only where the track is listed. The release page has play buttons, and the track and file pages each have an audio element. Each one plays on its own, and stops when the page changes. This design adds a music player: one player for the whole app, in the sidebar between the library menu and Import. It has play and pause, previous and next, seeking and volume, and a queue that keeps playing from page to page.

This document builds on the base design, [1_introduction.md](1_introduction.md). Here, "§4" and "phase 0" mean this document's own sections and phases, and "design §5.5" means the base design's, as it does everywhere else. Code and docs cite this document as "2_player.md §4" and "phase 0 of 2_player.md".

It covers:
- where the audio plays, and why that's the webview;
- the player in the sidebar, its controls and its shortcuts;
- the queue: what starts it, what changes it, and what it remembers;
- the commands it needs;
- what happens when a track can't play;
- macOS's Now Playing and media keys, and a Controls menu in the menu bar;
- the phases to build it in.

## 1. Summary

- **One player, in the sidebar's header,** between the library menu and Import. It shows the cover, title and artists of what's playing, with a seek bar, previous, play/pause and next, and the volume. Phase 1 adds shuffle, repeat and the queue.
- **It plays in the webview,** with one `<audio>` element fed by the file protocol, which already answers ranges (design §5.5). There's no native audio and no new Rust dependency.
- **It lives above the pages,** in a provider in `AppLayout`. It keeps playing from page to page, and stops when the library closes or switches.
- **A queue of tracks.** Playing a track on the release page queues the release from that track. The release table plays a whole release from its row. "Play next" and "Add to queue" add to the queue.
- **macOS shows what's playing.** Control Center's Now Playing shows the track, its artists and its cover, and its buttons, the media keys and headphones' controls play, pause and skip. It all goes through the webview's Media Session. A Controls menu in the menu bar does what the player's buttons do.
- **A new command for the player,** `player_tracks`, for what it shows of each queued track. Two more come later: one for the release table's Play, and one for the Controls menu.
- **Nothing changes in the database.** The player writes nothing but `localStorage`, which remembers the volume, and the queue of each library.
- **The queue's rules are tested,** with Vitest, the frontend's first test runner.
- **Three phases:** **0** The player → **1** The queue → **2** The Controls menu.

## 2. Scope

In scope:
- playing a library's tracks from a queue, one after another;
- play and pause, previous and next, seeking, volume and mute, shuffle and repeat;
- the queue: starting it from the release page, the release table, the track page and the track list, adding to it, reordering it, and remembering it;
- keyboard shortcuts in the app;
- Control Center's Now Playing, and the keyboard's media keys (phase 0);
- a Controls menu in the menu bar (phase 2);
- Vitest, for the queue's rules.

Out of scope:
- **The player anywhere but the sidebar.** When the sidebar is hidden, so is the player (§4.2).
- **Playing from anywhere else,** like a tag's values.
- **Gapless playback.** One `<audio>` element leaves a short gap between tracks (§3). If the gap is a problem, closing it takes native playback, in a later design.
- **Formats WebKit can't play.** A track that lofty reads but WebKit can't decode is skipped, with a toast (§7).
- **Play counts, ratings, history and playlists.** They're the library's data, so they'd go in the database, through a migration, which is a one-way door. They need their own design.
- **Playing files that aren't tracks.** The file page keeps its own preview (§7.2).
- **Output devices, equalizers, crossfades and lyrics.**
- **Writing to the user's files.** Playing only reads them, through the file protocol, as every page already does.

## 3. Where the audio plays

> Changed during implementation, see [Appendix A](#appendix-a-webkit-seeks-flac-by-estimate).

| | A. The webview's `<audio>` | B. Native, in Rust |
|---|---|---|
| How | One `<audio>` element, its `src` set to `fileUrl(track.file.id)` | symphonia decodes, and cpal plays, on a thread of their own |
| Formats | What WebKit plays: on macOS, at least MP3, AAC, ALAC, FLAC, WAV and AIFF, the formats of the audio fixtures | What symphonia decodes, which adds Ogg Vorbis, but still not Opus, APE, WavPack, DSF or WMA |
| Gaps between tracks | A short one | None, if the next track is decoded ahead |
| Seeking | Already works: the protocol answers ranges (design §5.5) | Its own, per format |
| Where its state is | In the webview, with the controls | In Rust. The webview follows it through events, and sends commands back |
| Now Playing, media keys | Through Media Session. WKWebView already publishes what plays (§8.1) | `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter`, through objc2 |
| New dependencies | shadcn's `slider` | symphonia, cpal, a resampler, objc2's bindings |

**Decision: A.** It's a provider and its controls, built on what the release page already does. B is the way forward if gaps or formats come to matter. The controls only call `usePlayer()` (§4.1), so the engine can change without them changing.

## 4. The player

### 4.1 Where it lives

- **`PlayerProvider`** (`src/components/player/`) holds the queue and renders the app's one `<audio>` element, which has no controls of its own. `AppLayout` renders it inside the `SidebarProvider`, around the sidebar and the page. The `SidebarProvider` is keyed by the library's path, so:
  - changing pages doesn't touch the player, and the music keeps playing;
  - switching libraries mounts the player again, so the audio stops and the queue is the new library's (§5.7);
  - closing the library goes to Welcome, outside `AppLayout`, which unmounts the player, and the audio stops.
- **The element isn't in the sidebar.** Below 768 px wide, the sidebar is a `Sheet`, mounted only while it's open, and the window can be 720 px wide. A collapsed sidebar is off screen. Neither may stop the music. So the sidebar only shows the controls (`SidebarPlayer`), and the provider plays.
- **One element plays every track.** Moving to another track sets the element's `src`, then calls `play()`. WebKit lets an element play without a click once the user has started it, so the next track can start at `ended`, even in the background. §10 checks that.
- **`usePlayer()`** is how the rest of the app uses the player. It gives what's queued and what's playing, the volume, shuffle and repeat, and the actions: `play(ids, start)`, `playNext(ids)`, `addToQueue(ids)`, `toggle()`, `previous()`, `next()`, `seek(seconds)`, `jump(entry)`, `remove(entry)`, `move(entry, by)` and `clear()`. Pages never touch the element.
- **The time** comes from `usePlayerTime()`, which subscribes to the element's `timeupdate` and `durationchange` events with `useSyncExternalStore`. Only the seek bar and its times use it, so the rest of the app doesn't render again four times a second.
- **The queue's rules** are a reducer in `src/lib/queue.ts`: plain functions from a queue and an action to the next queue, with no React and no element. The provider keeps the queue with `useReducer`. There's no state library, as design §7.2 says. The reducer is tested (§5.8).

### 4.2 In the sidebar

> Changed during implementation, see [Appendix B](#appendix-b-no-seeking-for-now).

The sidebar's header holds, from the top: the library menu, the player, then Import.

```
┌──────────────────────────────┐
│ ▤ Music                    ⇕ │  the library menu
│   liquidslunk                │
│ ┌────┐ Bridges               │  the track, linking to its page
│ │    │ Alika                 │  who performs it, each linking to the artist
│ └────┘ Virmalised            │  the release, muted, linking to its page
│ ━━━━━━━━━●────────────────── │  the seek bar
│ 1:23                    4:05 │  time played, and the track's length
│ [shf] [<<] [play] [>>] [rpt] │  shuffle, previous, play/pause, next, repeat
│ [vol] ━━━━━━●─────── [queue] │  mute, the volume, the queue
│ ⤓ Import                     │
└──────────────────────────────┘
```

- **What's playing:** the release's cover (`Cover`, 48 px: `size-12`), which links to the release, then three lines. Each is cut to one line, with its whole text in a tooltip:
  - the track's title, linking to the track's page, in its language (`Title`);
  - who performs it, each name linking to its artist (`Names`), counted as the track page counts them (`performers()`);
  - the release's title, muted, linking to the release.
- **The seek bar** is shadcn's `slider`, with the time played on its left and the track's length on its right. The length is the element's once it has read the file, and the catalog's `durationMs` until then.
- **The buttons:** previous, play/pause and next, in the middle. Shuffle on the left and repeat on the right come in phase 1.
- **The volume:** a speaker button that mutes and unmutes, then a slider. The queue's button, at the end, comes in phase 1.
- **With nothing queued,** the player is a single muted line, "Nothing playing", beside the cover's placeholder. It takes little room until it's used.
- **While the element waits for data,** the play button shows a spinner.
- **For assistive tech,** every button has an `aria-label`, and a tooltip naming its shortcut (§4.4). The seek bar reads as "1:23 of 4:05".
- **When the sidebar is hidden,** collapsed or in its sheet below 768 px, so is the player. Nothing else shows it, not even in the page header. The music keeps playing, and the shortcuts, the Controls menu and Now Playing still work.

The exact spacing is settled on a screenshot, in phase 0.

### 4.3 Controls

> Changed during implementation, see [Appendix B](#appendix-b-no-seeking-for-now).

- **Play/pause** plays or pauses the current track.
- **Previous** goes back to the start of the track once its first 3 seconds have played, and to the previous track before that. On the first track, it goes back to its start, or to the last track with repeat all.
- **Next** goes to the next track, and plays it if the player was playing. On the last track, it goes to the first with repeat all. Otherwise the player stops, with the last track paused at its start, so Play plays it again.
- **When a track ends,** the player goes on as Next does. With repeat one, the track plays again.
- **Seeking:** dragging the seek bar shows where it'll land, and seeks when it's let go. Clicking the bar seeks to that point.
- **The volume** goes from 0 to 100%. The browser remembers it and whether it's muted (`slunk.player.volume` in `localStorage`), for every library, as it does the theme.

### 4.4 Shortcuts

As in Apple's Music app:

| Keys | What they do |
|---|---|
| Space | Play or pause |
| ⌘→ | Next |
| ⌘← | Previous |
| ⌘↑, ⌘↓ | The volume up or down, by 10% |

- They're ignored while typing in a field, where they move the caret. Space is also ignored while a button or another control has focus, since Space presses it.
- `usePlayerShortcuts()` handles them in the provider, as `useHistoryShortcuts()` handles ⌘[ and ⌘]. None is taken already: `d` toggles the theme, and ⌘[ and ⌘] go back and forward.
- They work while the sidebar is hidden.
- From phase 2, the Controls menu lists them, and may handle some of them itself (§8.2).

## 5. The queue

### 5.1 What it holds

- **Entries,** in the order they play. An entry is a track id with a key of its own, so a track queued twice is two entries.
- **The current entry,** playing or paused. The entries before it have played, and Previous goes back through them.
- **The order before shuffling,** while shuffle is on (§5.4).

The queue holds ids only. What it shows of each track comes from `player_tracks` (§6), so it's always what the catalog says now.

### 5.2 Starting it

These replace the queue, and play:
- **The release page.** Play, by the tracklist's heading, plays the release from its first track, in the tracklist's order: by disc, then by number. Each track's play button, at the end of its row as it is now, plays the release from that track. The current track's row is marked: its number becomes a speaker icon, and its button pauses. The page's own `<audio>` element goes.
- **The track page.** Play takes the place of the `<audio controls>` element, and plays the track alone.
- **The release table** (phase 1). Each row's cover gets a play button over it, shown on hover and on keyboard focus. It plays the release from its first track, in the tracklist's order, with the ids from `release_track_ids` (§6.2). The table is on the release list, a label's page and an artist's Releases tab, so all three get it. A release with no tracks says so in a toast.
- **The track list** (phase 1). Each row gets a play button, which plays the page's rows, in the list's order, from that row. So a filtered list ("Genre is Rock") plays like a playlist, a page at a time.

### 5.3 Adding to it

Phase 1 adds two ways to add tracks without replacing the queue:
- **Play next** puts tracks right after the current one.
- **Add to queue** puts them at the end.

They're in a menu by Play on the release page, for the whole release, and in a menu on each of the tracklist's rows, on the track page, and on each row of the track list, for one track. With nothing queued, either one starts a queue, paused at its first track. A toast says what was added: "Added 12 tracks to the queue".

### 5.4 Shuffle and repeat

Both come in phase 1.
- **Shuffle** plays the entries after the current one in a random order. Turning it off plays them in their order again, from the current one. While it's on, Play next still puts tracks right after the current one, and Add to queue still puts them at the end.
- **A queue started with shuffle on** starts at the track that was clicked, or at a random one from the release's Play, and shuffles the rest.
- **Repeat** goes from off, to all, to one, and back to off. With repeat all, the first entry follows the last. With repeat one, a track that ends plays again, while Next and Previous still move.
- The browser remembers both, for every library (`slunk.player.shuffle` and `slunk.player.repeat` in `localStorage`).

### 5.5 The queue sheet

> Changed during implementation, see [Appendix D](#appendix-d-the-queue-sheets-rows-are-dragged).

In phase 1, the queue's button opens a `Sheet` on the right, with the whole queue in order:
- Each entry is a row with its cover, title, artists and length, each linking to its page. The ones that have played are muted, and the current one is marked. The sheet opens scrolled to it.
- Clicking a row plays it.
- Each row can move up or down, or be removed, as in `OrderedList`. Removing the current entry goes on to the next.
- **Clear** empties the queue, and the music stops.

### 5.6 When the library changes

- **Saves.** The player reads `player_tracks` with `useCommand()`. It's mounted, so the `MutationCache` invalidates it after every save, and it's fetched again (design §7.2): a renamed track shows its new title in the player. The query keeps its last data while the queue's ids change (`placeholderData`), so the player doesn't flicker when the queue does.
- **Nothing interrupts what's playing.** The element's `src` changes only when the current entry does, never on a fetch. When a track's audio file is changed, the new file plays the next time the track starts.
- **Deleted tracks** aren't in `player_tracks`' answer, so their entries leave the queue. When the current entry's track is deleted, the player goes on to the next entry, and plays it if it was playing.
- **Switching libraries** stops the music, and the queue becomes the new library's (§4.1 and §5.7).

### 5.7 What it remembers

> Changed during implementation, see [Appendix B](#appendix-b-no-seeking-for-now).

Phase 1 remembers the queue:
- **The queue, per library:** its entries, in the order they play and, while shuffled, in their order before; the current entry; and the time played in it. It's kept in `localStorage`, under `slunk.player.queue:<the library's path>`, since ids belong to a library. `slunk.imageFolder:<the library's path>` works the same way (design §6.4).
- **When it's written:** when the queue changes, on pause, and every few seconds while playing. **When it's read:** when a library opens, at launch or on a switch. It's restored paused: the app never starts playing on its own.
- **It's a convenience.** When `localStorage` fails, it's ignored, as `useRemembered()` on the release page already does.
- **It's not kept in the database.** The queue is the app's state, not the library's metadata, and a table for it would take a migration, which is a one-way door.
- **It's not kept in `settings.json`.** That file belongs to Rust, and holds what the launch needs (design §6.2).

### 5.8 Testing it

The queue's rules are the part most likely to be off by one, and they're plain functions. So the frontend gets its first tests:
- **Vitest** runs them with `npm test`, which is `vitest run`. It reads `vite.config.ts`, so `@/` resolves as it does in the app.
- **Tests sit next to what they test,** as `*.test.ts`. They import `describe`, `it` and `expect` from `vitest`, with no globals, so `tsc -b` checks them like the rest of `src/`.
- **They test plain functions,** in Node, with no DOM. Components aren't tested: screenshots check them.
- **`lib/queue.test.ts`** covers the reducer's rules:
  - Previous before and after 3 seconds;
  - the end of the queue, with repeat off, all and one;
  - removing the current entry, and a track queued twice;
  - shuffling and unshuffling, and Play next and Add to queue while shuffled;
  - deleted tracks leaving the queue;
  - stopping once every entry has failed.
- **AGENTS.md's "Before you finish"** gets `npm test`, beside `npm run typecheck` and `npm run lint`.

## 6. The backend

### 6.1 What the player shows

Phase 0 adds one command, in `src-tauri/src/commands/catalog/tracks.rs`:

```rust
/// What the player shows of a track.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTrack {
    pub id: i64,
    pub title: String,
    /// Null: the release's
    pub title_language: Option<String>,
    /// Read from the audio file. Null when it can't be read.
    pub duration_ms: Option<i64>,
    /// Who performs it, as the track page counts them
    pub performers: Vec<CreditedArtist>,
    pub release: ReleaseHeader,
    /// The release's cover
    pub cover: Option<FileRef>,
    /// The audio file
    pub file: TrackFile,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTracksArgs {
    pub ids: Vec<i64>,
}
```

- **What it returns.** `player_tracks({ ids })` returns a `PlayerTrack` for each id that's a track, each once, in the order asked. An id that isn't a track is left out, and isn't an error: a remembered queue can name a track deleted since.
- **How it loads them.** One query over the tracks' ids, with the chunked `in (…)` helper. Then their releases, and their artists with `Related`, as the track list loads them (design §5.4). It writes nothing.
- **Its shape** is the other commands': a thin wrapper over `catalog::tracks::player_tracks(library, args)`, with a Rust test. The test covers the order, unknown ids, an id asked for twice, and who performs on a regular and on a various-artists release.

### 6.2 A release's tracks

Phase 1 adds `release_track_ids({ id })`, in the same module, for the release table's Play (§5.2). It returns the ids of the release's tracks by disc, then by number, as the tracklist orders them. A release with no tracks, or one that isn't there, gives an empty list. Its test covers the order across discs.

### 6.3 The Controls menu

Phase 2 builds the menu in Rust, with a command that sets what it shows, and an event for each item that's clicked (§8.2).

Nothing else changes in the backend. The protocol already serves a file's bytes and ranges (design §5.5), and the CSP allows `slunk:` in `media-src`.

## 7. When a track can't play

### 7.1 Skipping it

- **When:** the element fires `error` when a file is missing, which the protocol answers with a 404, or when WebKit can't decode its format. Ogg, APE, WavPack, Musepack, DSF and WMA files can all be tracks (design Appendix D).
- **What happens:** the player shows a toast, "Can't play Bridges", with the file's path and "The file is missing, or its format can't be played here." Then it goes on to the next track, and plays it.
- **When nothing plays:** when every entry has failed since the last one that played, the player stops, rather than going round the queue with repeat all.
- The toast doesn't tell a missing file from a format apart. The library check finds missing files.

### 7.2 One sound at a time

- The file page keeps its own preview, an `<audio controls>` element, since a file doesn't have to be a track. The track page's and the release page's elements go (§5.2).
- The provider listens for `play` on the whole document, in the capture phase, since media events don't bubble. When any media element starts, it pauses the others: the player pauses when a preview starts, and a preview pauses when the player starts.

## 8. Now Playing, media keys and the Controls menu

### 8.1 Now Playing and media keys

> Changed during implementation, see [Appendix B](#appendix-b-no-seeking-for-now) and [Appendix C](#appendix-c-now-playings-actions-are-set-when-playback-starts).

- **What macOS does:** it shows what's playing in Control Center's Now Playing, and in the menu bar's Now Playing item. It sends the keyboard's media keys (F7 to F9), and headphones' controls, to the app that played last.
- **What it shows now.** WKWebView already publishes what its media elements play. On 2026-10-19, a track playing on the release page showed in Now Playing with the app's name as its title, a dash for its artist, and the app's icon for its cover, with 15-second skip buttons where previous and next should be. Only its time and progress were right. The page sets no metadata and no handlers, so WebKit shows its defaults.
- **Phase 0 sets them,** with `navigator.mediaSession`, from the provider:
  - **The metadata:** the track's title; who performs it, as the player names them, joined with commas; the release's title; and the cover, as `fileUrl(cover.id)`. It's set whenever the current entry changes, or its data does, and cleared when nothing is queued.
  - **The state:** `playbackState` is `playing` or `paused`, or `none` with nothing queued.
  - **The handlers:** `play`, `pause`, `previoustrack`, `nexttrack` and `seekto` call `usePlayer()`. `seekbackward` and `seekforward` are set to `null`, so Now Playing shows previous and next instead of the 15-second skips.
  - The media keys and headphones' controls reach the same handlers, since macOS sends them to the app that plays.
- **Phase 0 checks** (§10) that Now Playing then shows the right data, that it changes with each track, and that it loads the cover from `slunk://`. If it can't, the provider gives it a `blob:` URL of the cover's bytes instead.
- **If Media Session falls short,** the fallback is native. Rust sets `MPNowPlayingInfoCenter` and handles `MPRemoteCommandCenter` through objc2's bindings, and talks to the provider through a command and an event, as the Controls menu does (§8.2). An appendix would plan it, for phase 2.

### 8.2 The Controls menu

> Changed during implementation, see [Appendix E](#appendix-e-the-webview-keeps-its-shortcuts).

The menu bar gets a Controls menu, between View and Window, as in Music. It has:
- **Play** or **Pause,** as the player is;
- **Next** and **Previous,** as the player's buttons (§4.3);
- **Shuffle,** checked while it's on;
- **Repeat,** a submenu with Off, All and One, the current one checked;
- **Volume Up** and **Volume Down,** by 10%.

How it works:
- **The menu is built in Rust,** in `src-tauri/src/menu.rs`. Setting a menu replaces Tauri's default one, so it starts from the default and adds Controls to it.
- **Clicks** reach the webview as a typed event, `PlayerControl`: `toggle`, `next`, `previous`, `shuffle`, `repeat` with its mode, `volumeUp` and `volumeDown`. It's tauri-specta's, like the commands, so it's in `src/bindings.ts`. The provider listens for it, and calls `usePlayer()`.
- **What the menu shows** comes from the webview. Whenever the player plays or pauses, the queue empties or fills, or shuffle or repeat changes, the provider calls `set_controls_menu({ playing, queued, shuffle, repeat })`. Rust then sets Play or Pause, Shuffle's check and Repeat's, and turns the items off while nothing is queued. The provider unmounts at Welcome, and as it goes, it calls the command with nothing queued.
- **Shortcuts.** If the menu had ⌘→, ⌘←, ⌘↑ and ⌘↓ as key equivalents, it might take them before the webview does, and ⌘← in a text field would skip a track instead of moving the caret. Phase 2 checks that (§10):
  - if a focused field still gets them first, the menu's items have them, and the webview's handlers for them go;
  - otherwise, the items show no key equivalents, and the webview keeps them (§4.4).

  Play or Pause never has one: Space as a key equivalent would take it from every field. Space stays in the webview.

## 9. Phases

Each phase ends with something usable and documented, and every check in AGENTS.md's "Before you finish" passes.

**Phase 0: The player**
- The backend: `player_tracks`, and its test (§6.1).
- Vitest, with `npm test` (§5.8).
- The frontend:
  - `lib/queue.ts`, the reducer, with its tests: play, next, previous, jump, remove and clear;
  - `PlayerProvider` in `AppLayout`, with the element, `usePlayer()`, `usePlayerTime()` and the shortcuts;
  - shadcn's `slider`;
  - `SidebarPlayer`, between the library menu and Import: what's playing, the seek bar and its times, previous, play/pause and next, and the volume;
  - the release page: Play, and the tracks' play buttons through the player, with the current track marked;
  - the track page: Play, instead of its `<audio controls>`;
  - skipping what can't play, and one sound at a time (§7);
  - the volume, remembered;
  - Now Playing, through Media Session (§8.1).
- Phase 0's checks (§10), each recorded as an appendix where it changes this design.
- Docs:
  - `frontend.md` gets a Player section and a Tests section, the player's files in Structure, and the sidebar's header, the release page and the track page as they then are;
  - `backend.md` gets `player_tracks`;
  - `getting-started.md` gets `npm test` in its table of scripts;
  - AGENTS.md's "Before you finish" gets `npm test`.
- *Exit:*
  - A release plays from any track to its end, each track going on to the next, with the window in the background and with the sidebar hidden.
  - Pause, previous, next, seeking and the volume work from the sidebar and from the keyboard.
  - The music keeps playing from page to page, and stops when the library switches or closes.
  - A missing file is skipped, with a toast.
  - Now Playing shows the track's title, its artists, its release and its cover, with previous and next. Its buttons and the media keys work while another app is in front.
  - `npm test` passes.

**Phase 1: The queue**
- `lib/queue.ts`, with its tests: play next, add to queue, move, shuffle and repeat.
- The sidebar: shuffle, repeat, the queue's button, and the queue sheet.
- Play next and Add to queue on the release page, on its tracklist's rows, on the track page and on the track list's rows. The track list's play buttons.
- The release table's Play, with `release_track_ids` and its test (§6.2).
- Remembering the queue per library, and shuffle and repeat.
- Docs: the queue in `frontend.md`, and `release_track_ids` in `backend.md`.
- *Exit:*
  - A release plays from its row on the release list, on a label's page and on an artist's Releases tab.
  - A queue built from two releases and a filtered track list plays in order, shuffled, and repeated.
  - The queue sheet reorders the queue and clears it.
  - After a restart, the queue is there, paused where it was.

**Phase 2: The Controls menu**
- The menu, `set_controls_menu` and the `PlayerControl` event (§8.2).
- Its shortcuts, as phase 2's check decides (§10).
- A native Now Playing, only if phase 0 found that Media Session falls short (§8.1).
- Docs: the menu, its command and its event in `backend.md`, and the menu in `frontend.md`.
- Mark this design complete.
- *Exit:* the Controls menu plays, pauses, skips, shuffles, repeats and sets the volume, with the sidebar hidden too. Its items follow the player: Pause while it plays, Shuffle and Repeat checked as they are, and all of them off while nothing is queued.

## 10. Risks and checks

> Changed during implementation, see [Appendix A](#appendix-a-webkit-seeks-flac-by-estimate) and [Appendix E](#appendix-e-the-webview-keeps-its-shortcuts).

Each of these is checked early in the phase that needs it. Anything that changes this design is recorded as an appendix.
- **Playing in the background** (phase 0). macOS can throttle a hidden app (App Nap), and WebKit can refuse `play()` without a click. Check that the next track starts at `ended` with the window hidden, minimized, and behind another app. If WebKit refuses, the webview's autoplay setting is the first thing to try.
- **Formats** (phase 0). Register the audio fixtures in a scratch library in `dev-library/`, and play each one's format: FLAC, M4A (AAC and ALAC), AAC in ADTS, WAV, AIFF and MP3.
- **Seeking with the slider** (phase 0), in FLAC, MP3 and M4A, and near the end of a long file, where the protocol answers each open range with 1 MB (design §5.5).
- **The gap between tracks** (phase 0). Listen to how long it is. If it's a problem, a second element that loads the next track ahead is the first fix to try, before anything native.
- **Now Playing** (phase 0, §8.1). Check that the metadata and handlers replace the app's name, the dash and the 15-second skips, and that Now Playing changes with each track. Check that it loads the cover from `slunk://`, and that the media keys reach the handlers while another app is in front.
- **Renders** (phase 0). The time changes about four times a second. With React's profiler, check that only the seek bar and its times render again for it.
- **The menu's key equivalents** (phase 2, §8.2). With ⌘← on a menu item, check whether a focused text field still moves its caret.

## 11. Alternatives considered

- **Native playback in Rust (§3, B).** It would close the gaps between tracks and play more formats. It would cost a decoder, an audio output, and keeping two states in step. It's the way forward if the gaps or formats come to matter, behind the same `usePlayer()`.
- **A bar along the bottom of the window,** as most players have. The sidebar is where the player was asked for. It also takes no height from the pages, which are mostly tall lists, and it keeps the app's controls together.
- **The player as a page of its own.** It couldn't be seen while browsing, which is when the controls are needed.
- **A small player in the page header while the sidebar is hidden.** It isn't wanted: the player goes where the sidebar goes (§12).
- **Web Audio, for gapless playback.** It decodes a whole file into memory before playing it: over 200 MB for ten minutes of stereo at 44.1 kHz.
- **An element per page,** as the release page has now. The music stops when the page changes.
- **The queue in the database, or in `settings.json`** (§5.7).

## 12. Settled questions

The first draft left these open. They were settled on 2026-10-19:
1. **A test runner for the frontend:** Vitest, from phase 0 (§5.8).
2. **The player while the sidebar is hidden:** it stays hidden, and nothing shows it in the page header (§4.2).
3. **A Controls menu in the menu bar:** yes, in phase 2 (§8.2). And Now Playing, which showed the app's name instead of the track, shows the track from phase 0 (§8.1).
4. **Playing from more places:** from the release table, in phase 1 (§5.2). Nowhere else for now.
5. **Play counts and history:** not now (§2).

## Appendix A: WebKit seeks FLAC by estimate

- **Date:** 2026-10-19
- **Affects:** §3 Where the audio plays, §10 Risks and checks
- **Was:** seeking already works, since the protocol answers ranges (design §5.5). Phase 0 checks it in FLAC, MP3 and M4A.
- **Now:**
  - WebKit seeks a FLAC file by estimate, not with its seek table. After a seek, the time it shows can be several seconds off the audio that plays. On a live track 4:28 long, a seek to 4:18 played 20 seconds before the track ended, from about 4:08. A seek 2 seconds before the end of a 2:56 track played 7 seconds.
  - When the time reaches the track's length before the audio does, WebKit holds it there, with `ended` already set and no event, while the rest plays. Then it stretches the length to fit, 4:38 for that live track, and fires `pause` and `ended`. The player goes on only at `ended`, so no audio is cut, but the seek bar waits at its end meanwhile.
  - Phase 0 first took that held state for an end WebKit had missed, and went on to the next track, which cut the rest of the track. That's taken out.
  - Seeking in MP3 and M4A wasn't measured.
- **Why:** WKWebView plays audio through AVFoundation, which seeks FLAC this way. The protocol isn't the cause: WebKit got every byte it asked for, which logging its ranges showed. Exact seeks in FLAC would need the app to serve FLAC in a form WebKit seeks exactly, which would change §3's decision. That's undecided.

## Appendix B: No seeking for now

- **Date:** 2026-10-19
- **Affects:** §4.2 In the sidebar, §4.3 Controls, §5.7 What it remembers, §8.1 Now Playing and media keys
- **Was:** the seek bar seeks when it's let go, and Now Playing seeks through its `seekto` handler. A remembered queue comes back at the time played in its current track.
- **Now:**
  - Nothing in the player seeks. The bar shows how far the track has played, between the time played and the track's length, and can't be dragged.
  - Now Playing has no seek action: the player sets no `seekto`, `seekforward` or `seekbackward` handler, so it shows no scrubber or skips (Appendix C).
  - Previous still goes back to a track's start, since a file's start is exact.
  - Phase 1 brings a remembered queue back at the start of its current track.
- **Why:** WebKit seeks FLAC by estimate (Appendix A), so a seek can play from seconds away from the time it shows. The user decided to take seeking out until it can be exact, which needs a plan of its own: WebKit would have to be given FLAC in a form it seeks exactly.

## Appendix C: Now Playing's actions are set when playback starts

- **Date:** 2026-10-19
- **Affects:** §8.1 Now Playing and media keys
- **Was:** the provider sets the Media Session's handlers once. Setting `seekbackward` and `seekforward` to `null` makes Now Playing show previous and next, instead of 15-second skips.
- **Now:**
  - WebKit offers Now Playing exactly the actions that have handlers, plus play and pause. When none are registered with it, it offers its own: a scrubber and 15-second skips, but no previous or next.
  - A handler set before a media element has registered with WebKit's Now Playing is dropped. The provider set its handlers when it mounted, before anything played, so Now Playing showed WebKit's own actions: the skips and a scrubber that seeked.
  - So the provider sets its handlers again whenever its element starts playing: play, pause, previous and next. It sets none for seeking (Appendix B).
- **Why:** Control Center showed the skips and seeked the track, though the metadata set the same way was right. WebKit's source explains it: its Now Playing manager passes a supported command on only while it has a listener, which it creates when a media element registers.

## Appendix D: The queue sheet's rows are dragged

- **Date:** 2026-10-19
- **Affects:** §5.5 The queue sheet
- **Was:** each row can move up or down, or be removed, with buttons, as in `OrderedList`.
- **Now:**
  - A row is dragged from anywhere on it to another place, with dnd-kit. The drag starts once the pointer has moved a few pixels, so clicking a row still plays it, and its links still work. The list scrolls while a row is dragged near its edges. The up and down buttons are gone, and a row's button still removes it.
  - A grip at the start of each row shows that it drags. With the grip focused, the up and down arrow keys move a row a place, the focus stays on it, and a live region says where it went. This doesn't use dnd-kit's keyboard sensor, which listens on the document: Base UI's dialog popup, which the sheet is, stops arrow keys from leaving it.
- **Why:** the user asked for the queue to be dragged and dropped, from anywhere on a row, since a handle alone was too small a target.

## Appendix E: The webview keeps its shortcuts

- **Date:** 2026-10-19
- **Affects:** §8.2 The Controls menu, §10 Risks and checks
- **Was:** if a focused field still gets ⌘←, ⌘→, ⌘↑ and ⌘↓ before the menu does, the menu's items have them as key equivalents, and the webview's handlers for them go.
- **Now:**
  - A focused field does get them first. In the release list's search field, ⌘← and ⌘→ moved the caret to the start and the end, and the menu didn't act. So the items have them: ⌘→ for Next, ⌘← for Previous, ⌘↑ for Volume Up and ⌘↓ for Volume Down.
  - The webview's handlers stay, though. Without the provider's handler, ⌘↓ on a page longer than the window scrolled it to its end, and the menu never got the key. With the handler, the volume went down, and the page stayed where it was.
  - The handler calls `preventDefault()`, which tells WebKit the page used the key, so the menu doesn't act on it a second time. The menu acts on a key only where the page leaves it: with the handler held back, ⌘→ outside a field reached Next.
- **Why:** WebKit gives the page every key first, and passes it on to the menu only when nothing in the page used it. Outside a field, WebKit uses ⌘↑ and ⌘↓ itself, to scroll to the top and the end of the page, so without the handlers, the volume keys would scroll most pages instead. The check sent the keys inside the app, the way AppKit routes a key equivalent: to the window, whose WKWebView hands it to the page, then to the menu if the page leaves it. No tool could press them on the keyboard: that needs Accessibility access.
