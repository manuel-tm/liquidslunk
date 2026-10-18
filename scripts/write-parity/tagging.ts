// Phase 3's steps: the tagging context, tag definitions and a release's
// tags. They run after phase 2's, on the seeded library: text tag 1 is
// Genre, several values, on Nothing Was the Same (release 1) as Hip Hop, and
// on High School Musical (release 2) as Pop and Stage & Screen; number tag 1
// is Score, one value, 9 and 7.5 on those. Phase 1's track 1, on release 1,
// gets tags here, through GraphQL: tracks' tags are set from phase 5.
import type { Phase, Step } from "./media.ts"

const TAG = "id type name multiple"
const OWNED = `tags { tag { ${TAG} } values }`

type TagType = "text" | "number" | "date"
type Args = Record<string, unknown>
type Values = [TagType, number, string[]][]

const tags = (name: string, args: Args = {}): Step => ({
  name: `tags ${name}`,
  graphql: `query ($type: TagType, $page: Int, $perPage: Int, $sort: String, $q: String) {
    tags(type: $type, page: $page, perPage: $perPage, sort: $sort, q: $q) {
      total items { ${TAG} }
    }
  }`,
  variables: () => args,
  command: "tags",
  args: () => args,
})

const tag = (type: TagType, id: number): Step => ({
  name: `tag ${type} ${id}`,
  graphql: `query ($type: TagType!, $id: ID!) {
    tag(type: $type, id: $id) {
      ${TAG} releaseCount trackCount blockers
      multiValueReleases(perPage: 20) { total items { id title } }
      multiValueTracks(perPage: 20) { total items { id title } }
    }
  }`,
  variables: () => ({ type, id }),
  command: "tag",
  args: () => ({ type, id }),
})

const tagValues = (name: string, id: number, args: Args = {}): Step => ({
  name: `tagValues ${id} ${name}`,
  graphql: `query ($id: ID!, $page: Int, $perPage: Int, $q: String) {
    tag(type: text, id: $id) {
      ... on TextTag {
        values(page: $page, perPage: $perPage, q: $q) {
          total items { value releases tracks }
        }
      }
    }
  }`,
  variables: () => ({ id, ...args }),
  command: "tagValues",
  args: () => ({ id, ...args }),
  fromGraphql: (found) =>
    (found as { values: unknown } | null)?.values ?? { items: [], total: 0 },
})

const tagOptions = (q: string): Step => ({
  name: `tagOptions "${q}"`,
  graphql: `query ($q: String) {
    tags(q: $q, perPage: 20, sort: "name") { items { ${TAG} } }
  }`,
  variables: () => ({ q }),
  command: "tagOptions",
  args: () => ({ q }),
  fromGraphql: (page) => (page as { items: unknown }).items,
})

const tagValueOptions = (id: number, q: string): Step => ({
  name: `tagValueOptions ${id} "${q}"`,
  graphql: `query ($id: ID!, $q: String) {
    tag(type: text, id: $id) {
      ... on TextTag { values(q: $q, perPage: 20) { items { value } } }
    }
  }`,
  variables: () => ({ id, q }),
  command: "tagValueOptions",
  args: () => ({ id, q }),
  fromGraphql: (found) =>
    (
      found as { values: { items: { value: string }[] } } | null
    )?.values.items.map((item) => item.value) ?? [],
})

const tagsByRef = (refs: { type: TagType; id: number }[]): Step => ({
  name: `tagsByRef ${refs.map((ref) => `${ref.type} ${ref.id}`).join(", ")}`,
  graphql: `query ($refs: [TagRefInput!]!) { tagsByRef(refs: $refs) { ${TAG} } }`,
  variables: () => ({ refs }),
  command: "tagsByRef",
  args: () => ({ refs }),
})

const createTag = (type: TagType, input: Args): Step => ({
  name: `createTag ${type} ${JSON.stringify(input)}`,
  graphql: `mutation ($type: TagType!, $input: TagInput!) {
    createTag(type: $type, input: $input) { ${TAG} }
  }`,
  variables: () => ({ type, input }),
  command: "createTag",
  args: () => ({ type, ...input }),
})

const updateTag = (type: TagType, id: number, input: Args): Step => ({
  name: `updateTag ${type} ${id} ${JSON.stringify(input)}`,
  graphql: `mutation ($type: TagType!, $id: ID!, $input: TagInput!) {
    updateTag(type: $type, id: $id, input: $input) { ${TAG} }
  }`,
  variables: () => ({ type, id, input }),
  command: "updateTag",
  args: () => ({ type, id, ...input }),
})

const deleteTag = (type: TagType, id: number): Step => ({
  name: `deleteTag ${type} ${id}`,
  graphql: `mutation ($type: TagType!, $id: ID!) { deleteTag(type: $type, id: $id) }`,
  variables: () => ({ type, id }),
  command: "deleteTag",
  args: () => ({ type, id }),
  returnsId: true,
})

const releaseTags = (id: number): Step => ({
  name: `releaseTags ${id}`,
  graphql: `query ($id: ID!) { release(id: $id) { ${OWNED} } }`,
  variables: () => ({ id }),
  command: "releaseTags",
  args: () => ({ id }),
  fromGraphql: (release) => (release as { tags: unknown } | null)?.tags ?? null,
})

const setReleaseTags = (name: string, id: number, values: Values): Step => {
  const tags = values.map(([type, tagId, values]) => ({ type, tagId, values }))
  return {
    name: `setReleaseTags ${id} ${name}`,
    graphql: `mutation ($id: ID!, $tags: [TagValuesInput!]!) {
      setReleaseTags(id: $id, tags: $tags) { ${OWNED} }
    }`,
    variables: () => ({ id, tags }),
    command: "setReleaseTags",
    args: () => ({ id, tags }),
    fromGraphql: (release) => (release as { tags: unknown }).tags,
  }
}

