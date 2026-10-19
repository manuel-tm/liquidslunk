import { describe, expect, it } from "vitest"

import {
  currentEntry,
  emptyQueue,
  type Queue,
  type QueueAction,
  queueReducer,
} from "@/lib/queue"

/** The queue after these actions, from an empty one. */
function after(...actions: QueueAction[]): Queue {
  return actions.reduce(queueReducer, emptyQueue)
}

/** A queue of tracks 10, 20 and 30, playing from `start`. */
function playing(start = 0): QueueAction {
  return { type: "play", trackIds: [10, 20, 30], start }
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
    const queue = after({ type: "play", trackIds: [10, 10], start: 0 })
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
    const queue = after(playing(), { type: "play", trackIds: [], start: 0 })
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
    const queue = after({ type: "play", trackIds: [10, 20, 10], start: 0 })
    const removed = queueReducer(queue, {
      type: "remove",
      key: queue.entries[2].key,
    })
    expect(removed.entries.map((entry) => entry.trackId)).toEqual([10, 20])
  })

  it("empties the queue when the only one goes", () => {
    const queue = after({ type: "play", trackIds: [10], start: 0 })
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
    const queue = after({ type: "play", trackIds: [10, 20, 10, 30], start: 3 })
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
