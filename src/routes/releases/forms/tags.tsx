import { useMutation } from "@tanstack/react-query"
import { useState } from "react"

import { QueryError } from "@/components/query-error"
import {
  type TagRow,
  TagsEditor,
  tagRows,
  tagValuesInput,
} from "@/components/tags-editor"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { call, idParam, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

/** A release's Tags tab, which the import's last step reuses. */
export function ReleaseTagsForm({
  releaseId,
  submitLabel = "Save",
  onSaved,
}: {
  /** From the URL */
  releaseId: string
  submitLabel?: string
  onSaved: () => void
}) {
  const id = idParam(releaseId)
  // A form starts from fresh data (design §7.2)
  const { data: tags, error } = useCommand(
    "releaseTags",
    { id: id ?? 0 },
    { fresh: true, enabled: id !== null }
  )

  if (id === null || tags === null) return <NotFound />
  if (error) return <QueryError error={error} title="Can't load the tags" />
  if (!tags) return <Skeleton className="h-64 w-full max-w-2xl" />
  return (
    <Form
      key={id}
      releaseId={id}
      initial={tagRows(tags)}
      submitLabel={submitLabel}
      onSaved={onSaved}
    />
  )
}

function Form({
  releaseId,
  initial,
  submitLabel,
  onSaved,
}: {
  releaseId: number
  initial: TagRow[]
  submitLabel: string
  onSaved: () => void
}) {
  const [rows, setRows] = useState(initial)
  const save = useMutation({
    mutationFn: () =>
      call("setReleaseTags", { id: releaseId, tags: tagValuesInput(rows) }),
  })

  return (
    <form
      className="flex max-w-2xl flex-col gap-6"
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          await save.mutateAsync()
          onSaved()
        } catch {
          // The form shows the error
        }
      }}
    >
      {save.error && (
        <QueryError error={save.error} title="Can't save the tags" />
      )}
      <TagsEditor rows={rows} onChange={setRows} />
      <div>
        <Button type="submit" disabled={save.isPending}>
          {submitLabel}
        </Button>
      </div>
    </form>
  )
}
