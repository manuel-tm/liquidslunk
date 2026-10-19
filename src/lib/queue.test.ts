import { describe, expect, it } from "vitest"

import {
  currentEntry,
  emptyQueue,
  nextRepeat,
  type Queue,
  type QueueAction,
  queueReducer,
  type Repeat,
  restoredQueue,
  savedQueue,
  upNext,
} from "@/lib/queue"

/** The queue after these actions, from an empty one. */
function after(...actions: QueueAction[]): Queue {
  return actions.reduce(queueReducer, emptyQueue)
}

/** A queue of tracks 10, 20 and 30, playing from `start`. */
function playing(start = 0): QueueAction {
  return { type: "play", trackIds: [10, 20, 30], start, seed: 1 }
}

/** The current entry's track, or null. */
function track(queue: Queue) {
  return currentEntry(queue)?.trackId ?? null
}

describe("starting a queue", () => {
  it("plays the tracks from the one asked for", () => {
    const queue = after(playing(1))
    expect(queue.entries.map((entry) => entry.trackId)).toEqual([10, 20, 30])
    expect(track(queue)).toBe(20)
    expect(queue.playing).toBe(true)
  })

  it("gives a track queued twice two entries", () => {
    const queue = after({ type: "play", trackIds: [10, 10], start: 0, seed: 1 })
    const [first, second] = queue.entries
    expect(first.trackId).toBe(second.trackId)
    expect(first.key).not.toBe(second.key)
  })

  it("replaces the queue, with new keys", () => {
    const first = after(playing())
    const second = queueReducer(first, playing())
    const keys = new Set(first.entries.map((entry) => entry.key))
    expect(second.entries.some((entry) => keys.has(entry.key))).toBe(false)
  })

  it("keeps the start within the queue", () => {
    expect(track(after(playing(7)))).toBe(30)
    expect(track(after(playing(-1)))).toBe(10)
  })

  it("empties the queue with no tracks", () => {
    const queue = after(playing(), {
      type: "play",
      trackIds: [],
      start: 0,
      seed: 1,
    })
    expect(currentEntry(queue)).toBeNull()
    expect(queue.playing).toBe(false)
  })
})

describe("with nothing queued", () => {
  it("does nothing", () => {
    for (const action of [
      { type: "toggle" },
      { type: "setPlaying", playing: true },
      { type: "next" },
      { type: "previous", time: 0 },
      { type: "ended" },
      { type: "failed" },
    ] satisfies QueueAction[]) {
      expect(queueReducer(emptyQueue, action)).toBe(emptyQueue)
    }
  })
})

describe("play and pause", () => {
  it("toggles", () => {
    const paused = after(playing(), { type: "toggle" })
    expect(paused.playing).toBe(false)
    expect(queueReducer(paused, { type: "toggle" }).playing).toBe(true)
  })

  it("follows what the element says, without a new queue when it agrees", () => {
    const queue = after(playing())
    expect(queueReducer(queue, { type: "setPlaying", playing: true })).toBe(
      queue
    )
    expect(
      queueReducer(queue, { type: "setPlaying", playing: false }).playing
    ).toBe(false)
  })
})

describe("next", () => {
  it("goes to the next track, playing or paused as it was", () => {
    expect(after(playing(), { type: "next" })).toMatchObject({
      current: 1,
      playing: true,
    })
    expect(
      after(playing(), { type: "toggle" }, { type: "next" })
    ).toMatchObject({ current: 1, playing: false })
  })

  it("stops after the last track, at its start", () => {
    const last = after(playing(2))
    const stopped = queueReducer(last, { type: "next" })
    expect(track(stopped)).toBe(30)
    expect(stopped.playing).toBe(false)
    expect(stopped.restarts).toBe(last.restarts + 1)
  })
})

describe("previous", () => {
  it("goes back to the previous track in the first seconds", () => {
    const queue = after(playing(2), { type: "previous", time: 3 })
    expect(track(queue)).toBe(20)
    expect(queue.playing).toBe(true)
  })

  it("goes back to the track's start after them", () => {
    const queue = after(playing(2))
    const restarted = queueReducer(queue, { type: "previous", time: 3.5 })
    expect(track(restarted)).toBe(30)
    expect(restarted.restarts).toBe(queue.restarts + 1)
  })

  it("goes back to the first track's start", () => {
    const queue = after(playing())
    const restarted = queueReducer(queue, { type: "previous", time: 0 })
    expect(track(restarted)).toBe(10)
    expect(restarted.restarts).toBe(queue.restarts + 1)
  })

  it("stays paused when paused", () => {
    const queue = after(
      playing(1),
      { type: "toggle" },
      { type: "previous", time: 0 }
    )
    expect(queue).toMatchObject({ current: 0, playing: false })
  })
})

