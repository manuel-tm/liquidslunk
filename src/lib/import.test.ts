import { describe, expect, it } from "vitest"

import type { ArtistSuggestion } from "@/bindings"
import { creatable, pendingReason, settle, unpicked } from "@/lib/import"

/** A tag's suggestion: matched to artist `id`, or to none. */
function tag(name: string, id: number | null, matches = 0): ArtistSuggestion {
  return {
    name,
    artist: id === null ? null : { id, name, nameLanguage: null },
    creditedAs: null,
    matches: id === null ? matches : 1,
    otherNames: [],
  }
}

const option = (id: number, name: string) => ({ id: String(id), name })

describe("unpicked", () => {
  it("keeps the tags that picked no artist, each name once", () => {
    expect(
      unpicked([
        tag("Racer X", null),
        tag("Alika", 3),
        tag("J. Smith", null, 2),
        tag("Racer X", null),
      ])
    ).toEqual([
      { name: "Racer X", matches: 0 },
      { name: "J. Smith", matches: 2 },
    ])
  })
})

describe("a pending name", () => {
  it("can be created when no artist has it", () => {
    expect(creatable({ name: "Racer X", matches: 0 })).toBe(true)
    expect(pendingReason({ name: "Racer X", matches: 0 })).toBe(
      "Not an artist yet"
    )
  })

  it("is chosen when several artists have it", () => {
    expect(creatable({ name: "J. Smith", matches: 2 })).toBe(false)
    expect(pendingReason({ name: "J. Smith", matches: 2 })).toBe(
      "2 artists match"
    )
  })
})

describe("settle", () => {
  const values = {
    artists: [option(3, "Alika")],
    pending: [
      { name: "Racer X", matches: 0 },
      { name: "J. Smith", matches: 2 },
    ],
  }

  it("adds the artist chosen for the name at the end", () => {
    expect(settle(values, "Racer X", option(7, "Racer X"))).toEqual({
      artists: [option(3, "Alika"), option(7, "Racer X")],
      pending: [{ name: "J. Smith", matches: 2 }],
    })
  })

  it("doesn't add an artist that's listed already", () => {
    expect(settle(values, "J. Smith", option(3, "Alika")).artists).toEqual([
      option(3, "Alika"),
    ])
  })

  it("only removes the name when there's no artist", () => {
    expect(settle(values, "Racer X", null)).toEqual({
      artists: [option(3, "Alika")],
      pending: [{ name: "J. Smith", matches: 2 }],
    })
  })
})
