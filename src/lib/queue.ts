// The player's queue (2_player.md §5): the tracks it plays, in order, and the
// one that's current. Plain functions from a queue and an action to the next
// queue, with no React and no audio element, so they're tested on their own.
// `PlayerProvider` keeps the queue with `useReducer`, and makes the element
// follow it. Shuffling takes a seed from the action, so the reducer stays pure.

/** A queued track. Its key is its own, so a track queued twice is two. */
export type Entry = { key: number; trackId: number }

/** Off, all (the first entry follows the last), or one (it plays again). */
export type Repeat = "off" | "all" | "one"

export type Queue = {
  /** In the order they play */
  entries: Entry[]
  /** The current entry's index, or -1 with nothing queued */
  current: number
  /** Whether the current entry plays, rather than waits paused */
  playing: boolean
  /** Goes up whenever the current entry goes back to its start */
  restarts: number
  /** Entries that failed in a row, since one last played */
  failures: number
  /** The key the next entry gets */
  nextKey: number
  shuffle: boolean
  /** While shuffled, the entries in their order before shuffling */
  unshuffled: Entry[] | null
  repeat: Repeat
}

export type QueueAction =
  /**
   * A new queue, playing from the track at `start`. Shuffled, that track
   * plays first, or a random one without `start`, and the rest after it.
   */
  | { type: "play"; trackIds: number[]; start?: number; seed: number }
  /** Right after the current entry; with nothing queued, a queue, paused */
  | { type: "playNext"; trackIds: number[] }
  /** At the end; with nothing queued, a queue, paused */
  | { type: "add"; trackIds: number[] }
  | { type: "toggle" }
  /** Plays or pauses: the user, Now Playing, or the element saying so */
  | { type: "setPlaying"; playing: boolean }
  | { type: "next" }
  /** `time` is how far into the current track it is, in seconds */
  | { type: "previous"; time: number }
  /** The current track played to its end */
  | { type: "ended" }
  /** The current track can't play: missing, or a format that can't */
  | { type: "failed" }
  /** The current track started playing */
  | { type: "started" }
  | { type: "jump"; key: number }
  /** Moves an entry `by` places in the order they play */
  | { type: "move"; key: number; by: number }
  | { type: "remove"; key: number }
  /** Leaves only entries whose track is one of these: the others are gone */
  | { type: "keep"; trackIds: number[] }
  | { type: "clear" }
  | { type: "setShuffle"; shuffle: boolean; seed: number }
  | { type: "setRepeat"; repeat: Repeat }

/** Previous goes back to the track's start once this much has played. */
export const RESTART_AFTER = 3

export const emptyQueue: Queue = {
  entries: [],
  current: -1,
  playing: false,
  restarts: 0,
  failures: 0,
  nextKey: 1,
  shuffle: false,
  unshuffled: null,
  repeat: "off",
}

/** The current entry, or null with nothing queued. */
export function currentEntry(queue: Queue): Entry | null {
  return queue.entries[queue.current] ?? null
}

/** Repeat's next setting: off, all, one, then off again. */
export function nextRepeat(repeat: Repeat): Repeat {
  return repeat === "off" ? "all" : repeat === "all" ? "one" : "off"
}