describe("a track that ends", () => {
  it("goes on to the next one, playing", () => {
    // The element says it paused before it says it ended
    const queue = after(
      playing(),
      { type: "setPlaying", playing: false },
      { type: "ended" }
    )
    expect(queue).toMatchObject({ current: 1, playing: true })
  })

  it("stops the queue when it's the last", () => {
    const queue = after(playing(2), { type: "ended" })
    expect(queue).toMatchObject({ current: 2, playing: false })
  })
})

describe("a track that can't play", () => {
  it("is skipped", () => {
    expect(after(playing(), { type: "failed" })).toMatchObject({
      current: 1,
      playing: true,
      failures: 1,
    })
  })

  it("stops the queue once every track has failed in a row", () => {
    const queue = after(
      playing(),
      { type: "failed" },
      { type: "failed" },
      { type: "failed" }
    )
    expect(queue.playing).toBe(false)
    expect(queue.failures).toBe(3)
  })

  it("counts again once a track plays", () => {
    const queue = after(
      playing(),
      { type: "failed" },
      { type: "started" },
      { type: "failed" }
    )
    expect(queue).toMatchObject({ current: 2, playing: true, failures: 1 })
  })
})

describe("jumping to an entry", () => {
  it("plays it", () => {
    const queue = after(playing(), { type: "toggle" })
    const third = queue.entries[2]
    expect(queueReducer(queue, { type: "jump", key: third.key })).toMatchObject(
      { current: 2, playing: true }
    )
  })

  it("plays the current one from its start", () => {
    const queue = after(playing(1))
    const jumped = queueReducer(queue, {
      type: "jump",
      key: queue.entries[1].key,
    })
    expect(jumped.current).toBe(1)
    expect(jumped.restarts).toBe(queue.restarts + 1)
  })

  it("ignores an entry that isn't queued", () => {
    const queue = after(playing())
    expect(queueReducer(queue, { type: "jump", key: -1 })).toBe(queue)
  })
})

describe("removing entries", () => {
  it("keeps the current one current", () => {
    const queue = after(playing(1))
    const removed = queueReducer(queue, {
      type: "remove",
      key: queue.entries[0].key,
    })
    expect(track(removed)).toBe(20)
    expect(removed.current).toBe(0)
  })

  it("goes on to the next one when the current one goes", () => {
    const queue = after(playing(1), { type: "toggle" })
    const removed = queueReducer(queue, {
      type: "remove",
      key: queue.entries[1].key,
    })
    expect(track(removed)).toBe(30)
    expect(removed.playing).toBe(false)
  })

  it("stops at the last one left when the last one goes", () => {
    const queue = after(playing(2))
    const removed = queueReducer(queue, {
      type: "remove",
      key: queue.entries[2].key,
    })
    expect(track(removed)).toBe(20)
    expect(removed.playing).toBe(false)
  })

  it("removes one entry of a track queued twice", () => {
    const queue = after({
      type: "play",
      trackIds: [10, 20, 10],
      start: 0,
      seed: 1,
    })
    const removed = queueReducer(queue, {
      type: "remove",
      key: queue.entries[2].key,
    })
    expect(removed.entries.map((entry) => entry.trackId)).toEqual([10, 20])
  })

  it("empties the queue when the only one goes", () => {
    const queue = after({ type: "play", trackIds: [10], start: 0, seed: 1 })
    const removed = queueReducer(queue, {
      type: "remove",
      key: queue.entries[0].key,
    })
    expect(currentEntry(removed)).toBeNull()
    expect(removed.playing).toBe(false)
  })
})

