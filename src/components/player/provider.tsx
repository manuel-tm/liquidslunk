import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useEffectEvent,
  useMemo,
  useState,
  useSyncExternalStore,
} from "react"
import { toast } from "sonner"

import {
  commands,
  events,
  type PlayerEntry,
  type PlayerFailed,
  type PlayerState,
  type PlayerTrack,
  type Repeat,
  type RestoreArgs,
} from "@/bindings"
import { useCommand } from "@/lib/query"

// The browser remembers the volume, shuffle, repeat and the fade's length,
// for every library, as it does the theme, and the queue per library, since
// ids belong to one
const VOLUME = "slunk.player.volume"
const SHUFFLE = "slunk.player.shuffle"
const REPEAT = "slunk.player.repeat"
const FADE = "slunk.player.fade"
const QUEUE = "slunk.player.queue"
// ⌘↑ and ⌘↓ change the volume by this much
const VOLUME_STEP = 0.1

/** A queued track. Its key is its own, so a track queued twice is two. */
export type Entry = PlayerEntry

/** The player: what's queued, and what to do with it. */
export type Player = {
  entries: Entry[]
  current: Entry | null
  /** What the player shows of the current track, once it has loaded */
  track: PlayerTrack | null
  playing: boolean
  /** Whether it plays, but waits for its file */
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

/** How far into the current track it is. */
export type PlayerTime = {
  /** Seconds heard */
  time: number
}

/** The player before it has said anything: nothing queued. */
const NOTHING: PlayerState = {
  entries: [],
  current: null,
  playing: false,
  waiting: false,
  shuffle: false,
  repeat: "off",
  volume: 100,
  muted: false,
  fade: "10",
  remembered: null,
}

const PlayerContext = createContext<Player | null>(null)
const TimeContext = createContext<ReturnType<typeof timeStore> | null>(null)

/**
 * The time, as a store for `useSyncExternalStore`. Only what shows the time
 * subscribes, so the rest of the app doesn't render again on every
 * `PlayerTime`, four times a second.
 */
function timeStore() {
  let snapshot: PlayerTime = { time: 0 }
  const listeners = new Set<() => void>()
  return {
    get: () => snapshot,
    set(time: number) {
      if (time === snapshot.time) return
      snapshot = { time }
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

/**
 * What the browser remembers, for the player: the library's queue as the
 * player wrote it, which the player reads, and the settings, which are
 * checked here. Anything else gives the defaults.
 */
function remembered(library: string): RestoreArgs {
  let volume = 1
  let muted = false
  try {
    const saved = JSON.parse(recall(VOLUME) ?? "null") as {
      volume?: unknown
      muted?: unknown
    } | null
    if (
      typeof saved?.volume === "number" &&
      saved.volume >= 0 &&
      saved.volume <= 1
    ) {
      volume = saved.volume
      muted = saved.muted === true
    }
  } catch {
    // Remembering is a convenience
  }
  const repeat = recall(REPEAT)
  const fade = recall(FADE)
  return {
    queue: recall(`${QUEUE}:${library}`),
    volume: Math.round(volume * 100),
    muted,
    shuffle: recall(SHUFFLE) === "true",
    repeat: repeat === "all" || repeat === "one" ? repeat : "off",
    fade: fade === "off" || fade === "50" || fade === "200" ? fade : "10",
  }
}

/** Remembers the player as it says it is. */
function rememberState(library: string, state: PlayerState) {
  remember(`${QUEUE}:${library}`, state.remembered)
  remember(SHUFFLE, String(state.shuffle))
  remember(REPEAT, state.repeat)
  remember(FADE, state.fade)
  remember(
    VOLUME,
    JSON.stringify({ volume: state.volume / 100, muted: state.muted })
  )
}

/** Repeat's next setting: off, all, one, then off again. */
function nextRepeat(repeat: Repeat): Repeat {
  return repeat === "off" ? "all" : repeat === "all" ? "one" : "off"
}

/** The first letter of a sentence, capitalized. */
function sentence(text: string) {
  return text.charAt(0).toUpperCase() + text.slice(1)
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
 * The app's one player (frontend.md, "The player"), as the page sees it. It
 * plays in Rust: the provider shows the player's state, from its events, and
 * calls its commands. It's inside `AppLayout`'s keyed `SidebarProvider`, so
 * it mounts as the library opens, and hands the player what the browser
 * remembers of it, then remembers what the player says. The sidebar only
 * shows its controls, so a hidden sidebar doesn't stop the music.
 */
export function PlayerProvider({
  library,
  children,
}: {
  /** The open library's path, which its remembered queue is kept under */
  library: string
  children: ReactNode
}) {
  const [state, setState] = useState<PlayerState>(NOTHING)
  const [time] = useState(timeStore)

  // What the player shows of each queued track. Saves fetch it again, and
  // the last data stays while the queue's ids change.
  const { entries } = state
  const ids = useMemo(
    () => [...new Set(entries.map((entry) => entry.trackId))],
    [entries]
  )
  const { data } = useCommand(
    "playerTracks",
    { ids },
    { enabled: ids.length > 0, keepPrevious: true }
  )
  const tracks = useMemo(
    () => new Map((data ?? []).map((track) => [track.id, track])),
    [data]
  )
  const current =
    state.current === null ? null : (entries[state.current] ?? null)
  const track = (current && tracks.get(current.trackId)) ?? null

  // A track that can't play: the player has skipped it
  const onFailed = useEffectEvent(({ trackId, path, reason }: PlayerFailed) => {
    toast.error(`Can't play ${tracks.get(trackId)?.title ?? "the track"}`, {
      description: (
        <>
          {path && <span className="break-all">{path}</span>} {sentence(reason)}
          .
        </>
      ),
    })
  })

  // The player's events first, so nothing it says is missed, then what the
  // browser remembers, then the player as it is: unless it has said so
  // already, since what it says is newer
  useEffect(() => {
    let live = true
    let said = false
    const listening = [
      events.playerState.listen(({ payload }) => {
        said = true
        setState(payload)
        rememberState(library, payload)
      }),
      events.playerTime.listen(({ payload }) => time.set(payload.ms / 1000)),
      events.playerFailed.listen(({ payload }) => onFailed(payload)),
    ]
    void Promise.all(listening).then(async () => {
      await commands.playerRestore(remembered(library))
      const result = await commands.playerState()
      if (live && !said && result.status === "ok") setState(result.data)
    })
    return () => {
      live = false
      for (const unlisten of listening) void unlisten.then((stop) => stop())
    }
  }, [library, time])

  const play = useCallback(
    (trackIds: number[], start?: number) =>
      void commands.playerPlay({ trackIds, start }),
    []
  )
  const playNext = useCallback(
    (trackIds: number[]) => void commands.playerPlayNext({ trackIds }),
    []
  )
  const addToQueue = useCallback(
    (trackIds: number[]) => void commands.playerAddToQueue({ trackIds }),
    []
  )
  const toggle = useCallback(() => void commands.playerToggle(), [])
  const setPlaying = useCallback(
    (playing: boolean) => void commands.playerSetPlaying({ playing }),
    []
  )
  const next = useCallback(() => void commands.playerNext(), [])
  const previous = useCallback(() => void commands.playerPrevious(), [])
  const jump = useCallback(
    (entry: Entry) => void commands.playerJump({ key: entry.key }),
    []
  )
  const move = useCallback(
    (entry: Entry, by: number) =>
      void commands.playerMove({ key: entry.key, by }),
    []
  )
  const remove = useCallback(
    (entry: Entry) => void commands.playerRemove({ key: entry.key }),
    []
  )
  const clear = useCallback(() => void commands.playerClear(), [])
  const toggleShuffle = useCallback(
    () => void commands.playerSetShuffle({ shuffle: !state.shuffle }),
    [state.shuffle]
  )
  const cycleRepeat = useCallback(
    () => void commands.playerSetRepeat({ repeat: nextRepeat(state.repeat) }),
    [state.repeat]
  )
  // In hundredths, as the player keeps it
  const setVolume = useCallback(
    (volume: number) =>
      void commands.playerSetVolume({
        volume: Math.round(Math.min(1, Math.max(0, volume)) * 100),
        muted: false,
      }),
    []
  )
  const toggleMuted = useCallback(
    () =>
      void commands.playerSetVolume({
        volume: state.volume,
        muted: !state.muted,
      }),
    [state.volume, state.muted]
  )
  const volume = state.volume / 100

  // Space, ⌘→, ⌘←, ⌘↑ and ⌘↓, as in Music. The Controls menu shows the ⌘
  // ones, but only gets a key the page leaves: these take it first, before
  // WebKit scrolls to the end of a page for ⌘↓.
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

  // One sound at a time: the file page's preview starting pauses the
  // player, and the player starting pauses it. Media events don't bubble,
  // so this listens in the capture phase.
  useEffect(() => {
    function onPlay(event: Event) {
      if (event.target instanceof HTMLMediaElement) {
        void commands.playerSetPlaying({ playing: false })
      }
    }
    document.addEventListener("play", onPlay, true)
    return () => document.removeEventListener("play", onPlay, true)
  }, [])
  useEffect(() => {
    if (!state.playing) return
    for (const element of document.querySelectorAll<HTMLMediaElement>(
      "audio, video"
    )) {
      element.pause()
    }
  }, [state.playing])

  const player = useMemo<Player>(
    () => ({
      entries,
      current,
      track,
      playing: state.playing,
      waiting: state.waiting,
      volume,
      muted: state.muted,
      shuffle: state.shuffle,
      repeat: state.repeat,
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
      entries,
      current,
      track,
      state.playing,
      state.waiting,
      volume,
      state.muted,
      state.shuffle,
      state.repeat,
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
      <TimeContext.Provider value={time}>{children}</TimeContext.Provider>
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
