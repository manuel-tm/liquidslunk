import type {
  ExternalKind,
  ExternalMembership,
  ExternalRule,
  OutputFormat,
  RuleInput,
  RuleTarget,
  SyncOutcome,
  SyncPlan,
  SyncStage,
  TagChange,
} from "@/bindings"
import { formatReleased, formatSize } from "@/lib/format"

/** Each kind, as the pages name it. */
export const KIND_LABELS: Record<ExternalKind, string> = {
  folder: "Folder library",
  ipod: "iPod library",
}

/** What lossless sources become, as the profile names it. */
export const FORMAT_LABELS: Record<OutputFormat, string> = {
  original: "Keep the original",
  flac: "FLAC",
  alac: "Apple Lossless",
  aac: "AAC",
}

/** The formats each kind takes: an iPod plays Apple Lossless or AAC. */
export const FORMATS: Record<ExternalKind, OutputFormat[]> = {
  folder: ["original", "flac", "alac", "aac"],
  ipod: ["alac", "aac"],
}

/** A tag value rule's filter: "Genre is Jazz", "Score ≥ 8", "Bought 2023". */
export function describeTagRule(
  name: string,
  type: "text" | "number" | "date",
  {
    equals,
    min,
    max,
  }: { equals: string | null; min: string | null; max: string | null }
): string {
  const shown = (value: string) =>
    type === "date" ? formatReleased(value) : value
  if (equals !== null) return `${name} is ${shown(equals)}`
  if (min !== null && max !== null) {
    return `${name} ${shown(min)} to ${shown(max)}`
  }
  if (min !== null) return `${name} ≥ ${shown(min)}`
  return `${name} ≤ ${shown(max ?? "")}`
}

/** What a rule names, in words: a release's title, an artist's name, a tag filter. */
export function describeRule(target: RuleTarget): string {
  switch (target.type) {
    case "release":
      return target.release.title
    case "artist":
      return target.artist.name
    case "track":
      return target.track.title
    case "tag":
      return describeTagRule(target.tag.name, target.tag.type, target)
  }
}

/** A saved rule, to send back with the others. */
export function ruleInput(rule: ExternalRule): RuleInput {
  const { target } = rule
  switch (target.type) {
    case "release":
      return {
        exclude: rule.exclude,
        target: { type: "release", id: target.release.id },
      }
    case "artist":
      return {
        exclude: rule.exclude,
        target: { type: "artist", id: target.artist.id },
      }
    case "track":
      return {
        exclude: rule.exclude,
        target: { type: "track", id: target.track.id },
      }
    case "tag":
      return {
        exclude: false,
        target: {
          type: "tag",
          tag: { type: target.tag.type, id: target.tag.id },
          equals: target.equals,
          min: target.min,
          max: target.max,
        },
      }
  }
}

/** Whether two rules name the same thing. */
export function sameTarget(a: RuleInput, b: RuleInput): boolean {
  return JSON.stringify(a.target) === JSON.stringify(b.target)
}

/**
 * Where something is in an external library, in words: "On Car", "On Car,
 * through Pink Floyd", or for a release with only some tracks in it, "3 of 12
 * tracks on Car".
 */
export function describeMembership(membership: ExternalMembership): string {
  const { library, reason, tracks, total } = membership
  if (reason.type === "tracks") {
    return `${tracks} of ${total} ${total === 1 ? "track" : "tracks"} on ${library.name}`
  }
  const through =
    reason.type === "through"
      ? `, through ${describeRule(reason.rule.target)}`
      : ""
  const some = tracks < total ? ` (${tracks} of ${total} tracks)` : ""
  return `On ${library.name}${through}${some}`
}

function count(n: number, noun: string): string {
  return `${n.toLocaleString("en-US")} ${n === 1 ? noun : `${noun}s`}`
}

/**
 * What a sync's plan does, in short, counting tracks: "Add 42 tracks
 * (1.8 GB), update 7, remove 3." (4_external_libraries.md §8.2).
 */
