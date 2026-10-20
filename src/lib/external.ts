import type {
  ExternalKind,
  ExternalMembership,
  ExternalRule,
  OutputFormat,
  RuleInput,
  RuleTarget,
} from "@/bindings"
import { formatReleased } from "@/lib/format"

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
