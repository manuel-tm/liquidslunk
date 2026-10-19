import {
  createContext,
  type ReactNode,
  type SyntheticEvent,
  useCallback,
  useContext,
  useEffect,
  useEffectEvent,
  useMemo,
  useReducer,
  useRef,
  useState,
  useSyncExternalStore,
} from "react"
import { toast } from "sonner"

import {
  commands,
  events,
  type PlayerControl,
  type PlayerTrack,
} from "@/bindings"
import { shownName } from "@/lib/credited"
import { fileUrl, useCommand } from "@/lib/query"
import {
  currentEntry,
  type Entry,
  nextRepeat,
  queueReducer,
  type Repeat,
  restoredQueue,
  savedQueue,
  upNext,
} from "@/lib/queue"

import { playerElements } from "./elements"

// The browser remembers the volume, shuffle and repeat, for every library, as
// it does the theme, and the queue per library, since ids belong to one
// (2_player.md §5.7)
const VOLUME = "slunk.player.volume"
const SHUFFLE = "slunk.player.shuffle"
const REPEAT = "slunk.player.repeat"
const QUEUE = "slunk.player.queue"
// ⌘↑ and ⌘↓, and the Controls menu's Volume Up and Down, change the volume
// by this much
const VOLUME_STEP = 0.1

type Volume = { volume: number; muted: boolean }

/** The player (2_player.md §4.1): what's queued, and what to do with it. */
export type Player = {
  entries: Entry[]
  current: Entry | null
  /** What the player shows of the current track, once it has loaded */
  track: PlayerTrack | null
  playing: boolean
  /** Whether the element waits for data before it can play */
  waiting: boolean
  /** From 0 to 1 */
  volume: number
  muted: boolean
  shuffle: boolean
  repeat: Repeat
  /** What the player shows of each queued track, by id, once loaded */
  tracks: Map<number, PlayerTrack>
  /**
   * A new queue of these tracks, playing from the one at `start`: shuffled,
   * that one first, or a random one without `start`
   */
  play: (trackIds: number[], start?: number) => void
  /** Right after the current track; with nothing queued, a queue, paused */
  playNext: (trackIds: number[]) => void
  /** At the end; with nothing queued, a queue, paused */
  addToQueue: (trackIds: number[]) => void
  toggle: () => void
  setPlaying: (playing: boolean) => void
  next: () => void
  previous: () => void
  jump: (entry: Entry) => void
  /** Moves an entry `by` places in the order they play */
  move: (entry: Entry, by: number) => void
  remove: (entry: Entry) => void
  clear: () => void
  toggleShuffle: () => void
  /** Off, all, one, then off again */
  cycleRepeat: () => void
  setVolume: (volume: number) => void
  toggleMuted: () => void
}

/** How far into the current track the element is, in seconds. */
export type PlayerTime = {
  time: number
  /** The element's, once it has read the file, else NaN */
  duration: number
}

const PlayerContext = createContext<Player | null>(null)
const TimeContext = createContext<ReturnType<typeof timeStore> | null>(null)

/**
 * The element's time, as a store for `useSyncExternalStore`. Only what shows
 * the time subscribes, so the rest of the app doesn't render again on every
 * `timeupdate`, about four times a second.
 */
function timeStore() {
  let snapshot: PlayerTime = { time: 0, duration: NaN }
  const listeners = new Set<() => void>()
  return {
    get: () => snapshot,
    set(time: number, duration: number) {
      if (time === snapshot.time && Object.is(duration, snapshot.duration)) {
        return
      }
      snapshot = { time, duration }
      for (const listener of listeners) listener()
    },
    subscribe(listener: () => void) {
      listeners.add(listener)
      return () => void listeners.delete(listener)
    },
  }
}

/** What the browser remembers under `key`, or null. */
function recall(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

/** Remembers `value` under `key`, or forgets it for null. */
function remember(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key)
    else localStorage.setItem(key, value)
  } catch {
    // Remembering is a convenience
  }
}

/** The library's remembered queue, with shuffle and repeat, paused. */
function rememberedQueue(library: string) {
  const repeat = recall(REPEAT)
  let saved: unknown = null
  try {
    saved = JSON.parse(recall(`${QUEUE}:${library}`) ?? "null")
  } catch {
    // Something else under the key: no queue
  }
  return restoredQueue(saved, {
    shuffle: recall(SHUFFLE) === "true",
    repeat: repeat === "all" || repeat === "one" ? repeat : "off",
  })
}