export function describePlan(plan: SyncPlan): string {
  const tracks = plan.steps.filter((step) => step.trackId !== null)
  const added = tracks.filter((step) => step.action === "add")
  const removed = tracks.filter((step) => step.action === "remove")
  const updated = tracks.length - added.length - removed.length
  const covers = plan.steps.length - tracks.length
  const parts: string[] = []
  if (added.length > 0) {
    const bytes = added.reduce((sum, step) => sum + step.bytes, 0)
    parts.push(`add ${count(added.length, "track")} (${formatSize(bytes)})`)
  }
  if (updated > 0) {
    parts.push(
      parts.length === 0
        ? `update ${count(updated, "track")}`
        : `update ${updated}`
    )
  }
  if (removed.length > 0) {
    parts.push(
      parts.length === 0
        ? `remove ${count(removed.length, "track")}`
        : `remove ${removed.length}`
    )
  }
  if (parts.length === 0) {
    return covers > 0
      ? `Write ${count(covers, "cover")}.`
      : `Everything is up to date: ${count(plan.unchanged, "file")}.`
  }
  const sentence = parts.join(", ")
  const coverPart = covers > 0 ? `, and ${count(covers, "cover")}` : ""
  return `${sentence.charAt(0).toUpperCase()}${sentence.slice(1)}${coverPart}.`
}

/** Whether a plan's sync fits, in words, or how much it's short. */
export function describeSpace(plan: SyncPlan): string | null {
  if (plan.freeBytes === null) return null
  if (!plan.fits) {
    const where = plan.ipod ? "the iPod" : "the folder"
    return `It needs ${formatSize(plan.neededBytes - plan.freeBytes)} more than ${where} has free.`
  }
  if (plan.neededBytes <= 0) {
    return `${formatSize(plan.freeBytes)} is free.`
  }
  return `It needs ${formatSize(plan.neededBytes)}, and ${formatSize(plan.freeBytes)} is free.`
}

/** What a retag changes in a field: "Title: Shoka → 初夏". */
export function describeChange({ field, was, now }: TagChange): string {
  const shown = (values: string[]) =>
    values.length === 0 ? "none" : values.join(", ")
  return `${field}: ${shown(was)} → ${shown(now)}`
}

/** Where a running sync is, as its progress says it. */
export const STAGE_LABELS: Record<SyncStage, string> = {
  erasing: "Erasing the other music",
  removing: "Removing files",
  moving: "Moving files",
  writing: "Converting and writing",
  retagging: "Retagging",
  covers: "Writing covers",
  database: "Writing the iPod's database",
  finishing: "Finishing",
}

/**
 * What an iPod's first sync asks before it erases the tracks liquidslunk
 * didn't write (4_external_libraries.md §7.8): "This iPod has 1,234 tracks
 * from somewhere else. Erase them, then sync?"
 */
export function describeOther(tracks: number): string {
  return `This iPod has ${count(tracks, "track")} from somewhere else. Erase ${tracks === 1 ? "it" : "them"}, then sync?`
}

/**
 * How a sync went: "Added 42, retagged 7 and removed 3.", after "Erased 12
 * tracks from somewhere else." on an iPod.
 */
export function describeOutcome(outcome: SyncOutcome): string {
  const erased =
    outcome.erased > 0
      ? `Erased ${count(outcome.erased, "track")} from somewhere else. `
      : ""
  const parts = (
    [
      ["added", outcome.added],
      ["converted again", outcome.converted],
      ["restored", outcome.restored],
      ["retagged", outcome.retagged],
      ["moved", outcome.moved],
      ["removed", outcome.removed],
    ] as const
  )
    .filter(([, n]) => n > 0)
    .map(([verb, n]) => `${verb} ${n}`)
  if (parts.length === 0) {
    return `${erased}${erased ? "Nothing else changed." : "Nothing changed."}`
  }
  const last = parts.pop()
  const sentence = parts.length > 0 ? `${parts.join(", ")} and ${last}` : last!
  return `${erased}${sentence.charAt(0).toUpperCase()}${sentence.slice(1)}.`
}
