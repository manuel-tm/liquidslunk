import { useMutation } from "@tanstack/react-query"
import { Link, useParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { DeleteButton } from "@/components/delete-button"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Skeleton } from "@/components/ui/skeleton"
import type { ListKind } from "@/bindings"
import { call, idParam, queryClient, useCommand } from "@/lib/query"
import { LISTS } from "@/routes/lists/config"
import { NotFound } from "@/routes/not-found"

/** A format or medium, and how many releases use it. */
export function ListItemRead({ kind }: { kind: ListKind }) {
  const { id = "" } = useParams()
  const itemId = idParam(id)
  const { path, title, one, releaseFilter } = LISTS[kind]
  const { data: item, error } = useCommand(
    "listItem",
    { kind, id: itemId ?? 0 },
    { enabled: itemId !== null }
  )
  const deleteItem = useMutation({
    mutationFn: (id: number) => call("deleteListItem", { kind, id }),
  })

  if (itemId === null || item === null) return <NotFound />

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
              onDelete={() => deleteItem.mutateAsync(item.id)}
              then={path}
              afterDelete={() =>
                queryClient.removeQueries({
                  queryKey: ["listItem", { kind, id: item.id }],
                })
              }
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