/** A seed for shuffling. */
function seed() {
  return Math.floor(Math.random() * 2 ** 32)
}

function rememberedVolume(): Volume {
  try {
    const saved = JSON.parse(
      localStorage.getItem(VOLUME) ?? "null"
    ) as Partial<Volume> | null
    const volume = saved?.volume
    if (typeof volume === "number" && volume >= 0 && volume <= 1) {
      return { volume, muted: saved?.muted === true }
    }
  } catch {
    // Remembering is a convenience
  }
  return { volume: 1, muted: false }
}

// Controls that Space presses, and fields that arrows move the caret in.
// Links aren't controls here: Space only scrolls from one.
const CONTROLS =
  "input, textarea, select, button, summary, [contenteditable='true'], [role='button'], [role='checkbox'], [role='combobox'], [role='menuitem'], [role='menuitemcheckbox'], [role='menuitemradio'], [role='option'], [role='radio'], [role='slider'], [role='switch'], [role='tab']"
const FIELDS = "input, textarea, select, [contenteditable='true']"

function within(target: EventTarget | null, selector: string) {
  return target instanceof Element && target.closest(selector) !== null
}

/**
 * The app's one player (2_player.md §4.1): the queue, and the two `<audio>`
 * elements that play it, one ahead of the other so tracks follow each other
 * with nearly no gap (3_gapless.md). It's inside `AppLayout`'s keyed
 * `SidebarProvider`, so it keeps playing from page to page, and stops when
 * the library closes or switches. The sidebar only shows its controls, so a
 * hidden sidebar doesn't stop the music. It starts with the library's
 * remembered queue, paused.
 */
