import type { Tagging } from "../../slunk/tagging.ts"
import type { Tag } from "../../slunk/tagging/repository.ts"
import type { Context } from "./context.ts"
import { loadAll, loadRequired } from "./loaders.ts"
import type { Resolvers } from "./resolvers_types.ts"

const TAG_TYPES = {
  text: "TextTag",
  number: "NumberTag",
  date: "DateTag",
} as const

/** A GraphQL id, as the database's number. */
const n = (id: string | number) => Number(id)

/** The fields every tag type resolves the same way. */
const tagFields = {
  releaseCount: async (tag: Tag, _args: unknown, { tagging }: Context) =>
    (await tagging.tagUsage(tag.type, tag.id)).releases,
  trackCount: async (tag: Tag, _args: unknown, { tagging }: Context) =>
    (await tagging.tagUsage(tag.type, tag.id)).tracks,
  blockers: (tag: Tag, _args: unknown, { tagging }: Context) =>
    tagging.tagBlockers(tag.type, tag.id),
  multiValueReleases: async (
    tag: Tag,
    args: { page: number; perPage: number },
    { tagging, loaders }: Context
  ) => {
    const { items, total } = await tagging.multiValueOwners(
      "release",
      tag.type,
      tag.id,
      args
    )
    return { items: await loadAll(loaders.release, items), total }
  },
  multiValueTracks: async (
    tag: Tag,
    args: { page: number; perPage: number },
    { tagging, loaders }: Context
  ) => {
    const { items, total } = await tagging.multiValueOwners(
      "track",
      tag.type,
      tag.id,
      args
    )
    return { items: await loadAll(loaders.track, items), total }
  },
}

type TagValuesArg = { type: Tag["type"]; tagId: string; values: string[] }

/** An owner's tags, from the API's input. */
export const tagValues = (inputs: TagValuesArg[]) =>
  inputs.map((input) => ({ ...input, tagId: n(input.tagId) }))

/** List filters, from the API's input. */
export function tagFilters(
  tagging: Tagging,
  inputs: {
    type: Tag["type"]
    tagId: string
    equals?: string | null
    contains?: string | null
    min?: string | null
    max?: string | null
  }[]
) {
  return tagging.tagFilters(
    inputs.map((input) => ({ ...input, tagId: n(input.tagId) }))
  )
}

export const taggingResolvers: Resolvers = {
  Query: {
    tags: (_parent, args, { tagging }) => tagging.listTags(args),
    tag: (_parent, { type, id }, { tagging }) => tagging.getTag(type, n(id)),
    tagsByRef: async (_parent, { refs }, { loaders }) => {
      const tags = await Promise.all(
        refs.map((ref) => loaders.tag[ref.type].load(n(ref.id)))
      )
      return tags.filter((tag) => tag !== null)
    },
  },

  Mutation: {
    createTag: (_parent, { type, input }, { tagging }) =>
      tagging.createTag(type, input),
    updateTag: (_parent, { type, id, input }, { tagging }) =>
      tagging.updateTag(type, n(id), input),
    deleteTag: async (_parent, { type, id }, { tagging }) => {
      await tagging.deleteTag(type, n(id))
      return id
    },
    setReleaseTags: async (_parent, { id, tags }, { catalog, tagging }) => {
      const release = await catalog.getRelease(n(id))
      if (!release) throw new Error(`There's no release ${id}`)
      await tagging.setTags("release", release.id, tagValues(tags))
      return release
    },
    setTagOnReleases: (_parent, { ids, type, tagId, value }, { tagging }) =>
      tagging.setTagOnReleases(ids.map(n), { type, tagId: n(tagId), value }),
    removeTagFromReleases: (
      _parent,
      { ids, type, tagId, value },
      { tagging }
    ) =>
      tagging.removeTagFromReleases(ids.map(n), {
        type,
        tagId: n(tagId),
        value,
      }),
  },

  Tag: {
    __resolveType: (tag) => TAG_TYPES[tag.type],
  },
  TextTag: {
    ...tagFields,
    values: (tag, args, { tagging }) => tagging.tagValues(tag.id, args),
  },
  NumberTag: tagFields,
  DateTag: tagFields,

  TagValues: {
    tag: (owned, _args, { loaders }) =>
      loadRequired(loaders.tag[owned.type], owned.tagId),
    values: (owned, _args, { tagging }) =>
      owned.values.map((value) => tagging.formatValue(owned.type, value)),
  },

  Release: {
    tags: (release, _args, { loaders }) => loaders.releaseTags.load(release.id),
  },

  Track: {
    tags: (track, _args, { loaders }) => loaders.trackTags.load(track.id),
  },
}
