import { useMutation } from "@tanstack/react-query"
import { CheckIcon, CircleAlertIcon } from "lucide-react"
import { type ReactNode, useState } from "react"
import { Link, useNavigate, useParams, useSearchParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { FolderBrowser } from "@/components/folder-browser"
import { OtherNamesOffer } from "@/components/other-names-offer"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import type { FileRefInput, ImportFiles } from "@/bindings"
import { shownName } from "@/lib/credited"
import { formatSize } from "@/lib/format"
import { offeredNames, pickedArtists, unpicked } from "@/lib/import"
import { call, idParam, useCommand } from "@/lib/query"
import { cn } from "@/lib/utils"
import { NotFound } from "@/routes/not-found"
import { DetailsTab } from "@/routes/releases/edit"
import { ReleaseCreditsForm } from "@/routes/releases/forms/credits"
import {
  EMPTY_DETAILS,
  ReleaseDetailsForm,
} from "@/routes/releases/forms/details"
import { ReleaseTagsForm } from "@/routes/releases/forms/tags"
import { ReleaseTracksForm } from "@/routes/releases/forms/tracks"

const STEPS = [
  "Folder",
  "Release",
  "Tracks",
  "Cover & files",
  "Credits",
  "Tags",
] as const
type Step = (typeof STEPS)[number]

/** The step's route, keeping `?folder=`. */
function stepUrl(
  step: Step,
  id: number | string | undefined,
  folder: string | null
) {
  const query = folder === null ? "" : `?${new URLSearchParams({ folder })}`
  if (step === "Folder")
    return `/import?${new URLSearchParams({ path: folder ?? "" })}`
  if (!id) return `/import/release${query}`
  const path = {
    Release: "release",
    Tracks: "tracks",
    "Cover & files": "files",
    Credits: "credits",
    Tags: "tags",
  }[step]
  return `/import/${id}/${path}${query}`
}

/** The steps at the top. Once the release exists, each one is a link. */
function ImportFrame({
  step,
  id,
  folder,
  children,
}: {
  step: Step
  id?: string
  folder: string | null
  children: ReactNode
}) {
  const current = STEPS.indexOf(step)
  return (
    <>
      <PageHeader
        crumbs={[{ label: "Import", to: "/import" }, { label: step }]}
      />
      <div className="flex flex-col gap-4 p-4">
        <ol className="flex flex-wrap items-center gap-1 text-xs">
          {STEPS.map((name, index) => {
            const reachable =
              name === "Folder" || name === "Release" || id !== undefined
            const label = (
              <span
                className={cn(
                  "inline-flex items-center gap-1.5 border px-2 py-1",
                  index === current && "border-primary font-medium",
                  index !== current && "text-muted-foreground"
                )}
              >
                {index < current ? (
                  <CheckIcon className="size-3" />
                ) : (
                  <span className="tabular-nums">{index + 1}.</span>
                )}
                {name}
              </span>
            )
            return (
              <li key={name}>
                {reachable && index !== current ? (
                  <Link
                    to={stepUrl(name, id, folder)}
                    className="hover:text-foreground"
                  >
                    {label}
                  </Link>
                ) : (
                  label
                )}
              </li>
            )
          })}
        </ol>
        {folder !== null && (
          <p className="text-xs text-muted-foreground">
            Importing{" "}
            <span className="font-mono text-foreground">{folder || "."}</span>
          </p>
        )}
        {children}
      </div>
    </>
  )
}

/** Step 1, `/import?path=…`: browse the library's folders and pick one. */
export function ImportFolder() {
  const [params] = useSearchParams()
  const path = params.get("path") ?? ""
  return (
    <ImportFrame step="Folder" folder={null}>
      <p className="text-xs text-muted-foreground">
        Pick the folder of a release. Folders that already hold registered files
        are marked, so imported ones stand out. Nothing is written to your
        files.
      </p>
      <FolderBrowser
        path={path}
        linkTo={(path) => `/import?${new URLSearchParams({ path })}`}
        actions={
          <ButtonLink to={stepUrl("Release", undefined, path)}>
            Import this folder
          </ButtonLink>
        }
      />
    </ImportFrame>
  )
}

/**
 * Step 2, `/import/release?folder=…`: creates the release, prefilled from
 * the files' tags or the folder's name. Saving replaces this history entry
 * with `/import/:id/release`, so going back edits the release instead.
 */
export function ImportNewRelease() {
  const navigate = useNavigate()
  const [params] = useSearchParams()
  const folder = params.get("folder") ?? ""
  // What's on disk now (design §7.2)
  const { data: suggestion, error } = useCommand(
    "releaseSuggestion",
    { folder },
    { fresh: true }
  )
  const unpickedArtists = unpicked(suggestion?.artists ?? [])
  const plural = (n: number, noun: string) =>
    `${n} ${noun}${n === 1 ? "" : "s"}`

  return (
    <ImportFrame step="Release" folder={folder}>
      {error ? (
        <QueryError error={error} title="Can't read the folder" />
      ) : !suggestion ? (
        <Skeleton className="h-96 w-full max-w-2xl" />
      ) : (
        <>
          <p className="text-xs">
            {plural(suggestion.music, "audio file")},{" "}
            {plural(suggestion.images, "image")},{" "}
            {plural(suggestion.others, "other")}
          </p>
          {suggestion.sameTitle.length > 0 && (
            <Alert className="max-w-2xl">
              <CircleAlertIcon />
              <AlertTitle>A release with this title exists</AlertTitle>
              <AlertDescription>
                Make sure it isn't imported already:{" "}
                {suggestion.sameTitle.map((release, index) => (
                  <span key={release.id}>
                    {index > 0 && ", "}
                    <Link to={`/releases/${release.id}`} className="underline">
                      {release.artists
                        .map((link) => shownName(link).name)
                        .join(", ")}{" "}
                      – {release.title}
                    </Link>
                  </span>
                ))}
              </AlertDescription>
            </Alert>
          )}
          {unpickedArtists.length > 0 && (
            <p className="max-w-2xl text-xs text-muted-foreground">
              The tags name{" "}
              {unpickedArtists.map((a, index) => (
                <span key={a.name}>
                  {index > 0 && ", "}"<span dir="auto">{a.name}</span>"
                  {a.matches > 1
                    ? `, which ${a.matches} artists match`
                    : ", which isn't an artist yet"}
                </span>
              ))}
              . Artists has each one typed in, in turn, to pick or create.
            </p>
          )}
          <OtherNamesOffer offers={offeredNames(suggestion.artists)} />
          <ReleaseDetailsForm
            submitLabel="Create and continue"
            offerSplit
            typedArtists={unpickedArtists.map((a) => a.name)}
            initial={{
              ...EMPTY_DETAILS,
              title: suggestion.title,
              released: suggestion.released ?? "",
              artists: pickedArtists(suggestion.artists),
            }}
            onSaved={(id) => {
              navigate(stepUrl("Release", id, folder), { replace: true })
              navigate(stepUrl("Tracks", id, folder))
            }}
          />
        </>
      )}
    </ImportFrame>
  )
}

/** Step 2 again, once the release exists: edits it. */
export function ImportRelease() {
  const navigate = useNavigate()
  const { id = "" } = useParams()
  const [params] = useSearchParams()
  const folder = params.get("folder")
  return (
    <ImportFrame step="Release" id={id} folder={folder}>
      <DetailsTab
        id={id}
        submitLabel="Save and continue"
        offerSplit
        onSaved={() => navigate(stepUrl("Tracks", id, folder))}
      />
    </ImportFrame>
  )
}

/**
 * Step 3: tracks from the folder's audio files. Next registers the checked
 * files that aren't registered yet, and creates their tracks, in one go.
 */
export function ImportTracks() {
  const navigate = useNavigate()
  const { id = "" } = useParams()
  const [params] = useSearchParams()
  const folder = params.get("folder")
  return (
    <ImportFrame step="Tracks" id={id} folder={folder}>
      <ReleaseTracksForm
        releaseId={id}
        folder={folder ?? undefined}
        submitLabel="Save and continue"
        onSaved={() => navigate(stepUrl("Cover & files", id, folder))}
      />
    </ImportFrame>
  )
}

/**
 * Step 4: the cover is chosen from the folder's images, and each file to
 * link gets a checkbox and a note. Next registers and links them.
 */
export function ImportFiles() {
  const navigate = useNavigate()
  const { id = "" } = useParams()
  const [params] = useSearchParams()
  const folder = params.get("folder")
  const releaseId = idParam(id)
  // What's on disk and in the database now (design §7.2)
  const { data, error } = useCommand(
    "importFiles",
    { id: releaseId ?? 0, folder: folder ?? "" },
    { fresh: true, enabled: releaseId !== null }
  )

  if (releaseId === null || data === null) return <NotFound />
  return (
    <ImportFrame step="Cover & files" id={id} folder={folder}>
      {error ? (
        <QueryError error={error} title="Can't read the folder" />
      ) : !data ? (
        <Skeleton className="h-64 w-full" />
      ) : (
        <FolderFilesForm
          key={data.id}
          release={data}
          onSaved={() => navigate(stepUrl("Credits", id, folder))}
        />
      )}
    </ImportFrame>
  )
}

type FolderFileRow = {
  path: string
  fileId?: number
  type: string
  sizeBytes: number
  linked: boolean
  note: string
}

function FolderFilesForm({
  release,
  onSaved,
}: {
  release: ImportFiles
  onSaved: () => void
}) {
  const linked = new Map(
    release.files.map((link) => [link.file.path, link.note ?? ""])
  )
  const [rows, setRows] = useState<FolderFileRow[]>(() =>
    release.folderFiles
      .filter((file) => file.type !== "music")
      .map((file) => ({
        path: file.path,
        fileId: file.file?.id,
        type: file.type,
        sizeBytes: file.sizeBytes,
        // Files already linked stay checked; the first visit links everything
        linked: release.files.length === 0 || linked.has(file.path),
        note: linked.get(file.path) ?? "",
      }))
  )
  const images = rows.filter((row) => row.type === "image")
  // An image named like a cover is the likely one, else the first image
  const named = images.find((row) =>
    /^(cover|folder|front)\b/i.test(row.path.split("/").pop() ?? "")
  )
  const [cover, setCover] = useState<string | null>(
    release.cover?.path ?? (named ?? images[0])?.path ?? null
  )
  // Links to files outside the folder are kept as they are
  const kept = release.files.filter(
    (link) => !rows.some((row) => row.path === link.file.path)
  )
  const save = useMutation({
    mutationFn: (input: {
      cover: FileRefInput | null
      files: (FileRefInput & { note?: string | null })[]
    }) => call("setReleaseFiles", { id: release.id, ...input }),
  })
  const edit = (path: string, changes: Partial<FolderFileRow>) =>
    setRows((current) =>
      current.map((row) => (row.path === path ? { ...row, ...changes } : row))
    )

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={async (event) => {
        event.preventDefault()
        const outside =
          release.cover && release.cover.path === cover ? release.cover : null
        const coverRow = rows.find((row) => row.path === cover)
        try {
          await save.mutateAsync({
            cover: outside
              ? { fileId: outside.id }
              : coverRow
                ? coverRow.fileId
                  ? { fileId: coverRow.fileId }
                  : { path: coverRow.path }
                : null,
            files: [
              ...kept.map((link) => ({
                fileId: link.file.id,
                note: link.note,
              })),
              ...rows
                // The cover is the primary image, not a file link too
                .filter((row) => row.linked && row.path !== cover)
                .map((row) => ({
                  ...(row.fileId ? { fileId: row.fileId } : { path: row.path }),
                  note: row.note.trim() || null,
                })),
            ],
          })
          onSaved()
        } catch {
          // The form shows the error
        }
      }}
    >
      {save.error && (
        <QueryError error={save.error} title="Can't save the files" />
      )}
      {rows.length === 0 ? (
        <p className="text-xs text-muted-foreground">
          The folder has no images or other files. Continue to the credits.
        </p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-16">Link</TableHead>
              <TableHead className="w-16">Cover</TableHead>
              <TableHead>File</TableHead>
              <TableHead className="text-right">Size</TableHead>
              <TableHead>Note</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((row) => (
              <TableRow key={row.path}>
                <TableCell>
                  <Checkbox
                    aria-label={`Link ${row.path}`}
                    disabled={row.path === cover}
                    checked={row.linked && row.path !== cover}
                    onCheckedChange={(linked) => edit(row.path, { linked })}
                  />
                </TableCell>
                <TableCell>
                  {row.type === "image" && (
                    <input
                      type="radio"
                      name="cover"
                      aria-label={`Use ${row.path} as the cover`}
                      checked={cover === row.path}
                      onChange={() => setCover(row.path)}
                    />
                  )}
                </TableCell>
                <TableCell className="font-mono">{row.path}</TableCell>
                <TableCell className="text-right tabular-nums">
                  {formatSize(row.sizeBytes)}
                </TableCell>
                <TableCell>
                  <Input
                    aria-label={`Note for ${row.path}`}
                    placeholder="Note, like back cover"
                    disabled={!row.linked}
                    value={row.note}
                    onChange={(e) => edit(row.path, { note: e.target.value })}
                  />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      {images.length > 0 && (
        <label className="flex items-center gap-2 text-xs">
          <input
            type="radio"
            name="cover"
            checked={cover === null}
            onChange={() => setCover(null)}
          />
          No cover
        </label>
      )}
      <div>
        <Button type="submit" disabled={save.isPending}>
          Save and continue
        </Button>
      </div>
    </form>
  )
}

/** Step 5: credits. */
export function ImportCredits() {
  const navigate = useNavigate()
  const { id = "" } = useParams()
  const [params] = useSearchParams()
  const folder = params.get("folder")
  return (
    <ImportFrame step="Credits" id={id} folder={folder}>
      <ReleaseCreditsForm
        releaseId={id}
        submitLabel="Save and continue"
        onSaved={() => navigate(stepUrl("Tags", id, folder))}
      />
    </ImportFrame>
  )
}

/** Step 6: tags. Finish opens the release page. */
export function ImportTags() {
  const navigate = useNavigate()
  const { id = "" } = useParams()
  const [params] = useSearchParams()
  const folder = params.get("folder")
  return (
    <ImportFrame step="Tags" id={id} folder={folder}>
      <ReleaseTagsForm
        releaseId={id}
        submitLabel="Finish"
        onSaved={() => navigate(`/releases/${id}`)}
      />
    </ImportFrame>
  )
}
