import { useQuery } from "@apollo/client/react"
import { useParams } from "react-router"

import { FileGallery } from "@/components/file-gallery"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Skeleton } from "@/components/ui/skeleton"
import { graphql } from "@/gql"
import { NotFound } from "@/routes/not-found"

const ReleaseFilesQuery = graphql(`
  query ReleaseFiles($id: ID!) {
    release(id: $id) {
      id
      title
      cover {
        id
        name
      }
      files {
        note
        file {
          id
          name
          type
        }
      }
    }
  }
`)

/** `/releases/:id/files`: the cover, then the linked files (slunk's design §7.6). */
export function ReleaseFiles() {
  const { id = "" } = useParams()
  const { data, error } = useQuery(ReleaseFilesQuery, { variables: { id } })

  if (data && !data.release) return <NotFound />
  const release = data?.release

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Releases", to: "/releases" },
          {
            label: release ? `${release.title} (${id})` : id,
            to: `/releases/${id}`,
          },
          { label: "Files" },
        ]}
      />
      <div className="p-4">
        {error ? (
          <QueryError error={error} title="Can't load the files" />
        ) : !release ? (
          <Skeleton className="h-64 w-full" />
        ) : (
          <FileGallery
            primary={release.cover}
            primaryLabel="Cover"
            links={release.files}
          />
        )}
      </div>
    </>
  )
}