describe("deleted tracks", () => {
  it("leave the queue, every entry of them", () => {
    const queue = after({
      type: "play",
      trackIds: [10, 20, 10, 30],
      start: 3,
      seed: 1,
    })
    const kept = queueReducer(queue, { type: "keep", trackIds: [20, 30] })
    expect(kept.entries.map((entry) => entry.trackId)).toEqual([20, 30])
    expect(track(kept)).toBe(30)
  })

  it("move the queue on when the current one is deleted", () => {
    const queue = after(playing(1))
    const kept = queueReducer(queue, { type: "keep", trackIds: [10, 30] })
    expect(kept).toMatchObject({ current: 1, playing: true })
    expect(track(kept)).toBe(30)
  })

  it("change nothing when every track is still there", () => {
    const queue = after(playing())
    expect(
      queueReducer(queue, { type: "keep", trackIds: [10, 20, 30, 40] })
    ).toBe(queue)
  })
})

describe("clearing", () => {
  it("empties the queue and stops", () => {
    const queue = after(playing(), { type: "clear" })
    expect(queue.entries).toEqual([])
    expect(currentEntry(queue)).toBeNull()
    expect(queue.playing).toBe(false)
  })

  it("never hands out a key again", () => {
    const first = after(playing())
    const second = after(playing(), { type: "clear" }, playing())
    expect(second.entries[0].key).toBeGreaterThan(first.entries[2].key)
  })
})

/** The queue's tracks, in the order they play. */
function tracks(queue: Queue) {
  return queue.entries.map((entry) => entry.trackId)
}

/** A queue of tracks 10 to 60, with shuffle and repeat set, from `start`. */
function modes(
  { shuffle = false, repeat = "off" }: { shuffle?: boolean; repeat?: Repeat },
  start?: number
): Queue {
  return after(
    { type: "setShuffle", shuffle, seed: 7 },
    { type: "setRepeat", repeat },
    { type: "play", trackIds: [10, 20, 30, 40, 50, 60], start, seed: 7 }
  )
}

describe("play next and add to queue", () => {
  it("puts tracks right after the current one, or at the end", () => {
    const queue = after(playing(1))
    expect(
      tracks(queueReducer(queue, { type: "playNext", trackIds: [1, 2] }))
    ).toEqual([10, 20, 1, 2, 30])
    expect(
      tracks(queueReducer(queue, { type: "add", trackIds: [1, 2] }))
    ).toEqual([10, 20, 30, 1, 2])
  })

  it("leaves the current track playing", () => {
    const queue = after(playing(1), { type: "playNext", trackIds: [1] })
    expect(queue).toMatchObject({ current: 1, playing: true })
  })

  it("start a queue, paused at its first track, when nothing's queued", () => {
    for (const type of ["playNext", "add"] as const) {
      const queue = after({ type, trackIds: [1, 2] })
      expect(tracks(queue)).toEqual([1, 2])
      expect(queue).toMatchObject({ current: 0, playing: false })
    }
  })

  it("give each added track a new key", () => {
    const queue = after(playing(), { type: "add", trackIds: [10] })
    expect(new Set(queue.entries.map((entry) => entry.key)).size).toBe(4)
  })
})

describe("moving an entry", () => {
  it("moves it in the order they play, keeping the current one current", () => {
    const queue = after(playing(1))
    const moved = queueReducer(queue, {
      type: "move",
      key: queue.entries[2].key,
      by: -2,
    })
    expect(tracks(moved)).toEqual([30, 10, 20])
    expect(track(moved)).toBe(20)
  })

  it("moves the current one with it", () => {
    const queue = after(playing(1))
    const moved = queueReducer(queue, {
      type: "move",
      key: queue.entries[1].key,
      by: 1,
    })
    expect(tracks(moved)).toEqual([10, 30, 20])
    expect(moved.current).toBe(2)
  })

  it("moves it several places at once, past the current one", () => {
    const queue = after({
      type: "play",
      trackIds: [10, 20, 30, 40, 50],
      start: 2,
      seed: 1,
    })
    const down = queueReducer(queue, {
      type: "move",
      key: queue.entries[0].key,
      by: 3,
    })
    expect(tracks(down)).toEqual([20, 30, 40, 10, 50])
    expect(track(down)).toBe(30)
    const up = queueReducer(queue, {
      type: "move",
      key: queue.entries[4].key,
      by: -4,
    })
    expect(tracks(up)).toEqual([50, 10, 20, 30, 40])
    expect(track(up)).toBe(30)
  })

  it("can't move past either end", () => {
    const queue = after(playing())
    const first = { type: "move", key: queue.entries[0].key, by: -1 } as const
    expect(queueReducer(queue, first)).toBe(queue)
  })
})

