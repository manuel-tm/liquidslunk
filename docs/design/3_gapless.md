# Gapless playback

> **Status: complete.** Its one phase (§6, phase 0) was built by 2026-10-19.

The player leaves about a third of a second of silence between two tracks. On most albums that goes unnoticed, since tracks start and end quietly. On a live album or a continuous mix, the music stops at every track and starts again. This design closes most of that gap, within the webview's `<audio>` elements that the player already uses.

It builds on [2_player.md](2_player.md), which is complete. Here, "§3" and "phase 0" mean this document's own sections and phase, and "2_player.md §3" means the player's. Code and docs cite this document as "3_gapless.md §3" and "phase 0 of 3_gapless.md".

## 1. Summary

> Changed during implementation, see [Appendix A](#appendix-a-by-the-settled-clock-a-start-takes-about-240-ms) and [Appendix B](#appendix-b-the-queue-goes-on-when-the-current-track-ends).

- **The gap is mostly WebKit starting an element, not loading a file.** On 2026-10-19, once a fully loaded element was told to play, its clock took 250 to 340 ms to start moving, depending on the format. Before that, `ended` fired up to 110 ms after the clock stopped. Loading the next track ahead doesn't shorten either (§2).
- **So the next track starts a little before the current one ends.** A second element loads the next track ahead, and plays it about 120 ms before the current track's end, by the current element's clock. The next track's start-up then overlaps the current track's last moments. In the tests, the next clock started within one audio buffer, about 21 ms, of the last one stopping.
- **Nearly gapless, not sample-accurate.** The start-up varies a little from one transition to the next, so a transition can leave a few milliseconds of silence, or overlap by a few. True gapless needs the audio decoded and played by the app itself (§5).
- **Nothing changes in the backend or the database.** It's the provider, the queue's rules, and their tests.
- **One phase,** whose first check is by ear (§4).

## 2. What was measured

> Changed during implementation, see [Appendix A](#appendix-a-by-the-settled-clock-a-start-takes-about-240-ms).

On 2026-10-19, in the app, with the eight one-second files of the formats fixtures (FLAC, AAC and ALAC in M4A, ADTS AAC, WAV, AIFF, CBR and VBR MP3), played one after another, muted.

- **How:** each element's `currentTime` was read about once a millisecond. WebKit's clock moves in steps of about 21 ms, 1,024 frames at 48 kHz, so that's how fine these numbers are. Silence is the time from the last step of one track's clock to the first step of the next one's. A negative silence is an overlap.
- **What it can't tell:** what the speakers play. The clock is the closest thing to it a page can read: the protocol's files can't go through Web Audio, since they're from another origin.

| How the next track starts | Silence, by the clock |
|---|---|
| As the player does now: on `ended`, its file is set on the one element, which then plays | 256 to 365 ms |
| Loaded ahead in a second element, which plays on `ended` | 217 to 365 ms |
| Loaded ahead, and played 300 ms before the end | Overlaps of 124 to 170 ms |
| Loaded ahead, and played 250 ms before the end | Overlaps of 64 to 146 ms |
| Loaded ahead, and played 150 ms before the end | 0 to 3 ms, and once an overlap of 85 ms, from MP3 to MP3 |
| Loaded ahead, and played 120 ms before the end | 0 to 1 ms |

Split up, the first two rows give the same picture: `ended` fires 0 to 110 ms after the clock stops, and a play takes 255 to 342 ms to start the clock, whether its file was loaded ahead or not. With another element already playing, the start-up is shorter, which is why the lead that closes the gap is under half of it.

Not measured yet: real tracks, minutes long; the window in the background; and listening (§4).

## 3. How it works

> Changed during implementation, see [Appendix A](#appendix-a-by-the-settled-clock-a-start-takes-about-240-ms), [Appendix B](#appendix-b-the-queue-goes-on-when-the-current-track-ends), [Appendix C](#appendix-c-a-worker-times-the-handoff) and [Appendix D](#appendix-d-webkit-can-start-the-element-loaded-ahead).

- **Two elements.** The provider renders two `<audio>` elements. One plays the current entry. The other holds the entry that plays next, loaded ahead. At each handoff, they swap.
- **What plays next** is what the queue would go on to when the current track ends, by its rules (2_player.md §4.3): the next entry; the first after the last with repeat all; nothing after the last otherwise; and the same entry again with repeat one, loaded in the other element. A pure function in `lib/queue.ts`, `upNext(queue)`, says which, and has its tests.
- **Loading ahead.** Once the current track is playing, the other element loads the next one's file. When the queue changes what plays next, by Play next, Add to queue, a move, a removal, Clear, shuffle or repeat, it loads the new one, or empties.
- **The handoff.** When the current element has 120 ms left, by its `currentTime` and `duration`, the other element plays, and the queue goes on as it does at `ended`. The element that's ending finishes its last moments, then fires `ended`, which the provider ignores, since it's no longer the current one. If the handoff doesn't happen, because the next file isn't ready, a track's length is wrong, or the window is too throttled, `ended` goes on as it does now.
- **Timing it.** `timeupdate` fires about four times a second, too coarse for a 120 ms lead. Once it shows less than a second left, the provider checks the time once a millisecond until the lead, with a `MessageChannel` loop, which WebKit doesn't throttle when the window is hidden or behind others, as it does timers. So that loop runs for about a second per track, and only near its end.
- **Next, Previous and jumping** stay as they are. Next uses the element loaded ahead, when it holds the next entry, so it also starts sooner.
- **Everything else follows the current element:** the time and the progress bar, Previous's 3 seconds, `waiting`, and the error that skips a track (2_player.md §7.1). An error from the element loading ahead waits until its track would play, then shows its toast and skips it, as now.
- **The volume and mute** apply to both elements. **One sound at a time** (2_player.md §7.2) treats both as the player's.
- **Now Playing** follows whichever element plays: the provider already sets its actions again whenever its element starts playing (2_player.md Appendix C). The metadata follows the queue, as now.

## 4. Risks and checks

> Changed during implementation, see [Appendix A](#appendix-a-by-the-settled-clock-a-start-takes-about-240-ms) and [Appendix C](#appendix-c-a-worker-times-the-handoff).

Each is checked early in the phase, and anything that changes this design is recorded as an appendix.
- **By ear.** On the formats library's live tracks, which run into each other on the record, each transition should have no gap and no doubling. The lead (120 ms) is tuned until it's right, and stays one constant.
- **Overlaps.** When an element starts faster than the lead allows for, both play at once for a moment: 85 ms, once, in §2. Check whether that's heard on the live tracks, as a doubled beat or a jump in level. If it is, the lead gets shorter, trading it for a few milliseconds of silence.
- **In the background.** The handoff still happens on time with the window hidden, minimized, and behind another app.
- **A track's length.** The handoff trusts the element's `duration`. Check it on VBR MP3, whose length WebKit may estimate. A wrong one falls back to `ended`.
- **Now Playing** shows each track as it starts, and its buttons and the media keys still work after a handoff.
- **The CPU.** The `MessageChannel` loop runs a second per track. Check that it doesn't show in Activity Monitor.
- **Not this design's:** MP3 and AAC files carry silence their encoder added at their start and end. AVFoundation may trim it or not, per file. A gapless album in those formats can still have a short gap, which only the app's own decoder could remove (§5).

## 5. Alternatives considered

- **Loading ahead only,** starting the next track on `ended`. It saves the load, but not the start-up: 217 to 365 ms (§2).
- **Web Audio,** scheduling each track to the sample. It decodes a whole file into memory first, as 2_player.md §11 found: over 200 MB for ten minutes. And the protocol's files are from another origin, so Web Audio would only get silence from them.
- **Media Source Extensions:** one element, with each next track appended to the same buffer, which plays them without a gap and could make seeking exact (2_player.md Appendix A). The backend would have to repackage every format as fragmented MP4, and WebKit would have to accept FLAC and ALAC that way. A project of its own.
- **The app's own playback,** in Rust (2_player.md §3, B): sample-accurate, exact seeking, and Ogg Vorbis too. It costs a decoder, an audio output, and a native Now Playing. It's the way to true gapless, if nearly gapless isn't enough.

## 6. Phase

**Phase 0: Nearly gapless**
- `lib/queue.ts`: `upNext()` and the handoff, with their tests: the next entry, the last one with and without repeat all, repeat one, shuffle, and a queue changed after loading ahead.
- The provider: the second element, loading ahead, the handoff and its timing.
- The checks (§4), each recorded as an appendix where it changes this design.
- Docs: the player in `frontend.md`.
- Mark this design complete.
- *Exit:*
  - The live tracks play through with no gap or doubling heard, with the window in front, hidden, and behind another app.
  - Next, Previous, jumping, shuffle, repeat, errors and Now Playing work as they did.
  - `npm test` passes.

## 7. Settled questions

The first draft left these open. They were settled on 2026-10-19, when this design was taken up to be built:
1. **Nearly gapless now, or true gapless?** Nearly gapless, now: this design. True gapless means the app plays audio itself (§5), a much larger design, which would also bring exact seeking. It gets a design of its own if this one isn't enough.
2. **Always, or only within a release?** Always, from any track to any other, as Music does.

## Appendix A: By the settled clock, a start takes about 240 ms

- **Date:** 2026-10-19
- **Affects:** §1 Summary, §2 What was measured, §3 How it works, §4 Risks and checks
- **Was:** a play takes 255 to 342 ms to start the clock, but less with another element already playing: played 120 ms before the end, the next clock started within 0 to 1 ms of the last one stopping. So the lead is 120 ms, and a transition is within a few milliseconds.
- **Now:**
  - A starting clock's first readings run ahead of it. For about a third of a second after a play, WebKit moves `currentTime` on an estimate, then holds it back until AVFoundation's clock catches up. From then on, `performance.now()` minus `currentTime`, read at each `timeupdate`, stays within a millisecond for the rest of the track, and the element's `pause` at the track's end comes within 2 ms of where that line says the track ends. That settled line is the element's clock. §2's first steps were the estimate, about 100 ms early.
  - By the settled clock, a play takes 200 to 310 ms to start the clock, with a median of about 245 ms, and another element playing doesn't shorten it: medians of 239 and 241 ms, with and without, in the same session. Each file starts at one of a few levels about 25 ms apart (about 206, 232, 245, 272 and 285 ms), mostly the same one each time. About one start in 40 takes 50 to 250 ms longer.
  - So the lead is 240 ms. In the player, on the formats fixtures, transitions leave 5 to 48 ms of silence, with a median of 24 ms. Shuffled, they range from an overlap of 14 ms to 48 ms of silence, and repeat all's wrap from the last track to the first overlaps by 14 ms. On the formats library's live tracks, the two transitions leave 32 ms of silence, and an overlap of 27 ms.
  - So it's nearly gapless to within about 50 ms either way, not within a few milliseconds (§1). Whether that's heard is the check by ear (§4), which may still tune the lead.
  - By ear, on 2026-10-19, the transitions had no gap or doubling heard, and the player's other behavior was as before. The lead stays 240 ms.
  - An MP3's `duration` was 26 ms too long until its end, where WebKit corrected it (1.045 to 1.019 s), so its handoff came 26 ms late. That leaves a shorter silence, not an overlap.
  - These were measured with both elements at a volume of 0.0001 rather than muted. Muted, in a hidden window, macOS suspended the page for up to 13 seconds at a time, since nothing audible played, and the handoffs and the measurements stopped with it.
- **Why:** §2's numbers didn't hold in the player. With 120 ms of lead, transitions left 130 to 280 ms of silence by the settled clock.

## Appendix B: The queue goes on when the current track ends

- **Date:** 2026-10-19
- **Affects:** §3 How it works
- **Was:** when the current element has 120 ms left, the other element plays, and the queue goes on as it does at `ended`. The element that's ending fires `ended`, which the provider ignores.
- **Now:**
  - The handoff comes in two steps. At the lead, the other element plays, and nothing else happens. When the current track ends, at its `ended`, the elements swap, and the queue goes on with a `handoff` action: `ended` without a restart, since what's up next already plays from its start, even when it's the same entry.
  - Until then, the ending element is still the current one. Its `pause` at its end isn't the player pausing.
  - During those last moments, Pause, Previous, a jump, or a change to what's up next stops the next track, back at its start. Next keeps it playing.
  - A track whose `duration` was too short would play on over the next one, so it's cut off 250 ms after the end it was expected at.
  - The element loaded ahead and the code that runs both are in `components/player/elements.ts`, apart from the provider.
- **Why:** work on the main thread right after `play()` delays the start by as long as it lasts: 60 ms of it added 60 to 85 ms to most files' start-up. From 150 ms after the play, it added nothing. Going on with the queue at the play rendered the app right then, and in a debug build, start-ups in the player were 30 to 150 ms longer than outside it. And this way, the player shows the next track when it's heard.

## Appendix C: A worker times the handoff

- **Date:** 2026-10-19
- **Affects:** §3 How it works, §4 Risks and checks
- **Was:** in a track's last second, a `MessageChannel` loop checks the time once a millisecond.
- **Now:**
  - In WKWebView, a `MessageChannel` loop runs 9 to 10 times a millisecond, as fast as the page can post. While it ran, the web content process used 70 to 83% of a core, which shows in Activity Monitor (§4).
  - Instead, a dedicated worker's 1 ms `setTimeout` ticks, and the time is checked at each tick. With the window hidden and the page playing audio, the ticks came every 5 to 6 ms, 99% of them within 7 ms, at 2 to 6% of a core.
  - A worker that has sat idle in the hidden page for minutes ticks much more slowly: after 270 s, every 82 ms on median, and up to 154 ms. With one worker for the whole queue, the live tracks' second handoff, 268 s after the first, didn't happen. So each watch starts a new worker, and ends it when it stops. A new one ticked every 5 ms, even in a page that had been hidden for most of an hour.
  - From the last tick before the lead, the rest of the wait, up to 12 ms, is spent on the main thread. Blocking before `play()` doesn't delay the start.
  - The page's own timers are no use: hidden, they ran about once a second, even with audio playing.
  - Vite bundles the worker as a file of its own, which the CSP's `default-src 'self'` allows.
- **Why:** the CPU check (§4).

## Appendix D: WebKit can start the element loaded ahead

- **Date:** 2026-10-19
- **Affects:** §3 How it works
- **Was:** the element loaded ahead plays only at the handoff, or when Next swaps it in.
- **Now:**
  - Twice, as the window came into view, WebKit started the paused element loaded ahead by itself, a second before its handoff, so both tracks played at once.
  - So the other element plays only for a handoff. When it starts any other way, it's paused and put back at its start at once, long before its clock starts. Calling `play()` on it from outside the player checked that: it was paused at its start within 2 ms, and its handoff came on time.
- **Why:** it's WebKit's own behavior, whose cause wasn't found: the window couldn't be brought into view on demand to test it.
