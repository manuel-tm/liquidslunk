import { useMutation } from "@tanstack/react-query"
import { useState } from "react"

import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import type { Tag, TagType } from "@/bindings"
import { call } from "@/lib/query"
import { TAG_TYPE_LABELS, TAG_TYPES } from "@/lib/tags"

export type TagFieldValues = { type: TagType; name: string; multiple: boolean }

/** A tag definition's fields (slunk's design §7.8). The type is set once. */
export function TagFields({
  values,
  onChange,
  typeFixed,
}: {
  values: TagFieldValues
  onChange: (changes: Partial<TagFieldValues>) => void
  /** When editing: the type shows, but can't change */
  typeFixed?: boolean
}) {
  return (
    <>
      <Field>
        <FieldLabel htmlFor="tag-type">Type</FieldLabel>
        {typeFixed ? (
          <p id="tag-type" className="text-xs">
            {TAG_TYPE_LABELS[values.type]}
          </p>
        ) : (
          <>
            <Select
              items={TAG_TYPE_LABELS}
              value={values.type}
              onValueChange={(type) => type && onChange({ type })}
            >
              <SelectTrigger id="tag-type" className="w-40">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {TAG_TYPES.map((type) => (
                  <SelectItem key={type} value={type}>
                    {TAG_TYPE_LABELS[type]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <FieldDescription>
              Text, like Genre or Barcode. A number, like Score or Price. Or a
              date, like Date purchased. It can't change later.
            </FieldDescription>
          </>
        )}
      </Field>
      <Field>
        <FieldLabel htmlFor="tag-name">Name</FieldLabel>
        <Input
          id="tag-name"
          required
          value={values.name}
          onChange={(e) => onChange({ name: e.target.value })}
        />
      </Field>
      <Field orientation="horizontal">
        <Switch
          id="tag-multiple"
          checked={values.multiple}
          onCheckedChange={(multiple) => onChange({ multiple })}
        />
        <FieldContent>
          <FieldLabel htmlFor="tag-multiple">
            Several values per release
          </FieldLabel>
          <FieldDescription>
            Like Genre: "Pop, Rock". Otherwise a release has at most one value,
            like Score.
          </FieldDescription>
        </FieldContent>
      </Field>
    </>
  )
}

/**
 * Creates a tag from a picker, where a name alone isn't enough: it asks for
 * the type, and single or multiple. Closing it creates nothing.
 */
export function NewTagDialog({
  name,
  onDone,
}: {
  name: string
  /** The new tag, or null when cancelled */
  onDone: (tag: Tag | null) => void
}) {
  const [values, setValues] = useState<TagFieldValues>({
    type: "text",
    name,
    multiple: false,
  })
  const create = useMutation({
    mutationFn: () => call("createTag", values),
  })

  return (
    <Dialog open onOpenChange={(open) => !open && onDone(null)}>
      <DialogContent>
        <form
          className="flex flex-col gap-4"
          onSubmit={async (event) => {
            event.preventDefault()
            // The dialog is portaled, but React still bubbles the submit to
            // the form that opened it
            event.stopPropagation()
            try {
              onDone(await create.mutateAsync())
            } catch {
              // The dialog shows the error
            }
          }}
        >
          <DialogHeader>
            <DialogTitle>New tag</DialogTitle>
            <DialogDescription>
              Tags are defined once, then set on any release.
            </DialogDescription>
          </DialogHeader>
          <FieldGroup>
            {create.error && (
              <QueryError error={create.error} title="Can't create the tag" />
            )}
            <TagFields
              values={values}
              onChange={(changes) => setValues((v) => ({ ...v, ...changes }))}
            />
          </FieldGroup>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onDone(null)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={create.isPending}>
              Create tag
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