describe("shuffle", () => {
  it("plays the entries after the current one in another order", () => {
    const queue = after(
      { type: "play", trackIds: [10, 20, 30, 40, 50, 60], start: 1, seed: 7 },
      { type: "setShuffle", shuffle: true, seed: 7 }
    )
    expect(tracks(queue).slice(0, 2)).toEqual([10, 20])
    expect(tracks(queue).slice(2).sort()).toEqual([30, 40, 50, 60])
    expect(tracks(queue).slice(2)).not.toEqual([30, 40, 50, 60])
    expect(track(queue)).toBe(20)
  })

  it("is the same for the same seed", () => {
    expect(tracks(modes({ shuffle: true }))).toEqual(
      tracks(modes({ shuffle: true }))
    )
  })

  it("off, plays the rest in their order again, from the current one", () => {
    const shuffled = modes({ shuffle: true })
    const current = track(shuffled)
    const off = queueReducer(shuffled, {
      type: "setShuffle",
      shuffle: false,
      seed: 1,
    })
    expect(track(off)).toBe(current)
    const rest = tracks(off).slice(off.current + 1)
    expect(rest).toEqual([...rest].sort())
    expect(off.unshuffled).toBeNull()
  })

  it("starts a new queue at the track that was clicked", () => {
    const queue = modes({ shuffle: true }, 3)
    expect(track(queue)).toBe(40)
    expect(queue.current).toBe(0)
    expect(tracks(queue).sort()).toEqual([10, 20, 30, 40, 50, 60])
  })

  it("starts a new queue at a random track without one", () => {
    const firsts = new Set(
      [1, 2, 3, 4, 5, 6, 7, 8].map((seed) =>
        track(
          after(
            { type: "setShuffle", shuffle: true, seed },
            { type: "play", trackIds: [10, 20, 30, 40, 50, 60], seed }
          )
        )
      )
    )
    expect(firsts.size).toBeGreaterThan(1)
  })

  it("keeps play next right after the current one, and add at the end", () => {
    const queue = after(
      { type: "play", trackIds: [10, 20, 30, 40], start: 0, seed: 7 },
      { type: "setShuffle", shuffle: true, seed: 7 },
      { type: "playNext", trackIds: [1] },
      { type: "add", trackIds: [2] }
    )
    expect(tracks(queue)[1]).toBe(1)
    expect(tracks(queue).at(-1)).toBe(2)
    // Unshuffled, they keep those places in the order before
    const off = queueReducer(queue, {
      type: "setShuffle",
      shuffle: false,
      seed: 1,
    })
    expect(tracks(off)).toEqual([10, 1, 20, 30, 40, 2])
  })

  it("forgets removed entries in the order before", () => {
    const queue = after(
      { type: "play", trackIds: [10, 20, 30], start: 0, seed: 7 },
      { type: "setShuffle", shuffle: true, seed: 7 }
    )
    const twenty = queue.entries.find((entry) => entry.trackId === 20)!
    const removed = queueReducer(queue, { type: "remove", key: twenty.key })
    expect(removed.unshuffled?.map((entry) => entry.trackId)).toEqual([10, 30])
  })
})

