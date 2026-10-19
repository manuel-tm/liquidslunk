// The player's queue (2_player.md §5): the tracks it plays, in order, and the
// one that's current. Plain functions from a queue and an action to the next
// queue, with no React and no audio element, so they're tested on their own.
// `PlayerProvider` keeps the queue with `useReducer`, and makes the element
// follow it.

/** A queued track. Its key is its own, so a track queued twice is two. */
export type Entry = { key: number; trackId: number }

export type Queue = {
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
}

export type QueueAction =
  /** A new queue, from `start`, playing */
  | { type: "play"; trackIds: number[]; start: number }
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
  | { type: "remove"; key: number }
  /** Leaves only entries whose track is one of these: the others are gone */
  | { type: "keep"; trackIds: number[] }
  | { type: "clear" }

/** Previous goes back to the track's start once this much has played. */
export const RESTART_AFTER = 3

export const emptyQueue: Queue = {
  entries: [],
  current: -1,
  playing: false,
  restarts: 0,
  failures: 0,
  nextKey: 1,
}

/** The current entry, or null with nothing queued. */
export function currentEntry(queue: Queue): Entry | null {
  return queue.entries[queue.current] ?? null
}

export function queueReducer(queue: Queue, action: QueueAction): Queue {
  const queued = queue.current >= 0
  switch (action.type) {
    case "play": {
      if (action.trackIds.length === 0) return clear(queue)
      const entries = action.trackIds.map((trackId, index) => ({
        key: queue.nextKey + index,
        trackId,
      }))
      return {
        ...queue,
        entries,
        current: Math.min(Math.max(action.start, 0), entries.length - 1),
        playing: true,
        failures: 0,
        nextKey: queue.nextKey + entries.length,
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
      return action.time > RESTART_AFTER || queue.current === 0
        ? restart(queue, queue.playing)
        : { ...queue, current: queue.current - 1 }
    case "ended":
      return queued ? advance(queue, true) : queue
    case "failed": {
      if (!queued) return queue
      const failed = { ...queue, failures: queue.failures + 1 }
      // Every entry failed: stop, rather than going round
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
    case "remove":
      return without(queue, (entry) => entry.key === action.key)
    case "keep": {
      const kept = new Set(action.trackIds)
      return without(queue, (entry) => !kept.has(entry.trackId))
    }
    case "clear":
      return clear(queue)
  }
}

/** The next entry, playing or not; after the last, stopped at its start. */
function advance(queue: Queue, playing: boolean): Queue {
  return queue.current < queue.entries.length - 1
    ? { ...queue, current: queue.current + 1, playing }
    : restart(queue, false)
}

/** The current entry, from its start. */
function restart(queue: Queue, playing: boolean): Queue {
  return { ...queue, playing, restarts: queue.restarts + 1 }
}

function clear(queue: Queue): Queue {
  return { ...emptyQueue, restarts: queue.restarts, nextKey: queue.nextKey }
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
  const current = currentEntry(queue)
  if (current && !removed(current)) {
    return { ...queue, entries, current: entries.indexOf(current) }
  }
  // The first entry left after the current one
  const next = queue.entries
    .slice(queue.current + 1)
    .find((entry) => !removed(entry))
  return next
    ? { ...queue, entries, current: entries.indexOf(next) }
    : restart({ ...queue, entries, current: entries.length - 1 }, false)
}
