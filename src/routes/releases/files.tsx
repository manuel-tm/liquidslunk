import { useParams } from "react-router"

import { FileGallery } from "@/components/file-gallery"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Skeleton } from "@/components/ui/skeleton"
import { idParam, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

/** `/releases/:id/files`: the cover, then the linked files (slunk's design §7.6). */
export function ReleaseFiles() {
  const { id = "" } = useParams()
  const releaseId = idParam(id)
  const { data: release, error } = useCommand(
    "releaseFiles",
    { id: releaseId ?? 0 },
    { enabled: releaseId !== null }
  )

  if (releaseId === null || release === null) return <NotFound />

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
