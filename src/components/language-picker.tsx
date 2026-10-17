import { useQuery } from "@apollo/client/react"
import { useState } from "react"

import { type Option, Picker } from "@/components/picker"
import { graphql } from "@/gql"
import { canonicalTag, LANGUAGES, languageName } from "@/lib/languages"

const LanguagesQuery = graphql(`
  query Languages {
    languages
  }
`)

// Like the other pickers (slunk's design §7.3)
const LIMIT = 20

const optionOf = (tag: string): Option => ({
  id: tag,
  name: `${languageName(tag)} · ${tag}`,
})

/**
 * A language tag (slunk's 3_alternate_names.md §7.6). It offers the tags already in
 * use first, so a language keeps one tag, then searches every language by
 * name or tag. Any other valid tag can be typed in.
 */
export function LanguagePicker({
  value,
  onChange,
  id,
  placeholder = "Language",
  "aria-label": ariaLabel,
}: {
  value: string | null
  onChange: (tag: string | null) => void
  id?: string
  placeholder?: string
  "aria-label"?: string
}) {
  const [query, setQuery] = useState("")
  const { data } = useQuery(LanguagesQuery)
  const tags = [...new Set([...(data?.languages ?? []), ...LANGUAGES])]
  const typed = query.trim().toLowerCase()
  const options = tags
    .map(optionOf)
    .filter((option) => option.name.toLowerCase().includes(typed))
    .slice(0, LIMIT)
  const custom = canonicalTag(query)
  const offerCustom = custom !== null && !tags.includes(custom)

  return (
    <Picker
      id={id}
      aria-label={ariaLabel}
      noun="language"
      placeholder={placeholder}
      value={value ? optionOf(value) : null}
      onChange={(option) => onChange(option?.id ?? null)}
      options={options}
      query={query}
      onQuery={setQuery}
      onCreate={offerCustom ? async () => optionOf(custom) : undefined}
      createLabel={() => `Use the tag ${custom}`}
    />
  )
}
