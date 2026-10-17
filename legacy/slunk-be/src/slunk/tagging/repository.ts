import type { ListQuery, Page, Paging } from "../paging.ts"

/** Each type has its own tables, so the same id exists once per type. */
export type TagType = "text" | "number" | "date"

export const TAG_TYPES: readonly TagType[] = ["text", "number", "date"]

export type Tag = {
  type: TagType
  id: number
  name: string
  /** Several values per release or track, or at most one (design §4.3) */
  multiple: boolean
}

export type TagValues = { name: string; multiple: boolean }

/** What tags are set on: releases, and since phase 4 tracks (design §8). */
export type TagOwner = "release" | "track"

/** Text, a number, or a date as epoch seconds at midnight UTC. */
export type TagValue = string | number

/** One value of a tag, on a release or track. */
export type TagValueRow = { type: TagType; tagId: number; value: TagValue }

/** How many releases and tracks have a tag, or one of its values. */
export type TagUsage = { releases: number; tracks: number }

/** A text tag's value, and how many releases and tracks have it. */
export type ValueCount = TagUsage & { value: string }

/**
 * A list filter on one tag. Text matches `equals` or `contains`, ignoring
 * case. Numbers and dates match the range, both ends included.
 */
export type TagFilter = {
  type: TagType
  tagId: number
  equals?: string
  contains?: string
  min?: number
  max?: number
}

export interface TagsRepository {
  /** Every type's tags when `type` is left out */
  list(query: ListQuery<"name"> & { type?: TagType }): Promise<Page<Tag>>
  get(type: TagType, id: number): Promise<Tag | null>
  getMany(type: TagType, ids: readonly number[]): Promise<Tag[]>
  /** Tags of any type with exactly this name, ignoring case */
  findByName(name: string): Promise<Tag[]>
  insert(type: TagType, values: TagValues): Promise<Tag>
  update(type: TagType, id: number, values: TagValues): Promise<Tag | null>
  delete(type: TagType, id: number): Promise<void>
  /** How many releases and tracks have a value */
  usage(type: TagType, id: number): Promise<TagUsage>
  /** A text tag's values, the most used first */
  values(
    id: number,
    query: Paging & { search?: string }
  ): Promise<Page<ValueCount>>
  /** Owners with several values, which keep a tag from being single-value */
  multiValueOwners(
    owner: TagOwner,
    type: TagType,
    id: number,
    paging: Paging
  ): Promise<Page<number>>
  /** Values by owner: by tag name, then in the order they were set */
  valuesOf(
    owner: TagOwner,
    ids: readonly number[]
  ): Promise<Map<number, TagValueRow[]>>
  /** Replaces every tag value on a release or track, keeping their order */
  setValues(owner: TagOwner, id: number, values: TagValueRow[]): Promise<void>
  /**
   * Adds a value to each owner that doesn't have it. With `replace`, their
   * other values of the tag go first. Returns how many owners changed.
   */
  addValue(
    owner: TagOwner,
    ids: readonly number[],
    value: TagValueRow,
    replace: boolean
  ): Promise<number>
  /**
   * Removes a tag's values from each owner: every value, or just `value`.
   * Returns how many owners changed.
   */
  removeValues(
    owner: TagOwner,
    ids: readonly number[],
    tag: { type: TagType; tagId: number; value?: TagValue }
  ): Promise<number>
}

/** Every repository tagging needs, and a way to use them in one transaction. */
export interface TaggingStore {
  tags: TagsRepository
  transaction<T>(fn: (store: TaggingStore) => Promise<T>): Promise<T>
}