describe("repeat", () => {
  it("goes from off, to all, to one, and back to off", () => {
    expect(nextRepeat("off")).toBe("all")
    expect(nextRepeat("all")).toBe("one")
    expect(nextRepeat("one")).toBe("off")
  })

  it("all: the first entry follows the last", () => {
    const last = modes({ repeat: "all" }, 5)
    expect(queueReducer(last, { type: "next" })).toMatchObject({
      current: 0,
      playing: true,
    })
    expect(
      queueReducer(queueReducer(last, { type: "setPlaying", playing: false }), {
        type: "ended",
      })
    ).toMatchObject({ current: 0, playing: true })
  })

  it("all: previous on the first entry goes to the last", () => {
    const first = modes({ repeat: "all" }, 0)
    expect(queueReducer(first, { type: "previous", time: 0 }).current).toBe(5)
  })

  it("all: a queue of one track starts it again", () => {
    const one = after(
      { type: "setRepeat", repeat: "all" },
      { type: "play", trackIds: [10], seed: 1 }
    )
    const again = queueReducer(one, { type: "ended" })
    expect(again).toMatchObject({ current: 0, playing: true })
    expect(again.restarts).toBe(one.restarts + 1)
  })

  it("one: a track that ends plays again", () => {
    const queue = modes({ repeat: "one" }, 2)
    const again = queueReducer(queue, { type: "ended" })
    expect(again).toMatchObject({ current: 2, playing: true })
    expect(again.restarts).toBe(queue.restarts + 1)
  })

  it("one: next and previous still move, and the queue still stops", () => {
    const queue = modes({ repeat: "one" }, 2)
    expect(queueReducer(queue, { type: "next" }).current).toBe(3)
    expect(queueReducer(queue, { type: "previous", time: 0 }).current).toBe(1)
    const last = modes({ repeat: "one" }, 5)
    expect(queueReducer(last, { type: "next" })).toMatchObject({
      current: 5,
      playing: false,
    })
  })

  it("one: a track that can't play is skipped, not tried again", () => {
    const queue = modes({ repeat: "one" }, 2)
    expect(queueReducer(queue, { type: "failed" }).current).toBe(3)
  })

  it("all: stops once every entry has failed, rather than going round", () => {
    let queue = modes({ repeat: "all" }, 4)
    for (let i = 0; i < 6; i++) queue = queueReducer(queue, { type: "failed" })
    expect(queue.playing).toBe(false)
    expect(queue.failures).toBe(6)
  })

  it("survive clearing the queue", () => {
    const queue = queueReducer(modes({ shuffle: true, repeat: "one" }), {
      type: "clear",
    })
    expect(queue).toMatchObject({ shuffle: true, repeat: "one" })
  })
})

/** The track up next, or null. */
function next(queue: Queue) {
  return upNext(queue)?.trackId ?? null
}

describe("what plays next", () => {
  it("is the next entry", () => {
    expect(next(after(playing()))).toBe(20)
    const queue = after(playing(1))
    expect(upNext(queue)).toBe(queue.entries[2])
  })

  it("is nothing after the last, or with nothing queued", () => {
    expect(upNext(after(playing(2)))).toBeNull()
    expect(upNext(emptyQueue)).toBeNull()
  })

  it("is the first after the last with repeat all", () => {
    const last = modes({ repeat: "all" }, 5)
    expect(upNext(last)).toBe(last.entries[0])
  })

  it("is the only entry again with repeat all", () => {
    const one = after(
      { type: "setRepeat", repeat: "all" },
      { type: "play", trackIds: [10], seed: 1 }
    )
    expect(upNext(one)).toBe(currentEntry(one))
  })

  it("is the current entry again with repeat one, the last one too", () => {
    for (const start of [2, 5]) {
      const queue = modes({ repeat: "one" }, start)
      expect(upNext(queue)).toBe(currentEntry(queue))
    }
  })

  it("follows the shuffled order", () => {
    const queue = modes({ shuffle: true })
    expect(upNext(queue)).toBe(queue.entries[1])
  })

  it("is what a track that ends goes on to", () => {
    for (const queue of [
      after(playing()),
      modes({ repeat: "all" }, 5),
      modes({ shuffle: true }, 2),
    ]) {
      const ended = queueReducer(queue, { type: "ended" })
      expect(currentEntry(ended)).toBe(upNext(queue))
    }
  })

  it("changes with the queue after it was loaded ahead", () => {
    const queue = after(playing())
    const ahead = upNext(queue)
    // Play next and add to queue
    expect(next(queueReducer(queue, { type: "playNext", trackIds: [1] }))).toBe(
      1
    )
    expect(
      next(queueReducer(after(playing(2)), { type: "add", trackIds: [1] }))
    ).toBe(1)
    // A move, a removal, and clearing
    const moved = queueReducer(queue, { type: "move", key: ahead!.key, by: 1 })
    expect(next(moved)).toBe(30)
    expect(next(queueReducer(queue, { type: "remove", key: ahead!.key }))).toBe(
      30
    )
    expect(upNext(queueReducer(queue, { type: "clear" }))).toBeNull()
    // Shuffle and repeat
    const shuffled = queueReducer(modes({}), {
      type: "setShuffle",
      shuffle: true,
      seed: 7,
    })
    expect(upNext(shuffled)).toBe(shuffled.entries[1])
    expect(next(shuffled)).not.toBe(20)
    const one = queueReducer(queue, { type: "setRepeat", repeat: "one" })
    expect(upNext(one)).toBe(currentEntry(queue))
    const all = queueReducer(after(playing(2)), {
      type: "setRepeat",
      repeat: "all",
    })
    expect(next(all)).toBe(10)
  })
})

