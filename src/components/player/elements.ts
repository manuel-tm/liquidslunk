// The player's two `<audio>` elements, for nearly gapless playback
// (frontend.md, "Nearly gapless"). WebKit takes about a quarter of a second to
// start an element's clock, loaded or not, so the next track starts in a
// second element that long before the current one ends, and its start-up
// overlaps the current track's last moments.

import Ticks from "./ticks.worker?worker"

/**
 * How long before the current track ends the next one starts, in seconds, by
 * the current element's clock: about how long WebKit takes to start an
 * element's clock, so the next one starts as the current one stops.
 */
export const LEAD = 0.24
// From this long before a track ends, its time is checked at each of the
// worker's ticks, for the handoff: `timeupdate`, about four times a second,
// is too coarse
const WATCH = 1
// The ticks come every few milliseconds. From the last one before the lead,
// the handoff waits out the rest, up to this long, on the main thread.
const SPIN = 0.012
// The element that's ending is cut off once it has played this long past
// where its track should have ended: its length was wrong, and it would play
// over the next track
const OVERRUN = 0.25

/** An entry, with its file's URL once the player knows it. */
export type Source = { key: number; src: string | null }

/**
 * The player's two elements. The current one plays the
 * current entry. The other holds what's up next, loaded ahead.
 *
 * The handoff comes in two steps. When the current track has `LEAD` left, the
 * other element starts, and nothing else happens: work on the main thread
 * right after `play()`, like a render, delays the start by as long as it
 * takes. When the current track ends, the elements swap, and `handedOff` is
 * called, for the queue to go on. They also swap when Next or a jump finds its
 * entry loaded ahead.
 *
 * Both elements' events reach the provider, which acts on the current one's,
 * and passes the rest here. `shown` is called whenever the other element
 * becomes the current one: the provider shows its time, and says what's up
 * next again, through `setAhead()`.
 */
