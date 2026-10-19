const SIZE_UNITS = ["KB", "MB", "GB", "TB"]

/** A size in B, KB, MB or GB, in powers of 1000 as macOS shows them. */
export function formatSize(bytes: number): string {
  if (bytes < 1000) return `${bytes} B`
  let value = bytes
  let unit = -1
  while (value >= 1000 && unit < SIZE_UNITS.length - 1) {
    value /= 1000
    unit++
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${SIZE_UNITS[unit]}`
}

/** An instant, in epoch seconds, as a date and time in the viewer's zone. */
export function formatDateTime(seconds: number): string {
  return new Date(seconds * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  })
}

/** An instant, in epoch seconds, as a date in the viewer's zone. */
export function formatDate(seconds: number): string {
  return new Date(seconds * 1000).toLocaleDateString(undefined, {
    dateStyle: "medium",
  })
}

/** A duration as `m:ss`, or `h:mm:ss` from an hour. */
export function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return ""
  const total = Math.round(ms / 1000)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const seconds = String(total % 60).padStart(2, "0")
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}`
    : `${minutes}:${seconds}`
}

/**
 * A release date with only its known parts: `2006`, `Jan 2006` or `Jan 10,
 * 2006`. It comes as `2006`, `2006-01` or `2006-01-10`, and isn't shifted to
 * the viewer's time zone.
 */
export function formatReleased(released: string | null | undefined): string {
  if (!released) return ""
  const [year, month, day] = released.split("-").map(Number)
  const date = new Date(Date.UTC(year, (month ?? 1) - 1, day ?? 1))
  return date.toLocaleDateString(undefined, {
    timeZone: "UTC",
    year: "numeric",
    month: month ? "short" : undefined,
    day: day ? "numeric" : undefined,
  })
}

/**
 * A credit role, to compare ignoring ASCII case as the database does: "Mixed
 * By" and "Mixed by" are the same role.
 */
export function roleKey(role: string): string {
  return role.trim().replace(/[A-Z]/g, (c) => c.toLowerCase())
}
