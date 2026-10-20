import { useMutation } from "@tanstack/react-query"
import { useState } from "react"
import { useNavigate, useParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import type { ListKind } from "@/bindings"
import { call, idParam, useCommand } from "@/lib/query"
import { LISTS } from "@/routes/lists/config"
import { NotFound } from "@/routes/not-found"

/** Creates (no `:id`) or renames a format or medium. */
export function ListItemForm({ kind }: { kind: ListKind }) {
  const { id } = useParams()
  const itemId = idParam(id)
  const { path, title } = LISTS[kind]
  // A form starts from fresh data (frontend.md, "Commands")
  const { data: item, error } = useCommand(
    "listItem",
    { kind, id: itemId ?? 0 },
    { fresh: true, enabled: itemId !== null }
  )

  if ((id !== undefined && itemId === null) || item === null) {
    return <NotFound />
  }

  return (
    <>
      <PageHeader
        crumbs={[
          { label: title, to: path },
          ...(id
            ? [
                {
                  label: item ? `${item.name} (${item.id})` : id,
                  to: `${path}/${id}`,
                },
                { label: "Update" },
              ]
            : [{ label: "New" }]),
        ]}
      />
      <div className="max-w-md p-4">
        {error ? (
          <QueryError error={error} title="Can't load it" />
        ) : id && !item ? (
          <Skeleton className="h-24 w-full" />
        ) : (
          // Keyed, so a form for another row, or a new one, starts afresh
          <NameForm
            key={item?.id ?? "new"}
            kind={kind}
            id={item?.id}
            initial={item?.name ?? ""}
          />
        )}
      </div>
    </>
  )
}

function NameForm({
  kind,
  id,
  initial,
}: {
  kind: ListKind
  id?: number
  initial: string
}) {
  const navigate = useNavigate()
  const { path } = LISTS[kind]
  const [name, setName] = useState(initial)
  const save = useMutation({
    mutationFn: () =>
      id === undefined
        ? call("createListItem", { kind, name })
        : call("renameListItem", { kind, id, name }),
  })

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          const saved = await save.mutateAsync()
          navigate(`${path}/${saved.id}`)
        } catch {
          // The form shows the error
        }
      }}
    >
      <FieldGroup>
        {save.error && <QueryError error={save.error} title="Can't save" />}
        <Field>
          <FieldLabel htmlFor="name">Name</FieldLabel>
          <Input
            id="name"
            value={name}
            required
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        <div className="flex gap-2">
          <Button type="submit" disabled={save.isPending}>
            Save
          </Button>
          <ButtonLink variant="outline" to={id ? `${path}/${id}` : path}>
            Cancel
          </ButtonLink>
        </div>
      </FieldGroup>
    </form>
  )
}