export function playerElements(
  handedOff: () => void,
  shown: (element: HTMLAudioElement) => void
) {
  const elements: (HTMLAudioElement | null)[] = [null, null]
  let active = 0
  // The entry each element holds, or null once it's empty or has played
  const holds: (number | null)[] = [null, null]
  // Whether each one's file failed to load
  const broken = [false, false]
  // Whether the current element has started its file: only then does the
  // other one load ahead
  let started = false
  // What's up next, or undefined from a swap until the provider says again,
  // since what was up next is now current
  let ahead: Source | null | undefined
  // While the other element has started what's up next and the current one
  // plays its last moments: when, on `performance.now()`'s clock, the current
  // one should have ended by
  let handoff: { endsBy: number } | null = null
  // The worker that ticks while the watch runs
  let ticks: Worker | null = null

  const other = () => 1 - active

  function load(index: number, { key, src }: Source) {
    const element = elements[index]
    if (!element || src === null) return
    element.src = src
    holds[index] = key
    broken[index] = false
  }

  function empty(index: number) {
    holds[index] = null
    broken[index] = false
    const element = elements[index]
    if (!element?.getAttribute("src")) return
    element.pause()
    element.removeAttribute("src")
    element.load()
  }

  /**
   * The other element loads what's up next, or empties: once the current one
   * has started, and unless it's already playing it.
   */
  function loadAhead() {
    if (!started || handoff || ahead === undefined) return
    const index = other()
    if (!ahead) return empty(index)
    if (holds[index] !== ahead.key) load(index, ahead)
  }

  /**
   * The other element becomes the current one, playing or not. The one that
   * was current loads ahead once the provider says what's up next now.
   */
  function swap() {
    const from = active
    active = other()
    holds[from] = null
    ahead = undefined
    elements[from]?.pause()
    const element = elements[active]
    started = !!element && !element.paused
    if (element) shown(element)
  }

  /**
   * The handoff stops before the swap: the other element stops, back at its
   * start, still loaded ahead.
   */
  function cancel() {
    if (!handoff) return
    handoff = null
    const element = elements[other()]
    if (!element) return
    element.pause()
    element.currentTime = 0
  }

  /**
   * The current track ended, or overran: the other element, which already
   * plays, becomes the current one. False when it can't play, and the track
   * ending should go on as it does without a handoff.
   */
  function complete(): boolean {
    if (!handoff) return false
    handoff = null
    if (broken[other()]) return false
    swap()
    handedOff()
    return true
  }

  /** Whether the other element can start what's up next now. */
  function ready() {
    const index = other()
    const element = elements[index]
    return (
      !!element &&
      !!ahead &&
      holds[index] === ahead.key &&
      !broken[index] &&
      element.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA
    )
  }

  /** One check of the time: whether to check again. */
  function tick(): boolean {
    const element = elements[active]
    if (!element) return false
    if (handoff) {
      if (performance.now() <= handoff.endsBy) return true
      // Its length was wrong: it's cut off, unless the next one can't play
      complete()
      return false
    }
    if (element.paused || !ahead) return false
    const left = element.duration - element.currentTime
    const wait = left - LEAD
    if (wait > SPIN) return left <= WATCH
    // Not loaded yet: it can still start late, or at `ended`
    if (!ready()) return left > 0 && !broken[other()]
    // Blocking the main thread before `play()` doesn't delay the start
    const lead = performance.now() + wait * 1000
    while (performance.now() < lead) {
      // The rest of the wait, too short for another tick
    }
    elements[other()]?.play().catch(() => {})
    handoff = { endsBy: lead + (LEAD + OVERRUN) * 1000 }
    return true
  }

  /**
   * Checks the time at each of a worker's ticks, every few milliseconds,
   * while a handoff is near or under way. A worker, since WebKit throttles
   * the page's own timers in a hidden window, and a new one each time, since
   * it slows the ticks of one that has sat idle there for minutes.
   */
  function watch() {
    if (ticks) return
    const worker = new Ticks()
    worker.onmessage = () => {
      if (ticks === worker && !tick()) stop()
    }
    ticks = worker
  }

  function stop() {
    ticks?.terminate()
    ticks = null
  }

  /**
   * Whether the current element nears its end with something up next, and
   * the watch should run.
   */
  function near(element: HTMLAudioElement) {
    return (
      element === elements[active] &&
      !element.paused &&
      !!ahead &&
      element.duration - element.currentTime <= WATCH
    )
  }

  return {
    /** For each element's `ref`. */
    attach(index: number, element: HTMLAudioElement | null) {
      elements[index] = element
    },
    /** The element that plays the current entry. */
    current: (): HTMLAudioElement | null => elements[active],
    /** Whether `element` plays the current entry, so its events count. */
    isCurrent: (element: HTMLAudioElement) => element === elements[active],
    /**
     * Whether `element` is the current one, playing its last moments after
     * the next track started: its `pause` is its end, not the player's.
     */
    isEnding: (element: HTMLAudioElement) =>
      element === elements[active] && handoff !== null,
    /** Whether `element` is one of the player's. */
    ours: (element: Element) => elements.includes(element as HTMLAudioElement),

    /**
     * The current element follows the queue: it plays `current`, or pauses,
     * or empties with nothing queued. An entry loaded ahead swaps in. A
     * handoff under way goes on only while the queue still plays the track
     * that's ending.
     */
    follow(current: Source | null, playing: boolean) {
      if (!current) {
        handoff = null
        stop()
        started = false
        empty(0)
        empty(1)
        return
      }
      if (handoff) {
        if (current.key === holds[active] && playing) return
        // Next, to what has already started: it goes on
        if (current.key === holds[other()] && playing) handoff = null
        else cancel()
      }
      if (holds[active] !== current.key) {
        if (holds[other()] === current.key && !broken[other()]) swap()
        else if (current.src === null) return
        else {
          load(active, current)
          started = false
        }
      }
      const element = elements[active]
      if (!element) return
      if (playing) {
        // Changing the source aborts a play() that hasn't started yet
        if (element.paused) element.play().catch(() => {})
      } else if (!element.paused) {
        element.pause()
      }
    },

    /**
     * What's up next, which the other element loads ahead: whenever it
     * changes, and after each swap.
     */
    setAhead(next: Source | null) {
      ahead = next
      if (handoff && next?.key !== holds[other()]) cancel()
      loadAhead()
    },

    /** An element started playing. */
    playing(element: HTMLAudioElement) {
      if (element !== elements[active]) return
      started = true
      loadAhead()
      if (near(element)) watch()
    },

    /**
     * The other element started playing. It plays only for a handoff: WebKit
     * sometimes starts the one loaded ahead by itself, as the window comes
     * into view, and it's stopped, back at its start, long before it's heard.
     */
    otherPlayed(element: HTMLAudioElement) {
      if (element === elements[active] || handoff) return
      element.pause()
      element.currentTime = 0
    },

    /** An element's time moved. */
    timeUpdated(element: HTMLAudioElement) {
      if (near(element)) watch()
    },

    /**
     * An element ended. The current one's end completes a handoff under way:
     * true then, and the queue goes on through `handedOff`, not `ended`.
     */
    ended(element: HTMLAudioElement): boolean {
      return element === elements[active] && complete()
    },

    /**
     * The other element's file failed to load. Its track is skipped when it
     * would play: the current element tries it then, and fails as it would
     * have.
     */
    failed(element: HTMLAudioElement) {
      const index = elements.indexOf(element)
      if (index !== active && element.getAttribute("src")) broken[index] = true
    },

    /** The current track goes back to its start. */
    restart() {
      cancel()
      const element = elements[active]
      if (element) element.currentTime = 0
    },

    /** Another media element started: the player's pause. */
    pause() {
      cancel()
      for (const element of elements) element?.pause()
    },

    setVolume(volume: number, muted: boolean) {
      for (const element of elements) {
        if (!element) continue
        element.volume = volume
        element.muted = muted
      }
    },

    /** The provider unmounts: the watch stops. */
    dispose: stop,
  }
}

export type PlayerElements = ReturnType<typeof playerElements>
