import { useMutation } from "@tanstack/react-query"
import { useState } from "react"
import { toast } from "sonner"

import { AddToExternalLibrary } from "@/components/external-libraries"
import { type TagOption, TagPicker } from "@/components/pickers"
import { QueryError } from "@/components/query-error"
import { TagValueInput } from "@/components/tag-value-input"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { call } from "@/lib/query"

const releases = (n: number) => `${n} release${n === 1 ? "" : "s"}`

/**
 * The release list's bulk edit: what's selected, and what can be done to it all
 * at once.
 */
export function BulkEditBar({
  selected,
  total,
  selectingAll,
  onSelectAll,
  onClear,
}: {
  selected: number[]
  /** How many releases match the list's filters */
  total: number
  selectingAll: boolean
  onSelectAll: () => void
  onClear: () => void
}) {
  return (
    <div className="flex flex-wrap items-center gap-2 border bg-muted/50 px-3 py-2 text-xs">
      <span className="font-medium">{releases(selected.length)} selected</span>
      {selected.length < total && (
        <Button
          variant="link"
          size="sm"
          disabled={selectingAll}
          onClick={onSelectAll}
        >
          Select all {total} that match
        </Button>
      )}
      <Button variant="ghost" size="sm" onClick={onClear}>
        Clear
      </Button>
      <div className="ml-auto flex gap-2">
        <BulkTagDialog mode="set" ids={selected} />
        <BulkTagDialog mode="remove" ids={selected} />
        <AddToExternalLibrary
          releases={selected}
          what={releases(selected.length)}
          size="sm"
        />
      </div>
    </div>
  )
}

/** Sets a tag on the selected releases, or removes one from them. */
function BulkTagDialog({
  mode,
  ids,
}: {
  mode: "set" | "remove"
  ids: number[]
}) {
  const [open, setOpen] = useState(false)
  const [tag, setTag] = useState<TagOption | null>(null)
  const [value, setValue] = useState("")
  const setMode = mode === "set"
  // The list shows what changed once it's fetched again, as every query
  // does after a mutation
  const change = useMutation({
    mutationFn: (tag: TagOption) => {
      const args = { ids, type: tag.type, tagId: Number(tag.tagId) }
      return setMode
        ? call("setTagOnReleases", { ...args, value })
        : call("removeTagFromReleases", { ...args, value: value || null })
    },
  })

  function close() {
    setOpen(false)
    setTag(null)
    setValue("")
    change.reset()
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => (next ? setOpen(true) : close())}
    >
      <DialogTrigger render={<Button variant="outline" size="sm" />}>
        {setMode ? "Set a tag" : "Remove a tag"}
      </DialogTrigger>
      <DialogContent>
        <form
          className="flex flex-col gap-4"
          onSubmit={async (event) => {
            event.preventDefault()
            if (!tag) return
            try {
              const changed = await change.mutateAsync(tag)
              toast.success(
                changed === 0
                  ? "Nothing to change"
                  : `${setMode ? "Set" : "Removed"} ${tag.name} ${setMode ? "on" : "from"} ${releases(changed)}`
              )
              close()
            } catch {
              // The dialog shows the error
            }
          }}
        >
          <DialogHeader>
            <DialogTitle>
              {setMode
                ? `Set a tag on ${releases(ids.length)}`
                : `Remove a tag from ${releases(ids.length)}`}
            </DialogTitle>
            <DialogDescription>
              {setMode
                ? "A single-value tag gets this value instead of its old one. A multi-value tag gets it added."
                : "Removes one value, or every value of the tag when the value is left empty."}
            </DialogDescription>
          </DialogHeader>
          <FieldGroup>
            {change.error && (
              <QueryError error={change.error} title="Can't change them" />
            )}
            <Field>
              <FieldLabel htmlFor={`bulk-${mode}-tag`}>Tag</FieldLabel>
              <TagPicker
                id={`bulk-${mode}-tag`}
                creatable={setMode}
                value={tag}
                onChange={setTag}
              />
            </Field>
            {tag && (
              <Field>
                <FieldLabel htmlFor={`bulk-${mode}-value`}>Value</FieldLabel>
                <TagValueInput
                  id={`bulk-${mode}-value`}
                  type={tag.type}
                  tagId={tag.tagId}
                  value={value}
                  required={setMode}
                  onValueChange={setValue}
                />
                {!setMode && (
                  <FieldDescription>
                    Empty removes every value.
                  </FieldDescription>
                )}
              </Field>
            )}
          </FieldGroup>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              Cancel
            </Button>
            <Button
              type="submit"
              variant={setMode ? "default" : "destructive"}
              disabled={change.isPending || !tag || (setMode && !value.trim())}
            >
              {setMode ? "Set" : "Remove"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