export function queueReducer(queue: Queue, action: QueueAction): Queue {
  const queued = queue.current >= 0
  switch (action.type) {
    case "play":
      return play(queue, action.trackIds, action.start, action.seed)
    case "playNext":
    case "add": {
      if (action.trackIds.length === 0) return queue
      const [added, nextKey] = entriesOf(queue, action.trackIds)
      if (!queued) {
        return {
          ...queue,
          entries: added,
          current: 0,
          playing: false,
          failures: 0,
          unshuffled: queue.shuffle ? added : null,
          nextKey,
        }
      }
      const at =
        action.type === "add" ? queue.entries.length : queue.current + 1
      return {
        ...queue,
        entries: insert(queue.entries, at, added),
        unshuffled:
          queue.unshuffled &&
          insert(
            queue.unshuffled,
            action.type === "add"
              ? queue.unshuffled.length
              : queue.unshuffled.indexOf(queue.entries[queue.current]) + 1,
            added
          ),
        nextKey,
      }
    }
    case "toggle":
      return queued ? { ...queue, playing: !queue.playing } : queue
    case "setPlaying":
      return queued && queue.playing !== action.playing
        ? { ...queue, playing: action.playing }
        : queue
    case "next":
      return queued ? advance(queue, queue.playing) : queue
    case "previous":
      if (!queued) return queue
      if (action.time > RESTART_AFTER) return restart(queue, queue.playing)
      if (queue.current > 0) return { ...queue, current: queue.current - 1 }
      return queue.repeat === "all" && queue.entries.length > 1
        ? { ...queue, current: queue.entries.length - 1 }
        : restart(queue, queue.playing)
    case "ended":
      if (!queued) return queue
      return queue.repeat === "one"
        ? restart(queue, true)
        : advance(queue, true)
    case "failed": {
      if (!queued) return queue
      const failed = { ...queue, failures: queue.failures + 1 }
      // Every entry failed: stop, rather than going round. Repeat one doesn't
      // try the same track again.
      return failed.failures >= queue.entries.length
        ? { ...failed, playing: false }
        : advance(failed, queue.playing)
    }
    case "started":
      return queue.failures === 0 ? queue : { ...queue, failures: 0 }
    case "jump": {
      const index = queue.entries.findIndex(({ key }) => key === action.key)
      if (index < 0) return queue
      return index === queue.current
        ? restart(queue, true)
        : { ...queue, current: index, playing: true }
    }
    case "move": {
      const from = queue.entries.findIndex(({ key }) => key === action.key)
      const to = from + action.by
      if (from < 0 || to < 0 || to >= queue.entries.length || from === to) {
        return queue
      }
      const current = currentEntry(queue)
      const entries = [...queue.entries]
      const [entry] = entries.splice(from, 1)
      entries.splice(to, 0, entry)
      return {
        ...queue,
        entries,
        current: current ? entries.indexOf(current) : -1,
      }
    }
    case "remove":
      return without(queue, (entry) => entry.key === action.key)
    case "keep": {
      const kept = new Set(action.trackIds)
      return without(queue, (entry) => !kept.has(entry.trackId))
    }
    case "clear":
      return clear(queue)
    case "setShuffle":
      if (action.shuffle === queue.shuffle) return queue
      return action.shuffle ? shuffleRest(queue, action.seed) : unshuffle(queue)
    case "setRepeat":
      return queue.repeat === action.repeat
        ? queue
        : { ...queue, repeat: action.repeat }
  }
}

function entriesOf(queue: Queue, trackIds: number[]): [Entry[], number] {
  const entries = trackIds.map((trackId, index) => ({
    key: queue.nextKey + index,
    trackId,
  }))
  return [entries, queue.nextKey + entries.length]
}

function insert(entries: Entry[], at: number, added: Entry[]): Entry[] {
  return [...entries.slice(0, at), ...added, ...entries.slice(at)]
}

function play(
  queue: Queue,
  trackIds: number[],
  start: number | undefined,
  seed: number
): Queue {
  if (trackIds.length === 0) return clear(queue)
  const [entries, nextKey] = entriesOf(queue, trackIds)
  const started = { ...queue, playing: true, failures: 0, nextKey }
  const clamp = (index: number) =>
    Math.min(Math.max(index, 0), entries.length - 1)
  if (!queue.shuffle) {
    return {
      ...started,
      entries,
      current: clamp(start ?? 0),
      unshuffled: null,
    }
  }
  const random = generator(seed)
  const first = entries[clamp(start ?? Math.floor(random() * entries.length))]
  const rest = shuffled(
    entries.filter((entry) => entry !== first),
    random
  )
  return {
    ...started,
    entries: [first, ...rest],
    current: 0,
    unshuffled: entries,
  }
}

/** The next entry, playing or not; after the last, by repeat. */
function advance(queue: Queue, playing: boolean): Queue {
  if (queue.current < queue.entries.length - 1) {
    return { ...queue, current: queue.current + 1, playing }
  }
  if (queue.repeat === "all") {
    return queue.entries.length > 1
      ? { ...queue, current: 0, playing }
      : restart(queue, playing)
  }
  // Repeat one only plays a track again when it ends
  return restart(queue, false)
}

/** The current entry, from its start. */
function restart(queue: Queue, playing: boolean): Queue {
  return { ...queue, playing, restarts: queue.restarts + 1 }
}

function clear(queue: Queue): Queue {
  return {
    ...emptyQueue,
    restarts: queue.restarts,
    nextKey: queue.nextKey,
    shuffle: queue.shuffle,
    unshuffled: queue.shuffle ? [] : null,
    repeat: queue.repeat,
  }
}

/** Shuffle on: the entries after the current one, in a random order. */
function shuffleRest(queue: Queue, seed: number): Queue {
  const played = queue.entries.slice(0, queue.current + 1)
  const rest = queue.entries.slice(queue.current + 1)
  return {
    ...queue,
    shuffle: true,
    unshuffled: queue.entries,
    entries: [...played, ...shuffled(rest, generator(seed))],
  }
}

