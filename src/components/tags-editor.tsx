import { XIcon } from "lucide-react"
import { useState } from "react"

import { ChipList } from "@/components/chip-list"
import type { PickerProps } from "@/components/picker"
import {
  type TagOption,
  TagPicker,
  TagValuePicker,
  tagOption,
} from "@/components/pickers"
import { TagValueInput } from "@/components/tag-value-input"
import { Button } from "@/components/ui/button"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import type { TagType, TagValuesInput } from "@/gql/graphql"
import { TAG_PLACEHOLDERS, TAG_TYPE_LABELS, tagValueKey } from "@/lib/tags"

/** A tag being edited, with its values as typed. */
export type TagRow = { tag: TagOption; values: string[] }

/** Rows from a release's or track's `tags`. */
export function tagRows(
  tags: {
    tag: { id: string; type: TagType; name: string; multiple: boolean }
    values: string[]
  }[]
): TagRow[] {
  return tags.map(({ tag, values }) => ({ tag: tagOption(tag), values }))
}

/** Rows as the API takes them. */
export function tagValuesInput(rows: TagRow[]): TagValuesInput[] {
  return rows.map(({ tag, values }) => ({
    type: tag.type,
    tagId: tag.tagId,
    values,
  }))
}

/**
 * The tags set on a release or track, and a picker to add one (design
 * §7.6). A single-value tag is one input, and a multi-value tag is a list.
 */
export function TagsEditor({
  rows,
  onChange,
}: {
  rows: TagRow[]
  onChange: (rows: TagRow[]) => void
}) {
  // A new key clears the picker after each pick
  const [pickerKey, setPickerKey] = useState(0)
  const setValues = (index: number, values: string[]) =>
    onChange(rows.map((row, i) => (i === index ? { ...row, values } : row)))

  return (
    <div className="flex flex-col gap-6">
      {rows.length === 0 ? (
        <p className="text-xs text-muted-foreground">No tags yet.</p>
      ) : (
        <div className="flex flex-col gap-3">
          {rows.map((row, index) => {
            const id = `tag-${row.tag.type}-${row.tag.tagId}`
            return (
              <div
                key={row.tag.id}
                className="grid grid-cols-[10rem_1fr_auto] items-start gap-3"
              >
                <div className="flex flex-col pt-1.5">
                  <Label htmlFor={id}>{row.tag.name}</Label>
                  <span className="text-xs text-muted-foreground">
                    {TAG_TYPE_LABELS[row.tag.type]}
                    {row.tag.multiple && ", several values"}
                  </span>
                </div>
                {row.tag.multiple ? (
                  <ChipList
                    items={row.values.map((value) => ({
                      id: tagValueKey(value),
                      name: value,
                    }))}
                    onChange={(items) =>
                      setValues(
                        index,
                        items.map((item) => item.name)
                      )
                    }
                    picker={(props) =>
                      row.tag.type === "text" ? (
                        <TagValuePicker
                          {...props}
                          id={id}
                          tagId={row.tag.tagId}
                        />
                      ) : (
                        <ValueAdder {...props} id={id} type={row.tag.type} />
                      )
                    }
                  />
                ) : (
                  <div>
                    <TagValueInput
                      id={id}
                      type={row.tag.type}
                      tagId={row.tag.tagId}
                      value={row.values[0] ?? ""}
                      onValueChange={(value) => setValues(index, [value])}
                    />
                  </div>
                )}
                <Button
                  type="button"
                  size="icon-xs"
                  variant="ghost"
                  className="mt-1"
                  aria-label={`Remove ${row.tag.name}`}
                  onClick={() => onChange(rows.filter((_, i) => i !== index))}
                >
                  <XIcon />
                </Button>
              </div>
            )
          })}
        </div>
      )}
      <Field className="max-w-xs">
        <FieldLabel htmlFor="add-tag">Add a tag</FieldLabel>
        <div key={pickerKey}>
          <TagPicker
            id="add-tag"
            creatable
            value={null}
            exclude={rows.map((row) => row.tag.id)}
            onChange={(tag) => {
              if (!tag) return
              onChange([...rows, { tag, values: [] }])
              setPickerKey((key) => key + 1)
            }}
          />
        </div>
      </Field>
    </div>
  )
}

/** Adds a number or date to a multi-value tag's list. */
function ValueAdder({ id, type, onChange }: PickerProps & { type: TagType }) {
  const [text, setText] = useState("")
  const add = () => {
    const value = text.trim()
    if (value) onChange({ id: tagValueKey(value), name: value })
  }
  return (
    <div className="flex gap-1">
      <Input
        id={id}
        inputMode={type === "number" ? "decimal" : undefined}
        placeholder={TAG_PLACEHOLDERS[type]}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          // Enter adds the value instead of saving the form
          if (e.key === "Enter") {
            e.preventDefault()
            add()
          }
        }}
      />
      <Button type="button" variant="outline" onClick={add}>
        Add
      </Button>
    </div>
  )
}
