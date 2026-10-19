import { useMutation } from "@tanstack/react-query"
import { useParams } from "react-router"

import { AlternateNames } from "@/components/alternate-names"
import { ButtonLink } from "@/components/button-link"
import { CoverChooser } from "@/components/cover-chooser"
import { DeleteButton } from "@/components/delete-button"
import { PER_PAGE } from "@/components/list/list-layout"
import { Pager } from "@/components/list/pager"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { ReleaseTable } from "@/components/release-table"
import { Skeleton } from "@/components/ui/skeleton"
import type { FileRefInput } from "@/bindings"
import { useListParams } from "@/hooks/use-list-params"
import { textProps } from "@/lib/languages"
import { call, idParam, queryClient, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

/** `/labels/:id`: the logo and name, then the label's releases. */
export function LabelRead() {
  const { id = "" } = useParams()
  const labelId = idParam(id)
  const list = useListParams()
  const { data: label, error } = useCommand(
    "labelPage",
    {
      id: labelId ?? 0,
      page: list.page,
      perPage: PER_PAGE,
      sort: list.sort ?? "released",
    },
    { enabled: labelId !== null }
  )
  const deleteLabel = useMutation({
    mutationFn: (labelId: number) => call("deleteLabel", { id: labelId }),
  })
  const setPicture = useMutation({
    mutationFn: ({ id, picture }: { id: number; picture: FileRefInput }) =>
      call("setLabelPicture", { id, picture }),
  })

  if (labelId === null || label === null) return <NotFound />

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Labels", to: "/labels" },
          {
            label: label ? `${label.name} (${label.id})` : id,
            lang: label?.nameLanguage,
          },
        ]}
      >
        {label && (
          <>
            <ButtonLink variant="outline" to={`/labels/${id}/update`}>
              Edit
            </ButtonLink>
            <DeleteButton
              name={label.name}
              blockers={label.blockers}
              onDelete={() => deleteLabel.mutateAsync(label.id)}
              then="/labels"
              afterDelete={() => {
                for (const name of ["label", "labelPage"]) {
                  queryClient.removeQueries({
                    queryKey: [name, { id: label.id }],
                  })
                }
              }}
            />
          </>
        )}
      </PageHeader>
      <div className="flex flex-col gap-4 p-4">
        {error ? (
          <QueryError error={error} title="Can't load the label" />
        ) : !label ? (
          <Skeleton className="h-64 w-full" />
        ) : (
          <>
            <div className="flex items-center gap-4">
              <CoverChooser
                file={label.picture}
                alt={`${label.name} logo`}
                className="size-24"
                noun="logo"
                onChoose={(picture) =>
                  setPicture.mutateAsync({ id: label.id, picture })
                }
              />
              <div className="flex flex-col gap-2">
                <h1
                  className="font-heading text-2xl font-semibold"
                  {...textProps(label.nameLanguage)}
                >
                  {label.name}
                </h1>
                <AlternateNames names={label.alternateNames} />
              </div>
            </div>
            <div className="flex justify-end">
              <Pager
                page={list.page}
                perPage={PER_PAGE}
                total={label.releases.total}
                onPage={(page) => list.update({ page })}
              />
            </div>
            <ReleaseTable
              releases={label.releases.items}
              columns={[
                "cover",
                "title",
                "formats",
                "catalogNumber",
                "released",
              ]}
              withArtists
              sort={list.sort}
              onSort={(sort) => list.update({ sort })}
            />
          </>
        )}
      </div>
    </>
  )
}
