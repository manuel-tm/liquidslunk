// Dates as the user types them, stored as epoch seconds at midnight UTC
// (design §4.3 Dates). Release dates and date tags both use them.

export type DatePrecision = "year" | "month" | "day"

export type ParsedDate = { epoch: number; precision: DatePrecision }

/**
 * Parses `2006`, `2006-01` or `2006-01-10`. Unknown parts become the first
 * one, at midnight UTC.
 */
export function parseDate(text: string): ParsedDate {
  const match = /^(\d{4})(?:-(\d{2})(?:-(\d{2}))?)?$/.exec(text.trim())
  if (!match) {
    throw new Error(
      `Not a date: ${text}. Write it as 2006, 2006-01 or 2006-01-10.`
    )
  }
  const [, year, month, day] = match
  const time = Date.UTC(Number(year), Number(month ?? 1) - 1, Number(day ?? 1))
  if (
    formatDate(time / 1000, "day") !== `${year}-${month ?? "01"}-${day ?? "01"}`
  ) {
    throw new Error(`There's no such date: ${text}`)
  }
  return {
    epoch: time / 1000,
    precision: day ? "day" : month ? "month" : "year",
  }
}

/** The last day of a parsed date's year, month or day, at midnight UTC. */
export function lastDay({ epoch, precision }: ParsedDate): number {
  const date = new Date(epoch * 1000)
  const next = Date.UTC(
    date.getUTCFullYear() + (precision === "year" ? 1 : 0),
    date.getUTCMonth() + (precision === "month" ? 1 : 0),
    date.getUTCDate() + (precision === "day" ? 1 : 0)
  )
  return next / 1000 - 86400
}

/** A date with only its known parts: `2006`, `2006-01` or `2006-01-10`. */
export function formatDate(epoch: number, precision: DatePrecision): string {
  const iso = new Date(epoch * 1000).toISOString()
  return iso.slice(0, { year: 4, month: 7, day: 10 }[precision])
}
