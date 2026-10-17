/** A list's arguments, as every list query takes them (design §7.2). */
export type ListArgs = {
  page?: number | null
  perPage?: number | null
  /** A column, with `-` in front for descending */
  sort?: string | null
  /** Part of the name, title or path, ignoring case */
  q?: string | null
}

export type Paging = { offset: number; limit: number }

export type Page<T> = { items: T[]; total: number }

/** What a repository's list takes, once the arguments are checked. */
export type ListQuery<Sort extends string> = Paging & {
  sort: Sort | "id"
  descending: boolean
  search?: string
}

/** The rows a page covers. Pages hold at most 200 rows. */
export function pagingOf({ page, perPage }: ListArgs): Paging {
  page ??= 1
  perPage ??= 50
  if (!Number.isInteger(page) || page < 1) throw new Error("No such page")
  const limit = Math.min(Math.max(perPage, 1), 200)
  return { offset: (page - 1) * limit, limit }
}

/** The sort column and direction. `-id`, newest first, is the default. */
export function sortOf<S extends string>(
  sort: string | null | undefined,
  allowed: readonly S[],
  what: string
): { sort: S | "id"; descending: boolean } {
  sort ??= "-id"
  const column = sort.replace(/^-/, "")
  if (column !== "id" && !allowed.includes(column as S)) {
    throw new Error(`${what} can't be sorted by ${column}`)
  }
  return { sort: column as S | "id", descending: sort.startsWith("-") }
}

/**
 * The search text, or undefined when there's nothing to search for. It's in
 * NFC, like saved text, so either form of "é" finds the other.
 */
export function searchOf(q: string | null | undefined): string | undefined {
  return q?.normalize("NFC").trim() || undefined
}

/** Escapes a LIKE pattern, to use with `escape '\'`. */
export function likePattern(search: string): string {
  return `%${search.replace(/[\\%_]/g, "\\$&")}%`
}
