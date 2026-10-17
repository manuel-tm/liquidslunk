import { useQuery } from "@apollo/client/react"
import { useNavigate, useParams } from "react-router"
import { toast } from "sonner"

import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { TabLinks } from "@/components/tab-links"
import { Skeleton } from "@/components/ui/skeleton"
import { alternateTitleRow } from "@/components/alternate-titles-editor"
import { graphql } from "@/gql"
import { creditedOption } from "@/lib/credited"
import { NotFound } from "@/routes/not-found"
import {
  EMPTY_DETAILS,
  ReleaseDetailsForm,
} from "@/routes/releases/forms/details"
import { ReleaseCreditsForm } from "@/routes/releases/forms/credits"
import { ReleaseFilesForm } from "@/routes/releases/forms/files"
import { ReleaseTagsForm } from "@/routes/releases/forms/tags"
import { ReleaseTracksForm } from "@/routes/releases/forms/tracks"

const ReleaseTitleQuery = graphql(`
  query ReleaseTitle($id: ID!) {
    release(id: $id) {
      id
      title
    }
  }
`)

const ReleaseDetailsQuery = graphql(`
  query ReleaseDetails($id: ID!) {
    release(id: $id) {
      id
      title
      titleLanguage
      alternateTitles {
        title
        language
        printed
      }
      released
      variousArtists
      catalogNumber
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
      labels {
        id
        name
      }
      formats {
        id
        name
      }
      media {
        id
        name
      }
    }
  }
`)

export type EditTab = "details" | "tracks" | "files" | "credits" | "tags"

/**
 * `/releases/:id/update/…` (slunk's design §7.6): editing split into tabs in the
 * header, each a route that saves on its own and then goes to the release.
 */
export function ReleaseEdit({ tab }: { tab: EditTab }) {
  const { id = "" } = useParams()
  const navigate = useNavigate()
  // The tab's own query loads the title; this reads it from the cache
  const { data } = useQuery(ReleaseTitleQuery, {
    variables: { id },
    fetchPolicy: "cache-only",
  })
  const base = `/releases/${id}/update`
  const saved = () => {
    toast.success("Saved")
    navigate(`/releases/${id}`)
  }

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Releases", to: "/releases" },
          {
            label: data?.release ? `${data.release.title} (${id})` : id,
            to: `/releases/${id}`,
          },
          { label: "Update" },
        ]}
      >
        <TabLinks
          tabs={[
            { label: "Details", to: base },
            { label: "Tracks", to: `${base}/tracks` },
            { label: "Files", to: `${base}/files` },
            { label: "Credits", to: `${base}/credits` },
            { label: "Tags", to: `${base}/tags` },
          ]}
        />
      </PageHeader>
      <div className="p-4">
        {tab === "details" && <DetailsTab id={id} onSaved={saved} />}
        {tab === "tracks" && (
          <ReleaseTracksForm releaseId={id} onSaved={saved} />
        )}
        {tab === "files" && <ReleaseFilesForm releaseId={id} onSaved={saved} />}
        {tab === "credits" && (
          <ReleaseCreditsForm releaseId={id} onSaved={saved} />
        )}
        {tab === "tags" && <ReleaseTagsForm releaseId={id} onSaved={saved} />}
      </div>
    </>
  )
}

export function DetailsTab({
  id,
  submitLabel,
  offerSplit,
  onSaved,
}: {
  id: string
  submitLabel?: string
  /** The import's offer to split a title with " = " */
  offerSplit?: boolean
  onSaved: () => void
}) {
  const { data, error } = useQuery(ReleaseDetailsQuery, {
    variables: { id },
    fetchPolicy: "network-only",
  })
  if (error) return <QueryError error={error} title="Can't load the release" />
  if (!data) return <Skeleton className="h-96 w-full max-w-2xl" />
  if (!data.release) return <NotFound />
  const release = data.release
  return (
    <ReleaseDetailsForm
      releaseId={id}
      submitLabel={submitLabel}
      offerSplit={offerSplit}
      initial={{
        title: release.title,
        titleLanguage: release.titleLanguage,
        alternateTitles: release.alternateTitles.map(alternateTitleRow),
        released: release.released ?? "",
        variousArtists: release.variousArtists,
        catalogNumber: release.catalogNumber ?? "",
        artists: release.artists.map(creditedOption),
        labels: release.labels,
        formats: release.formats,
        media: release.media,
      }}
      onSaved={onSaved}
    />
  )
}

/** `/releases/create`: only the details. The edit tabs add the rest. */
export function ReleaseCreate() {
  const navigate = useNavigate()
  return (
    <>
      <PageHeader
        crumbs={[{ label: "Releases", to: "/releases" }, { label: "New" }]}
      />
      <div className="p-4">
        <ReleaseDetailsForm
          initial={EMPTY_DETAILS}
          onSaved={(id) => navigate(`/releases/${id}`)}
        />
      </div>
    </>
  )
}
