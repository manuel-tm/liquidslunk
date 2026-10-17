import { useMutation, useQuery } from "@apollo/client/react"
import { Link, useParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { DeleteButton } from "@/components/delete-button"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Skeleton } from "@/components/ui/skeleton"
import { graphql } from "@/gql"
import type { ListKind } from "@/gql/graphql"
import { LISTS } from "@/routes/lists/config"
import { NotFound } from "@/routes/not-found"

const ListItemQuery = graphql(`
  query ListItem($kind: ListKind!, $id: ID!) {
    listItem(kind: $kind, id: $id) {
      id
      name
      uses
      blockers
    }
  }
`)

const DeleteListItem = graphql(`
  mutation DeleteListItem($kind: ListKind!, $id: ID!) {
    deleteListItem(kind: $kind, id: $id)
  }
`)

/** A format or medium, and how many releases use it (slunk's design §7.8). */
export function ListItemRead({ kind }: { kind: ListKind }) {
  const { id = "" } = useParams()
  const { path, title, one, releaseFilter } = LISTS[kind]
  const { data, error } = useQuery(ListItemQuery, { variables: { kind, id } })
  const [deleteItem] = useMutation(DeleteListItem, { variables: { kind, id } })

  if (data && !data.listItem) return <NotFound />
  const item = data?.listItem

  return (
    <>
      <PageHeader
        crumbs={[
          { label: title, to: path },
          { label: item ? `${item.name} (${item.id})` : id },
        ]}
      >
        {item && (
          <>
            <ButtonLink variant="outline" to={`${path}/${id}/update`}>
              Edit
            </ButtonLink>
            <DeleteButton
              name={`the ${one} ${item.name}`}
              blockers={item.blockers}
              onDelete={() => deleteItem()}
              then={path}
            />
          </>
        )}
      </PageHeader>
      <div className="flex flex-col gap-2 p-4 text-xs">
        {error ? (
          <QueryError error={error} title={`Can't load the ${one}`} />
        ) : !item ? (
          <Skeleton className="h-16 w-64" />
        ) : (
          <>
            <h1 className="font-heading text-2xl font-semibold">{item.name}</h1>
            <p className="text-muted-foreground">
              Used by {item.uses} {item.uses === 1 ? "release" : "releases"}
              {item.uses > 0 && (
                <>
                  {" · "}
                  <Link
                    to={`/releases?${releaseFilter}=${item.id}`}
                    className="text-foreground hover:underline"
                  >
                    See them
                  </Link>
                </>
              )}
            </p>
          </>
        )}
      </div>
    </>
  )
}
