import { useMutation, useQuery } from "@apollo/client/react"
import { useState } from "react"
import { useNavigate, useParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { graphql } from "@/gql"
import type { ListKind } from "@/gql/graphql"
import { LISTS } from "@/routes/lists/config"
import { NotFound } from "@/routes/not-found"

const ListItemNameQuery = graphql(`
  query ListItemName($kind: ListKind!, $id: ID!) {
    listItem(kind: $kind, id: $id) {
      id
      name
    }
  }
`)

const CreateListItem = graphql(`
  mutation CreateListItem($kind: ListKind!, $name: String!) {
    createListItem(kind: $kind, name: $name) {
      id
      name
    }
  }
`)

const RenameListItem = graphql(`
  mutation RenameListItem($kind: ListKind!, $id: ID!, $name: String!) {
    renameListItem(kind: $kind, id: $id, name: $name) {
      id
      name
    }
  }
`)

/** Creates (no `:id`) or renames a format or medium. */
export function ListItemForm({ kind }: { kind: ListKind }) {
  const { id } = useParams()
  const { path, title } = LISTS[kind]
  const { data, error } = useQuery(ListItemNameQuery, {
    variables: { kind, id: id ?? "" },
    skip: id === undefined,
  })

  if (data && !data.listItem) return <NotFound />
  const item = data?.listItem

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
          <NameForm kind={kind} id={id} initial={item?.name ?? ""} />
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
  id?: string
  initial: string
}) {
  const navigate = useNavigate()
  const { path } = LISTS[kind]
  const [name, setName] = useState(initial)
  const [create, created] = useMutation(CreateListItem)
  const [rename, renamed] = useMutation(RenameListItem)
  const error = created.error ?? renamed.error

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          const saved = id
            ? (await rename({ variables: { kind, id, name } })).data!
                .renameListItem
            : (await create({ variables: { kind, name } })).data!.createListItem
          navigate(`${path}/${saved.id}`)
        } catch {
          // The form shows the error
        }
      }}
    >
      <FieldGroup>
        {error && <QueryError error={error} title="Can't save" />}
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
          <Button type="submit" disabled={created.loading || renamed.loading}>
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
