/**
 * Text as it's saved: in Unicode NFC, then trimmed (3_alternate_names.md
 * §4.2). The same characters can come in two forms, like "é" as one code
 * point or as "e" and an accent, which don't compare equal. NFC makes them
 * one. NFKC isn't used: it would rewrite what's printed, turning "E=MC²" into
 * "E=MC2". Paths aren't text: they have to match the disk.
 */
export function cleanText(text: string): string {
  return text.normalize("NFC").trim()
}

/** Cleaned text, or null when it's blank. */
export function cleanOrNull(text: string | null | undefined): string | null {
  return text == null ? null : cleanText(text) || null
}
