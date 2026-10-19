import {
  createContext,
  type ReactNode,
  type SyntheticEvent,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
  useSyncExternalStore,
} from "react"
import { toast } from "sonner"

import type { PlayerTrack } from "@/bindings"
import { shownName } from "@/lib/credited"
import { fileUrl, useCommand } from "@/lib/query"
import { currentEntry, emptyQueue, type Entry, queueReducer } from "@/lib/queue"

// The browser remembers the volume, for every library, as it does the theme
const VOLUME = "slunk.player.volume"
// ⌘↑ and ⌘↓ change the volume by this much
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
  /** A new queue of these tracks, playing from the one at `start` */
  play: (trackIds: number[], start?: number) => void
  toggle: () => void
  setPlaying: (playing: boolean) => void
  next: () => void
  previous: () => void
  jump: (entry: Entry) => void
  remove: (entry: Entry) => void
  clear: () => void
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
 * The app's one player (2_player.md §4.1): the queue, and the one `<audio>`
 * element that plays it. It's inside `AppLayout`'s keyed `SidebarProvider`,
 * so it keeps playing from page to page, and stops when the library closes
 * or switches. The sidebar only shows its controls, so a hidden sidebar
 * doesn't stop the music.
 */
export function PlayerProvider({ children }: { children: ReactNode }) {
  const audio = useRef<HTMLAudioElement>(null)
  const [queue, dispatch] = useReducer(queueReducer, emptyQueue)
  const [waiting, setWaiting] = useState(false)
  const [{ volume, muted }, setVolumeState] = useState(rememberedVolume)
  const [time] = useState(timeStore)

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

  // The element follows the queue. Its source changes only when the current
  // entry does, never on a fetch, so a save never interrupts what plays.
  const loaded = useRef<number | null>(null)
  useEffect(() => {
    const element = audio.current
    if (!element) return
    if (!current) {
      if (loaded.current !== null) {
        loaded.current = null
        element.pause()
        element.removeAttribute("src")
        element.load()
      }
      return
    }
    if (loaded.current !== current.key) {
      if (!track) return
      loaded.current = current.key
      element.src = fileUrl(track.file.id)
    }
    if (queue.playing) {
      if (element.paused) {
        // Changing the source aborts a play() that hasn't started yet
        element.play().catch(() => {})
      }
    } else if (!element.paused) {
      element.pause()
    }
  }, [current, track, queue.playing])

  // Back to the start of the current track: Previous, or the end of the queue
  const restarts = useRef(queue.restarts)
  useEffect(() => {
    if (queue.restarts === restarts.current) return
    restarts.current = queue.restarts
    if (audio.current) audio.current.currentTime = 0
  }, [queue.restarts])

  useEffect(() => {
    const element = audio.current
    if (!element) return
    element.volume = volume
    element.muted = muted
    try {
      localStorage.setItem(VOLUME, JSON.stringify({ volume, muted }))
    } catch {
      // Remembering is a convenience
    }
  }, [volume, muted])

  const play = useCallback(
    (trackIds: number[], start = 0) =>
      dispatch({ type: "play", trackIds, start }),
    []
  )
  const toggle = useCallback(() => dispatch({ type: "toggle" }), [])
  const setPlaying = useCallback(
    (playing: boolean) => dispatch({ type: "setPlaying", playing }),
    []
  )
  const next = useCallback(() => dispatch({ type: "next" }), [])
  const previous = useCallback(
    () => dispatch({ type: "previous", time: audio.current?.currentTime ?? 0 }),
    []
  )
  const jump = useCallback(
    (entry: Entry) => dispatch({ type: "jump", key: entry.key }),
    []
  )
  const remove = useCallback(
    (entry: Entry) => dispatch({ type: "remove", key: entry.key }),
    []
  )
  const clear = useCallback(() => dispatch({ type: "clear" }), [])
  const setVolume = useCallback(
    (volume: number) =>
      setVolumeState({
        volume: Math.min(1, Math.max(0, volume)),
        muted: false,
      }),
    []
  )
  const toggleMuted = useCallback(
    () => setVolumeState((state) => ({ ...state, muted: !state.muted })),
    []
  )

  // Space, ⌘→, ⌘←, ⌘↑ and ⌘↓, as in Music (2_player.md §4.4)
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
  // the player, and the player pauses it. Media events don't bubble, so this
  // listens in the capture phase.
  useEffect(() => {
    function onPlay(event: Event) {
      const element = audio.current
      const started = event.target
      if (!element || !(started instanceof HTMLMediaElement)) return
      if (started !== element) {
        element.pause()
        return
      }
      for (const other of document.querySelectorAll("audio, video")) {
        if (other !== element) (other as HTMLMediaElement).pause()
      }
    }
    document.addEventListener("play", onPlay, true)
    return () => document.removeEventListener("play", onPlay, true)
  }, [])

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
  // again whenever the element starts playing (Appendix C).
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

  // A track that can't play is skipped (2_player.md §7.1)
  function onError(event: SyntheticEvent<HTMLAudioElement>) {
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
      play,
      toggle,
      setPlaying,
      next,
      previous,
      jump,
      remove,
      clear,
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
      play,
      toggle,
      setPlaying,
      next,
      previous,
      jump,
      remove,
      clear,
      setVolume,
      toggleMuted,
    ]
  )

  return (
    <PlayerContext.Provider value={player}>
      <TimeContext.Provider value={time}>
        {children}
        <audio
          ref={audio}
          preload="auto"
          className="hidden"
          onPlay={() => dispatch({ type: "setPlaying", playing: true })}
          onPause={() => {
            setWaiting(false)
            dispatch({ type: "setPlaying", playing: false })
          }}
          onPlaying={() => {
            setActions()
            setWaiting(false)
            dispatch({ type: "started" })
          }}
          onWaiting={() => setWaiting(true)}
          onCanPlay={() => setWaiting(false)}
          // A track that ends says it paused first: `ended` plays the next one
          onEnded={() => dispatch({ type: "ended" })}
          onError={onError}
          onTimeUpdate={onTime}
          onDurationChange={onTime}
          onSeeked={onTime}
          onEmptied={(event) => {
            setWaiting(false)
            onTime(event)
          }}
        />
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
