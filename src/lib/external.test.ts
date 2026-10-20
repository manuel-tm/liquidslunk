import { describe, expect, it } from "vitest"

import type { ExternalMembership, ExternalRule, RuleTarget } from "@/bindings"
import {
  describeMembership,
  describeRule,
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
