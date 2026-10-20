import { describe, expect, it } from "vitest"

import type {
  ExternalMembership,
  ExternalRule,
  PlanStep,
  RuleTarget,
  SyncPlan,
} from "@/bindings"
import {
  describeChange,
  describeMembership,
  describeOther,
  describeOutcome,
  describePlan,
  describeRule,
  describeSpace,
  describeTagRule,
  ruleInput,
  sameTarget,
} from "@/lib/external"

const header = { id: 1, title: "The Wall", titleLanguage: null }
const artist: RuleTarget = {
  type: "artist",
  artist: { id: 1, name: "Pink Floyd", nameLanguage: null },
}
const release: RuleTarget = {
  type: "release",
  release: { ...header, cover: null, artists: [] },
}
const genre: RuleTarget = {
  type: "tag",
  tag: { id: 3, type: "text", name: "Genre", multiple: true },
  equals: "Jazz",
  min: null,
  max: null,
}

function membership(
  reason: ExternalMembership["reason"],
  tracks = 2,
  total = 2
): ExternalMembership {
  return {
    library: { id: 1, name: "Car", kind: "folder" },
    reason,
    tracks,
    total,
  }
}

describe("describeTagRule", () => {
  it("says a value, or the ends of a range", () => {
    const none = { equals: null, min: null, max: null }
    expect(describeTagRule("Genre", "text", { ...none, equals: "Jazz" })).toBe(
      "Genre is Jazz"
    )
    expect(describeTagRule("Score", "number", { ...none, min: "8" })).toBe(
      "Score ≥ 8"
    )
    expect(describeTagRule("Score", "number", { ...none, max: "5" })).toBe(
      "Score ≤ 5"
    )
    expect(
      describeTagRule("Score", "number", { ...none, min: "6", max: "8" })
    ).toBe("Score 6 to 8")
    expect(describeTagRule("Bought", "date", { ...none, max: "2023" })).toBe(
      "Bought ≤ 2023"
    )
  })
})

describe("describeRule", () => {
  it("names what a rule names", () => {
    expect(describeRule(artist)).toBe("Pink Floyd")
    expect(describeRule(release)).toBe("The Wall")
    expect(
      describeRule({
        type: "track",
        track: { id: 5, title: "Dogs", titleLanguage: null, release: header },
      })
    ).toBe("Dogs")
    expect(describeRule(genre)).toBe("Genre is Jazz")
  })
})

describe("ruleInput", () => {
  it("sends a rule back as it was saved", () => {
    const left: ExternalRule = { exclude: true, target: release }
    expect(ruleInput(left)).toEqual({
      exclude: true,
      target: { type: "release", id: 1 },
    })
    expect(ruleInput({ exclude: false, target: genre })).toEqual({
      exclude: false,
      target: {
        type: "tag",
        tag: { type: "text", id: 3 },
        equals: "Jazz",
        min: null,
        max: null,
      },
    })
    expect(
      sameTarget(
        ruleInput(left),
        ruleInput({ exclude: false, target: release })
      )
    ).toBe(true)
    expect(
      sameTarget(ruleInput(left), ruleInput({ exclude: false, target: artist }))
    ).toBe(false)
  })
})

describe("describeMembership", () => {
  it("says why, and how many of a release's tracks", () => {
    expect(describeMembership(membership({ type: "own" }))).toBe("On Car")
    expect(
      describeMembership(
        membership({
          type: "through",
          rule: { exclude: false, target: artist },
        })
      )
    ).toBe("On Car, through Pink Floyd")
    expect(
      describeMembership(
        membership(
          { type: "through", rule: { exclude: false, target: artist } },
          11,
          12
        )
      )
    ).toBe("On Car, through Pink Floyd (11 of 12 tracks)")
    expect(describeMembership(membership({ type: "tracks" }, 3, 12))).toBe(
      "3 of 12 tracks on Car"
    )
    expect(describeMembership(membership({ type: "tracks" }, 1, 1))).toBe(
      "1 of 1 track on Car"
    )
  })
})

function step(
  action: PlanStep["action"],
  bytes = 0,
  trackId: number | null = 1
): PlanStep {
  return {
    action,
    trackId,
    releaseId: trackId === null ? 1 : null,
    label: { title: "Dogs", artist: "Pink Floyd", album: "Animals" },
    path: "Pink Floyd/Animals/02 Dogs.m4a",
    from: null,
    changes: [],
    bytes,
  }
}

function plan(
  steps: PlanStep[],
  free: number | null = 10_000_000_000
): SyncPlan {
  const added = steps.reduce((sum, step) => sum + step.bytes, 0)
  return {
    id: 1,
    libraryId: 1,
    steps,
    leftOut: [],
    unchanged: 120,
    addedBytes: added,
    freedBytes: 0,
    neededBytes: added,
    freeBytes: free,
    fits: free === null || added <= free,
    ipod: null,
    other: [],
  }
}

describe("sync plans", () => {
  it("say what they do, counting tracks", () => {
    expect(
      describePlan(
        plan([
          step("add", 1_000_000_000),
          step("add", 800_000_000),
          step("retag"),
          step("move"),
          step("remove"),
          step("add", 0, null),
        ])
      )
    ).toBe("Add 2 tracks (1.8 GB), update 2, remove 1, and 1 cover.")
    expect(describePlan(plan([step("retag")]))).toBe("Update 1 track.")
    expect(describePlan(plan([]))).toBe("Everything is up to date: 120 files.")
  })

  it("say whether they fit", () => {
    expect(describeSpace(plan([step("add", 3_200_000_000)]))).toBe(
      "It needs 3.2 GB, and 10 GB is free."
    )
    expect(describeSpace(plan([step("add", 13_200_000_000)]))).toBe(
      "It needs 3.2 GB more than the folder has free."
    )
    expect(describeSpace(plan([], null))).toBeNull()
    const ipod = {
      name: "IPOD",
      mount: "/Volumes/IPOD",
      description: "iPod with video (5.5th generation), 30 GB, black",
      refusal: null,
      warning: null,
    }
    expect(
      describeSpace({ ...plan([step("add", 13_200_000_000)]), ipod })
    ).toBe("It needs 3.2 GB more than the iPod has free.")
  })

  it("say what a retag changes, and how a sync went", () => {
    expect(
      describeChange({ field: "Title", was: ["Shoka"], now: ["初夏"] })
    ).toBe("Title: Shoka → 初夏")
    expect(
      describeChange({ field: "Disc title", was: [], now: ["Live"] })
    ).toBe("Disc title: none → Live")
    const outcome = {
      added: 42,
      converted: 0,
      restored: 0,
      retagged: 7,
      moved: 0,
      removed: 3,
      failed: [],
      cancelled: false,
      bytes: 0,
      erased: 0,
    }
    expect(describeOutcome(outcome)).toBe("Added 42, retagged 7 and removed 3.")
    expect(
      describeOutcome({ ...outcome, added: 0, retagged: 0, removed: 0 })
    ).toBe("Nothing changed.")
    expect(describeOutcome({ ...outcome, erased: 1234, retagged: 0 })).toBe(
      "Erased 1,234 tracks from somewhere else. Added 42 and removed 3."
    )
    expect(describeOther(1234)).toBe(
      "This iPod has 1,234 tracks from somewhere else. Erase them, then sync?"
    )
  })
})