describe("the handoff", () => {
  it("goes on to what was up next, playing", () => {
    const queue = after(playing())
    const handedOff = queueReducer(queue, { type: "handoff" })
    expect(currentEntry(handedOff)).toBe(upNext(queue))
    expect(handedOff).toMatchObject({ current: 1, playing: true })
    expect(handedOff.restarts).toBe(queue.restarts)
  })

  it("goes on to the first after the last with repeat all", () => {
    const last = modes({ repeat: "all" }, 5)
    expect(queueReducer(last, { type: "handoff" })).toMatchObject({
      current: 0,
      playing: true,
    })
  })

  it("doesn't restart the same entry, which already plays again", () => {
    for (const queue of [
      modes({ repeat: "one" }, 2),
      after(
        { type: "setRepeat", repeat: "all" },
        { type: "play", trackIds: [10], seed: 1 }
      ),
    ]) {
      const handedOff = queueReducer(queue, { type: "handoff" })
      expect(handedOff).toBe(queue)
      expect(handedOff.restarts).toBe(queue.restarts)
    }
  })

  it("follows the shuffled order", () => {
    const queue = modes({ shuffle: true }, 2)
    expect(currentEntry(queueReducer(queue, { type: "handoff" }))).toBe(
      queue.entries[1]
    )
  })

  it("goes to what's up next now, when the queue changed after loading ahead", () => {
    const queue = queueReducer(after(playing()), {
      type: "playNext",
      trackIds: [1],
    })
    expect(track(queueReducer(queue, { type: "handoff" }))).toBe(1)
  })

  it("stops as a track that ends does, with nothing up next", () => {
    const last = after(playing(2))
    expect(queueReducer(last, { type: "handoff" })).toEqual(
      queueReducer(last, { type: "ended" })
    )
  })

  it("does nothing with nothing queued", () => {
    expect(queueReducer(emptyQueue, { type: "handoff" })).toBe(emptyQueue)
  })
})

describe("remembering a queue", () => {
  const off = { shuffle: false, repeat: "off" } as const

  it("remembers nothing when nothing's queued", () => {
    expect(savedQueue(emptyQueue)).toBeNull()
  })

  it("comes back paused, with its tracks and its current one", () => {
    const queue = after(
      { type: "play", trackIds: [10, 20, 10], start: 2, seed: 1 },
      { type: "move", key: 2, by: -1 }
    )
    const restored = restoredQueue(savedQueue(queue), off)
    expect(tracks(restored)).toEqual(tracks(queue))
    expect(restored.current).toBe(queue.current)
    expect(restored.playing).toBe(false)
    // New keys, each once, and the next one after them
    expect(new Set(restored.entries.map((entry) => entry.key)).size).toBe(3)
    expect(restored.nextKey).toBe(4)
  })

  it("comes back shuffled, able to unshuffle", () => {
    const shuffle = { shuffle: true, repeat: "all" } as const
    const queue = modes(shuffle, 0)
    const restored = restoredQueue(savedQueue(queue), shuffle)
    expect(tracks(restored)).toEqual(tracks(queue))
    expect(restored.repeat).toBe("all")
    const unshuffle = { type: "setShuffle", shuffle: false, seed: 1 } as const
    expect(tracks(queueReducer(restored, unshuffle))).toEqual(
      tracks(queueReducer(queue, unshuffle))
    )
  })

  it("keeps the order it played in when shuffle has been turned off since", () => {
    const queue = modes({ shuffle: true }, 0)
    expect(tracks(restoredQueue(savedQueue(queue), off))).toEqual(tracks(queue))
  })

  it("comes back empty from anything that isn't a queue", () => {
    for (const saved of [
      null,
      "queue",
      {},
      { trackIds: [], current: 0, unshuffled: null },
      { trackIds: [1, 2], current: 2, unshuffled: null },
      { trackIds: [1, -2], current: 0, unshuffled: null },
      { trackIds: [1, 2], current: 0, unshuffled: [0, 0] },
      { trackIds: [1, 2], current: 0, unshuffled: 3 },
    ]) {
      expect(currentEntry(restoredQueue(saved, off))).toBeNull()
    }
  })
})