export function PlayerProvider({
  library,
  children,
}: {
  /** The open library's path, which its remembered queue is kept under */
  library: string
  children: ReactNode
}) {
  const [queue, dispatch] = useReducer(queueReducer, library, rememberedQueue)
  const [waiting, setWaiting] = useState(false)
  const [{ volume, muted }, setVolumeState] = useState(rememberedVolume)
  const [time] = useState(timeStore)
  // The two elements: the current one, and the one loaded ahead. Each swap
  // asks for what's up next again.
  const [swaps, setSwaps] = useState(0)
  const [elements] = useState(() =>
    playerElements(
      () => dispatch({ type: "handoff" }),
      (element) => {
        setWaiting(false)
        setSwaps((swaps) => swaps + 1)
        time.set(element.currentTime, element.duration)
      }
    )
  )

  // What the player shows of each queued track. Saves fetch it again, and
  // the last data stays while the queue's ids change.
  const ids = useMemo(
    () => [...new Set(queue.entries.map((entry) => entry.trackId))],
    [queue.entries]
  )
  const { data, isPlaceholderData } = useCommand(
    "playerTracks",
    { ids },
    { enabled: ids.length > 0, keepPrevious: true }
  )
  const tracks = useMemo(
    () => new Map((data ?? []).map((track) => [track.id, track])),
    [data]
  )
  const current = currentEntry(queue)
  const track = (current && tracks.get(current.trackId)) ?? null

  // Deleted tracks aren't in the answer, so they leave the queue
  useEffect(() => {
    if (!data || isPlaceholderData) return
    const found = data.map((track) => track.id)
    if (ids.some((id) => !found.includes(id))) {
      dispatch({ type: "keep", trackIds: found })
    }
  }, [data, isPlaceholderData, ids])

  // The current element follows the queue. Its source changes only when the
  // current entry does, never on a fetch, so a save never interrupts what
  // plays.
  const currentKey = current?.key ?? null
  const currentSrc = track && fileUrl(track.file.id)
  useEffect(() => {
    elements.follow(
      currentKey === null ? null : { key: currentKey, src: currentSrc },
      queue.playing
    )
  }, [elements, currentKey, currentSrc, queue.playing])

  // The other one loads what's up next (3_gapless.md §3), and loads again
  // whenever the queue changes what that is, or the elements swap
  const ahead = upNext(queue)
  const nextKey = ahead?.key ?? null
  const nextTrack = ahead && tracks.get(ahead.trackId)
  const nextSrc = nextTrack ? fileUrl(nextTrack.file.id) : null
  useEffect(() => {
    elements.setAhead(nextKey === null ? null : { key: nextKey, src: nextSrc })
  }, [elements, nextKey, nextSrc, swaps])
  useEffect(() => () => elements.dispose(), [elements])

  // Remembered as it changes (2_player.md §5.7)
  const saved = JSON.stringify(savedQueue(queue))
  useEffect(() => {
    remember(`${QUEUE}:${library}`, saved === "null" ? null : saved)
  }, [library, saved])
  useEffect(() => {
    remember(SHUFFLE, String(queue.shuffle))
    remember(REPEAT, queue.repeat)
  }, [queue.shuffle, queue.repeat])

  // Back to the start of the current track: Previous, or the end of the queue
  const restarts = useRef(queue.restarts)
  useEffect(() => {
    if (queue.restarts === restarts.current) return
    restarts.current = queue.restarts
    elements.restart()
  }, [elements, queue.restarts])

  useEffect(() => {
    elements.setVolume(volume, muted)
    remember(VOLUME, JSON.stringify({ volume, muted }))
  }, [elements, volume, muted])

  const play = useCallback(
    (trackIds: number[], start?: number) =>
      dispatch({ type: "play", trackIds, start, seed: seed() }),
    []
  )
  const playNext = useCallback(
    (trackIds: number[]) => dispatch({ type: "playNext", trackIds }),
    []
  )
  const addToQueue = useCallback(
    (trackIds: number[]) => dispatch({ type: "add", trackIds }),
    []
  )
  const toggle = useCallback(() => dispatch({ type: "toggle" }), [])
  const setPlaying = useCallback(
    (playing: boolean) => dispatch({ type: "setPlaying", playing }),
    []
  )
  const next = useCallback(() => dispatch({ type: "next" }), [])
  const previous = useCallback(
    () =>
      dispatch({
        type: "previous",
        time: elements.current()?.currentTime ?? 0,
      }),
    [elements]
  )
  const jump = useCallback(
    (entry: Entry) => dispatch({ type: "jump", key: entry.key }),
    []
  )
  const move = useCallback(
    (entry: Entry, by: number) =>
      dispatch({ type: "move", key: entry.key, by }),
    []
  )
  const remove = useCallback(
    (entry: Entry) => dispatch({ type: "remove", key: entry.key }),
    []
  )
  const clear = useCallback(() => dispatch({ type: "clear" }), [])
  const toggleShuffle = useCallback(
    () =>
      dispatch({ type: "setShuffle", shuffle: !queue.shuffle, seed: seed() }),
    [queue.shuffle]
  )
  const cycleRepeat = useCallback(
    () => dispatch({ type: "setRepeat", repeat: nextRepeat(queue.repeat) }),
    [queue.repeat]
  )
  // In hundredths, so steps of 10% don't drift: 0.8 - 0.1 is 0.7000000000000001
  const setVolume = useCallback(
    (volume: number) =>
      setVolumeState({
        volume: Math.round(Math.min(1, Math.max(0, volume)) * 100) / 100,
        muted: false,
      }),
    []
  )
  const toggleMuted = useCallback(
    () => setVolumeState((state) => ({ ...state, muted: !state.muted })),
    []
  )

  // Space, ⌘→, ⌘←, ⌘↑ and ⌘↓, as in Music (2_player.md §4.4). The Controls
  // menu shows the ⌘ ones, but only gets a key the page leaves: these take it
  // first, before WebKit scrolls to the end of a page for ⌘↓ (Appendix E).
  const queued = current !== null
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!queued || event.altKey || event.ctrlKey || event.shiftKey) return
      if (!event.metaKey) {
        // Space presses a focused control instead
        if (event.key !== " " || event.repeat) return
        if (within(event.target, CONTROLS)) return
        event.preventDefault()
        toggle()
        return
      }
      // In a field, they move the caret
      if (within(event.target, FIELDS)) return
      const actions: Record<string, () => void> = {
        ArrowRight: next,
        ArrowLeft: previous,
        ArrowUp: () => setVolume(volume + VOLUME_STEP),
        ArrowDown: () => setVolume(volume - VOLUME_STEP),
      }
      const action = actions[event.key]
      if (!action) return
      event.preventDefault()
      action()
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [queued, toggle, next, previous, setVolume, volume])

  // One sound at a time (2_player.md §7.2): the file page's preview pauses
  // the player, and the player pauses it. Both of the player's elements are
  // the player's. Media events don't bubble, so this listens in the capture
  // phase.
  useEffect(() => {
    function onPlay(event: Event) {
      const started = event.target
      if (!(started instanceof HTMLMediaElement)) return
      if (!elements.ours(started)) {
        elements.pause()
        return
      }
      for (const other of document.querySelectorAll("audio, video")) {
        if (!elements.ours(other)) (other as HTMLMediaElement).pause()
      }
    }
    document.addEventListener("play", onPlay, true)
    return () => document.removeEventListener("play", onPlay, true)
  }, [elements])

  // Now Playing and the media keys (2_player.md §8.1)
  const title = track?.title
  const artist = track?.performers
    .map((performer) => shownName(performer).name)
    .join(", ")
  const album = track?.release.title
  const cover = track?.cover?.id
  useEffect(() => {
    if (!("mediaSession" in navigator)) return
    navigator.mediaSession.metadata =
      title === undefined
        ? null
        : new MediaMetadata({
            title,
            artist,
            album,
            artwork: cover === undefined ? [] : [{ src: fileUrl(cover) }],
          })
  }, [title, artist, album, cover])
  useEffect(() => {
    if (!("mediaSession" in navigator)) return
    navigator.mediaSession.playbackState = !queued
      ? "none"
      : queue.playing
        ? "playing"
        : "paused"
  }, [queued, queue.playing])
  // Now Playing offers exactly the actions with handlers: previous and next,
  // and no seeking (2_player.md Appendix B). WebKit drops a handler set before
  // a media element has registered with its Now Playing, and offers its own
  // actions instead: a scrubber and 15-second skips. So the handlers are set
  // again whenever an element starts playing (Appendix C): either one, since
  // they take turns (3_gapless.md §3).
  const setActions = useCallback(() => {
    if (!("mediaSession" in navigator)) return
    const handlers: [MediaSessionAction, MediaSessionActionHandler][] = [
      ["play", () => setPlaying(true)],
      ["pause", () => setPlaying(false)],
      ["previoustrack", previous],
      ["nexttrack", next],
    ]
    for (const [action, handler] of handlers) {
      navigator.mediaSession.setActionHandler(action, handler)
    }
  }, [setPlaying, previous, next])
  useEffect(() => {
    if (!("mediaSession" in navigator)) return
    const session = navigator.mediaSession
    setActions()
    return () => {
      for (const action of [
        "play",
        "pause",
        "previoustrack",
        "nexttrack",
      ] satisfies MediaSessionAction[]) {
        session.setActionHandler(action, null)
      }
      session.metadata = null
      session.playbackState = "none"
    }
  }, [setActions])

  // The Controls menu (2_player.md §8.2) shows the player as it is. It's off
  // with nothing queued, and so at Welcome, when the provider unmounts.
  useEffect(() => {
    void commands.setControlsMenu({
      playing: queue.playing,
      queued,
      shuffle: queue.shuffle,
      repeat: queue.repeat,
    })
  }, [queue.playing, queued, queue.shuffle, queue.repeat])
  useEffect(
    () => () =>
      void commands.setControlsMenu({
        playing: false,
        queued: false,
        shuffle: false,
        repeat: "off",
      }),
    []
  )
  // Its items do what the player's buttons do, with the sidebar hidden too
  const onControl = useEffectEvent((control: PlayerControl) => {
    switch (control.type) {
      case "toggle":
        return toggle()
      case "next":
        return next()
      case "previous":
        return previous()
      case "shuffle":
        return toggleShuffle()
      case "repeat":
        return dispatch({ type: "setRepeat", repeat: control.mode })
      case "volumeUp":
        return setVolume(volume + VOLUME_STEP)
      case "volumeDown":
        return setVolume(volume - VOLUME_STEP)
    }
  })
  useEffect(() => {
    const listening = events.playerControl.listen((event) =>
      onControl(event.payload)
    )
    return () => void listening.then((unlisten) => unlisten())
  }, [])

  // Only the current element's events count. The other one loads ahead, and
  // starts the next track just before the current one ends (3_gapless.md §3).
  const isCurrent = (event: SyntheticEvent<HTMLAudioElement>) =>
    elements.isCurrent(event.currentTarget)

  // A track that can't play is skipped (2_player.md §7.1). One that fails to
  // load ahead is, once it would play.
  function onError(event: SyntheticEvent<HTMLAudioElement>) {
    if (!isCurrent(event)) return elements.failed(event.currentTarget)
    if (!current || !event.currentTarget.getAttribute("src")) return
    toast.error(`Can't play ${track?.title ?? "the track"}`, {
      description: (
        <>
          {track && <span className="break-all">{track.file.path}</span>} The
          file is missing, or its format can't be played here.
        </>
      ),
    })
    setWaiting(false)
    dispatch({ type: "failed" })
  }

  function onTime(event: SyntheticEvent<HTMLAudioElement>) {
    if (!isCurrent(event)) return
    const element = event.currentTarget
    time.set(element.currentTime, element.duration)
  }

  const player = useMemo<Player>(
    () => ({
      entries: queue.entries,
      current,
      track,
      playing: queue.playing,
      waiting,
      volume,
      muted,
      shuffle: queue.shuffle,
      repeat: queue.repeat,
      tracks,
      play,
      playNext,
      addToQueue,
      toggle,
      setPlaying,
      next,
      previous,
      jump,
      move,
      remove,
      clear,
      toggleShuffle,
      cycleRepeat,
      setVolume,
      toggleMuted,
    }),
    [
      queue.entries,
      current,
      track,
      queue.playing,
      waiting,
      volume,
      muted,
      queue.shuffle,
      queue.repeat,
      tracks,
      play,
      playNext,
      addToQueue,
      toggle,
      setPlaying,
      next,
      previous,
      jump,
      move,
      remove,
      clear,
      toggleShuffle,
      cycleRepeat,
      setVolume,
      toggleMuted,
    ]
  )

  return (
    <PlayerContext.Provider value={player}>
      <TimeContext.Provider value={time}>
        {children}
        {[0, 1].map((index) => (
          <audio
            key={index}
            ref={(element) => elements.attach(index, element)}
            preload="auto"
            className="hidden"
            onPlay={(event) => {
              if (!isCurrent(event)) {
                return elements.otherPlayed(event.currentTarget)
              }
              dispatch({ type: "setPlaying", playing: true })
            }}
            onPause={(event) => {
              // A track that ends says it paused first. Once the next one has
              // started, that's not the player pausing.
              const element = event.currentTarget
              if (!isCurrent(event) || elements.isEnding(element)) return
              setWaiting(false)
              dispatch({ type: "setPlaying", playing: false })
            }}
            onPlaying={(event) => {
              setActions()
              if (!isCurrent(event)) return
              setWaiting(false)
              dispatch({ type: "started" })
              elements.playing(event.currentTarget)
            }}
            onWaiting={(event) => isCurrent(event) && setWaiting(true)}
            onCanPlay={(event) => isCurrent(event) && setWaiting(false)}
            // The next track plays: it already has, when it started just
            // before this one ended
            onEnded={(event) => {
              if (!isCurrent(event)) return
              if (!elements.ended(event.currentTarget)) {
                dispatch({ type: "ended" })
              }
            }}
            onError={onError}
            onTimeUpdate={(event) => {
              onTime(event)
              elements.timeUpdated(event.currentTarget)
            }}
            onDurationChange={onTime}
            onSeeked={onTime}
            onEmptied={(event) => {
              if (!isCurrent(event)) return
              setWaiting(false)
              onTime(event)
            }}
          />
        ))}
      </TimeContext.Provider>
    </PlayerContext.Provider>
  )
}

/** The player, for its controls and the pages that play tracks. */
export function usePlayer(): Player {
  const player = useContext(PlayerContext)
  if (!player) throw new Error("usePlayer must be used within PlayerProvider")
  return player
}

/** The current track's time, for the progress bar and its times. */
export function usePlayerTime(): PlayerTime {
  const store = useContext(TimeContext)
  if (!store)
    throw new Error("usePlayerTime must be used within PlayerProvider")
  return useSyncExternalStore(store.subscribe, store.get)
}