// Tags this phase creates: text 2 is Mood, number 2 Rating, date 1 Bought
const GENRE = 1
const SCORE = 1
const MOOD = 2
const RATING = 2
const BOUGHT = 1

// High School Musical's tags, which a save of its Mood has to send again
const HSM: Values = [
  ["text", GENRE, ["Pop", "Stage & Screen"]],
  ["number", SCORE, ["7.5"]],
]

export const tagging: Phase = {
  // Track 1 gets two Genre values, one in another case than release 1 will
  // have it, and a Score
  prepare: [
    {
      graphql: `mutation {
        updateTrack(id: 1, input: {
          releaseId: 1, fileId: 1, trackNumber: 1, title: "Early Summer",
          tags: [
            { type: text, tagId: 1, values: ["jazz", "Bebop"] },
            { type: number, tagId: 1, values: ["8"] }
          ]
        }) { id }
      }`,
    },
  ],
  steps: [
    tags("by default"),
    tags("of every type, by name", { sort: "name" }),
    tags("of one type", { type: "number" }),
    tags("searched", { q: "E", sort: "-name" }),
    tags("paged", { page: 2, perPage: 1, sort: "name" }),
    tags("by a column it can't", { sort: "multiple" }),
    tags("on page 0", { page: 0 }),
    tag("text", GENRE),
    tag("number", SCORE),
    tag("date", 1),
    tagValues("of Genre", GENRE),
    tagValues("searched", GENRE, { q: "P" }),
    tagValues("paged", GENRE, { page: 2, perPage: 2 }),
    tagValues("of a tag that isn't there", 999),
    tagOptions(""),
    tagOptions("s"),
    tagValueOptions(GENRE, ""),
    tagValueOptions(GENRE, "b"),
    tagsByRef([
      { type: "text", id: GENRE },
      { type: "number", id: SCORE },
      { type: "date", id: 1 },
      { type: "text", id: GENRE },
    ]),

    createTag("text", { name: "Mood", multiple: true }),
    createTag("number", { name: " Rating ", multiple: true }),
    createTag("date", { name: "Bought" }),
    createTag("text", { name: "score" }),
    createTag("text", { name: "GENRE", multiple: true }),
    createTag("date", { name: "  " }),
    tags("after creating", { sort: "name" }),

    setReleaseTags("cleaned, once each, in order", 1, [
      ["text", GENRE, ["Jazz", "jazz", " Bebop ", ""]],
      ["number", SCORE, ["8.50"]],
      ["date", BOUGHT, ["2006"]],
      ["text", MOOD, [" "]],
    ]),
    releaseTags(1),
    setReleaseTags("with numbers as JavaScript writes them", 3, [
      [
        "number",
        RATING,
        [
          "1e21",
          "1e-7",
          "0.000001",
          "-0",
          "0",
          "0.1",
          "123.456",
          "+5",
          ".5",
          "5.",
          "1E3",
          "1e3",
          "2.675",
          "-848658235700005.25",
        ],
      ],
    ]),
    setReleaseTags("with a leap day", 4, [
      ["date", BOUGHT, ["2024-02-29"]],
      ["text", MOOD, ["Calm"]],
    ]),
    setReleaseTags("in the year 100", 5, [["date", BOUGHT, ["0100-12"]]]),
    setReleaseTags("with two values for Score", 1, [
      ["number", SCORE, ["8", "9"]],
    ]),
    setReleaseTags("with a word for a number", 1, [["number", SCORE, ["abc"]]]),
    setReleaseTags("with an infinite number", 1, [
      ["number", RATING, ["1e400"]],
    ]),
    setReleaseTags("with no such day", 1, [["date", BOUGHT, ["2006-02-30"]]]),
    setReleaseTags("with a short year", 1, [["date", BOUGHT, ["06"]]]),
    setReleaseTags("in the year 99", 1, [["date", BOUGHT, ["0099"]]]),
    setReleaseTags("with a tag that isn't there", 1, [["text", 999, []]]),
    setReleaseTags("that isn't there", 999, []),
    releaseTags(999),
    releaseTags(1),
    releaseTags(3),

    // Switching a tag to single
    updateTag("text", MOOD, { name: "Moods", multiple: true }),
    setReleaseTags("with two moods", 2, [
      ...HSM,
      ["text", MOOD, ["Happy", "Upbeat"]],
    ]),
    updateTag("text", MOOD, { name: "Moods", multiple: false }),
    tag("text", MOOD),
    setReleaseTags("with one mood", 2, [...HSM, ["text", MOOD, ["Happy"]]]),
    updateTag("text", MOOD, { name: "Mood", multiple: false }),
    setReleaseTags("with two moods, single", 4, [
      ["text", MOOD, ["Calm", "Quiet"]],
    ]),
    updateTag("text", GENRE, { name: "Genre", multiple: false }),
    updateTag("text", GENRE, { name: "GENRE", multiple: true }),
    updateTag("number", 999, { name: "Nothing" }),
    updateTag("number", RATING, { name: "bought", multiple: true }),
    updateTag("number", SCORE, { name: "Score", multiple: true }),
    updateTag("number", SCORE, { name: "Score" }),
    tag("text", GENRE),
    tag("text", MOOD),
    tagValues("after the saves", GENRE),
    tagValueOptions(GENRE, "J"),

    // Deletes
    tag("date", BOUGHT),
    deleteTag("date", BOUGHT),
    deleteTag("text", GENRE),
    setReleaseTags("with none", 3, []),
    deleteTag("number", RATING),
    deleteTag("number", 999),
    tags("at the end", { sort: "name" }),
  ],
}
