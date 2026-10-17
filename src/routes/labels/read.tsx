import { useMutation, useQuery } from "@apollo/client/react"
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
import { graphql } from "@/gql"
import { useListParams } from "@/hooks/use-list-params"
import { textProps } from "@/lib/languages"
import { NotFound } from "@/routes/not-found"

const LabelQuery = graphql(`
  query Label($id: ID!, $page: Int!, $perPage: Int!, $sort: String) {
    label(id: $id) {
      id
      name
      nameLanguage
      alternateNames {
        name
        language
      }
      picture {
        id
      }
      blockers
    }
    releases(label: $id, page: $page, perPage: $perPage, sort: $sort) {
      total
      items {
        id
        title
        titleLanguage
        alternateTitles {
          title
          language
          printed
        }
        released
        catalogNumber
        cover {
          id
        }
        artists {
          artist {
            id
            name
            nameLanguage
          }
          creditedAs {
            name
            language
          }
        }
        formats {
          id
          name
        }
      }
    }
  }
`)

const SetLabelPicture = graphql(`
  mutation SetLabelPicture($id: ID!, $picture: FileRefInput!) {
    setLabelPicture(id: $id, picture: $picture) {
      id
      picture {
        id
      }
    }
  }
`)

const DeleteLabel = graphql(`
  mutation DeleteLabel($id: ID!) {
    deleteLabel(id: $id)
  }
`)

/** `/labels/:id`: the logo and name, then the label's releases (slunk's design §7.8). */
export function LabelRead() {
  const { id = "" } = useParams()
  const list = useListParams()
  const { data, error } = useQuery(LabelQuery, {
    variables: {
      id,
      page: list.page,
      perPage: PER_PAGE,
      sort: list.sort ?? "released",
    },
  })
  const [deleteLabel] = useMutation(DeleteLabel, { variables: { id } })
  const [setPicture] = useMutation(SetLabelPicture)

  if (data && !data.label) return <NotFound />
  const label = data?.label

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
              onDelete={() => deleteLabel()}
              then="/labels"
            />
          </>
        )}
      </PageHeader>
      <div className="flex flex-col gap-4 p-4">
        {error ? (
          <QueryError error={error} title="Can't load the label" />
        ) : !label || !data ? (
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
                  setPicture({ variables: { id, picture } })
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
                total={data.releases.total}
                onPage={(page) => list.update({ page })}
              />
            </div>
            <ReleaseTable
              releases={data.releases.items}
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