/** Shuffle off: the entries after the current one, in their order before. */
function unshuffle(queue: Queue): Queue {
  const order = new Map(queue.unshuffled?.map((entry, index) => [entry, index]))
  const played = queue.entries.slice(0, queue.current + 1)
  const rest = queue.entries
    .slice(queue.current + 1)
    .sort((a, b) => (order.get(a) ?? 0) - (order.get(b) ?? 0))
  return {
    ...queue,
    shuffle: false,
    unshuffled: null,
    entries: [...played, ...rest],
  }
}

/**
 * The queue without the entries `removed` picks. When the current one goes,
 * the next one left is current, playing as it was; with none after it, the
 * last one left is, stopped at its start.
 */
function without(queue: Queue, removed: (entry: Entry) => boolean): Queue {
  if (!queue.entries.some(removed)) return queue
  const entries = queue.entries.filter((entry) => !removed(entry))
  if (entries.length === 0) return clear(queue)
  const unshuffled =
    queue.unshuffled && queue.unshuffled.filter((entry) => !removed(entry))
  const current = currentEntry(queue)
  if (current && !removed(current)) {
    return { ...queue, entries, unshuffled, current: entries.indexOf(current) }
  }
  // The first entry left after the current one
  const next = queue.entries
    .slice(queue.current + 1)
    .find((entry) => !removed(entry))
  return next
    ? { ...queue, entries, unshuffled, current: entries.indexOf(next) }
    : restart(
        { ...queue, entries, unshuffled, current: entries.length - 1 },
        false
      )
}

/** A random order, Fisher–Yates, with the generator's numbers. */
function shuffled<T>(items: T[], random: () => number): T[] {
  const order = [...items]
  for (let i = order.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1))
    ;[order[i], order[j]] = [order[j], order[i]]
  }
  return order
}

/** Numbers from 0 to 1, the same ones for the same seed (mulberry32). */
function generator(seed: number): () => number {
  let state = seed >>> 0
  return () => {
    state = (state + 0x6d2b79f5) >>> 0
    let t = state
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

/**
 * What's remembered of a library's queue (2_player.md §5.7): its tracks in
 * the order they play, the current one, and while shuffled, their order
 * before, as indexes into `trackIds`. It comes back paused at the current
 * track's start, since there's no seeking (Appendix B).
 */
export type SavedQueue = {
  trackIds: number[]
  current: number
  unshuffled: number[] | null
}

/** The queue to remember, or null when nothing's queued. */
export function savedQueue(queue: Queue): SavedQueue | null {
  if (queue.current < 0) return null
  return {
    trackIds: queue.entries.map((entry) => entry.trackId),
    current: queue.current,
    unshuffled:
      queue.unshuffled &&
      queue.unshuffled.map((entry) => queue.entries.indexOf(entry)),
  }
}

/**
 * A remembered queue, paused, with the shuffle and repeat the browser
 * remembers. Anything that isn't a queue, like what an older version
 * remembered, gives an empty one.
 */
export function restoredQueue(
  saved: unknown,
  modes: { shuffle: boolean; repeat: Repeat }
): Queue {
  const empty = {
    ...emptyQueue,
    ...modes,
    unshuffled: modes.shuffle ? [] : null,
  }
  if (!isSavedQueue(saved)) return empty
  const entries = saved.trackIds.map((trackId, index) => ({
    key: index + 1,
    trackId,
  }))
  const unshuffled = !modes.shuffle
    ? null
    : saved.unshuffled
      ? saved.unshuffled.map((index) => entries[index])
      : entries
  return {
    ...empty,
    entries,
    current: saved.current,
    unshuffled,
    nextKey: entries.length + 1,
  }
}

function isSavedQueue(saved: unknown): saved is SavedQueue {
  if (typeof saved !== "object" || saved === null) return false
  const { trackIds, current, unshuffled } = saved as Partial<SavedQueue>
  if (!Array.isArray(trackIds) || trackIds.length === 0) return false
  if (!trackIds.every((id) => Number.isSafeInteger(id) && id > 0)) return false
  const index = (value: unknown): value is number =>
    Number.isInteger(value) && (value as number) >= 0
  if (!index(current) || current >= trackIds.length) return false
  if (unshuffled === null || unshuffled === undefined) return true
  // A shuffled order is every index once
  return (
    Array.isArray(unshuffled) &&
    unshuffled.length === trackIds.length &&
    new Set(unshuffled).size === trackIds.length &&
    unshuffled.every((value) => index(value) && value < trackIds.length)
  )
}
