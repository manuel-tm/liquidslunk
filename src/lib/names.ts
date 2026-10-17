/**
 * The alternate name a search matched, when the main name didn't
 * (slunk's 3_alternate_names.md §7.4), so a row can show why it's there. Ignores
 * case, like the backend's search.
 */
export function matchedName<T extends { name: string }>(
  main: string,
  alternates: T[],
  q: string | null | undefined
): T | null {
  const search = q?.normalize("NFC").trim().toLowerCase()
  if (!search || main.toLowerCase().includes(search)) return null
  return (
    alternates.find((alternate) =>
      alternate.name.toLowerCase().includes(search)
    ) ?? null
  )
}
