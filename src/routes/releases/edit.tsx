import { useNavigate, useParams } from "react-router"
import { toast } from "sonner"

import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { TabLinks } from "@/components/tab-links"
import { Skeleton } from "@/components/ui/skeleton"
import { alternateTitleRow } from "@/components/alternate-titles-editor"
import { creditedOption } from "@/lib/credited"
import { idParam, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"
import {
  EMPTY_DETAILS,
  ReleaseDetailsForm,
} from "@/routes/releases/forms/details"
import { ReleaseCreditsForm } from "@/routes/releases/forms/credits"
import { ReleaseFilesForm } from "@/routes/releases/forms/files"
import { ReleaseTagsForm } from "@/routes/releases/forms/tags"
import { ReleaseTracksForm } from "@/routes/releases/forms/tracks"

export type EditTab = "details" | "tracks" | "files" | "credits" | "tags"

/**
 * `/releases/:id/update/…`: editing split into tabs in the header, each a route
 * that saves on its own and then goes to the release.
 */
export function ReleaseEdit({ tab }: { tab: EditTab }) {
  const { id = "" } = useParams()
  const navigate = useNavigate()
  const releaseId = idParam(id)
  const { data: header } = useCommand(
    "releaseHeader",
    { id: releaseId ?? 0 },
    { enabled: releaseId !== null }
  )
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
            label: header ? `${header.title} (${id})` : id,
            lang: header?.titleLanguage,
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
  const releaseId = idParam(id)
  // A form starts from fresh data (design §7.2)
  const { data: release, error } = useCommand(
    "release",
    { id: releaseId ?? 0 },
    { fresh: true, enabled: releaseId !== null }
  )
  if (releaseId === null || release === null) return <NotFound />
  if (error) return <QueryError error={error} title="Can't load the release" />
  if (!release) return <Skeleton className="h-96 w-full max-w-2xl" />
  return (
    <ReleaseDetailsForm
      key={release.id}
      releaseId={release.id}
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
        // Options' ids are strings, as the pickers' are
        labels: release.labels.map(option),
        formats: release.formats.map(option),
        media: release.media.map(option),
      }}
      onSaved={onSaved}
    />
  )
}

const option = ({ id, name }: { id: number; name: string }) => ({
  id: String(id),
  name,
})

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
