import type { ComponentProps } from "react"

import { useTagValues } from "@/components/pickers"
import { Input } from "@/components/ui/input"
import type { TagType } from "@/gql/graphql"
import { TAG_PLACEHOLDERS } from "@/lib/tags"

/**
 * A tag value typed as text, whatever the type. A text tag suggests the
 * values releases already have, so they're spelled alike.
 */
export function TagValueInput({
  id,
  type,
  tagId,
  value,
  onValueChange,
  ...props
}: Omit<ComponentProps<typeof Input>, "type" | "value" | "list"> & {
  id: string
  type: TagType
  tagId: string
  value: string
  onValueChange: (value: string) => void
}) {
  const suggestions = useTagValues(tagId, value, type !== "text")
  return (
    <>
      <Input
        id={id}
        list={type === "text" ? `${id}-values` : undefined}
        inputMode={type === "number" ? "decimal" : undefined}
        placeholder={TAG_PLACEHOLDERS[type]}
        value={value}
        onChange={(e) => onValueChange(e.target.value)}
        {...props}
      />
      {type === "text" && (
        <datalist id={`${id}-values`}>
          {suggestions.map((suggestion) => (
            <option key={suggestion} value={suggestion} />
          ))}
        </datalist>
      )}
    </>
  )
}
