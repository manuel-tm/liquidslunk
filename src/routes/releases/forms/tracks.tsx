import { useMutation } from "@tanstack/react-query"
import { ArrowLeftRightIcon, SplitIcon, XIcon } from "lucide-react"
import { Fragment, useState } from "react"

import {
  type AlternateTitleRow,
  alternateTitleRow,
  alternateTitlesInput,
  alternateTitlesValid,
  TitlesPopover,
} from "@/components/alternate-titles-editor"
import { ChipList } from "@/components/chip-list"
import { OtherNamesOffer } from "@/components/other-names-offer"
import type { Option } from "@/components/picker"
import { ArtistPicker, FilePicker } from "@/components/pickers"
import { QueryError } from "@/components/query-error"
import {
  SplitTitleButton,
  splitTitle,
  swapTitle,
  type TitleValues,
} from "@/components/title-split"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import type { ReleaseTracks } from "@/bindings"
import { creditedInput, creditedOption } from "@/lib/credited"
import { formatDuration } from "@/lib/format"
import {
  type OfferedName,
  offeredNames,
  pickedArtists,
  unpicked,
  unpickedLabel,
} from "@/lib/import"
import { textProps } from "@/lib/languages"
import { call, idParam, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

type TrackRow = {
  key: string
  /** A track the release has */
  id?: number
  /** A registered file, for a new track */
  fileId?: number
  path: string
  cdNumber: number
  trackNumber: number | ""
  title: string
  /** Null: the release's */
  titleLanguage: string | null
  alternateTitles: AlternateTitleRow[]
  artists: Option[]
  features: Option[]
  durationMs: number | null
  /** Unchecked rows are skipped */
  include: boolean
  /** A row from the import's folder, which is unchecked instead of removed */
  fromFolder: boolean
  /** Why it can't be checked: it's another release's track */
  locked?: string
  /** Tag artist names that picked no artist */
  hint?: string
  /** Those names, typed into the artists picker in turn */
  typed?: string[]
  /** Its title was split at " = ", so it can be swapped */
  split?: boolean
}

function initialRows(data: ReleaseTracks): TrackRow[] {
  const rows: TrackRow[] = data.tracks.map((track) => ({
    key: `track-${track.id}`,
    id: track.id,
    fileId: track.file.id,
    path: track.file.path,
    cdNumber: track.cdNumber,
    trackNumber: track.trackNumber,
    title: track.title,
    titleLanguage: track.titleLanguage,
    alternateTitles: track.alternateTitles.map(alternateTitleRow),
    artists: track.artists.map(creditedOption),
    features: track.features.map(creditedOption),
    durationMs: track.durationMs,
    include: true,
    fromFolder: false,
  }))
  // On a regular release, the release's artists perform a track with none of
  // its own, so tags naming them don't become track artists
  const releaseArtists = new Set(
    data.variousArtists ? [] : data.artists.map((link) => link.artist.id)
  )
  for (const suggestion of data.trackSuggestions ?? []) {
    // Already one of this release's tracks: it has its row
    if (suggestion.track?.release.id === data.id) continue
    const unmatched = unpicked(suggestion.artists)
    rows.push({
      key: `path-${suggestion.path}`,
      fileId: suggestion.file?.id,
      path: suggestion.path,
      cdNumber: suggestion.cdNumber,
      trackNumber: suggestion.trackNumber,
      title: suggestion.title,
      titleLanguage: null,
      alternateTitles: [],
      artists: pickedArtists(suggestion.artists, releaseArtists),
      features: [],
      durationMs: suggestion.durationMs ?? null,
      include: !suggestion.track,
      fromFolder: true,
      locked: suggestion.track
        ? `Already a track on ${suggestion.track.release.title}`
        : undefined,
      hint: unmatched.length
        ? `Tags: ${unmatched.map(unpickedLabel).join(", ")}`
        : undefined,
      typed: unmatched.map((a) => a.name),
    })
  }
  // Sorted once: rows don't move while their numbers are being typed
  return rows.sort(
    (a, b) =>
      a.cdNumber - b.cdNumber || Number(a.trackNumber) - Number(b.trackNumber)
  )
}

/** A disc's title, its language, and its alternate titles. */
type DiscTitleValues = {
  title: string
  /** Null: the release's */
  language: string | null
  alternateTitles: AlternateTitleRow[]
}

const NO_DISC_TITLE: DiscTitleValues = {
  title: "",
  language: null,
  alternateTitles: [],
}

/** Titles by disc number: the release's, and else the folder's tags. */
function initialTitles(data: ReleaseTracks): Record<number, DiscTitleValues> {
  const titles: Record<number, DiscTitleValues> = {}
  for (const disc of data.discTitles) {
    titles[disc.cdNumber] = {
      title: disc.title,
      language: disc.language,
      alternateTitles: disc.alternateTitles.map(alternateTitleRow),
    }
  }
  for (const { cdNumber, discTitle } of data.trackSuggestions ?? []) {
    if (discTitle && titles[cdNumber] === undefined) {
      titles[cdNumber] = { ...NO_DISC_TITLE, title: discTitle }
    }
  }
  return titles
}

/**
 * A release's tracks: a row each, in order. Saving deletes the tracks whose
 * rows were removed, not their files. With `folder` (the import), there's also
 * a row for each of its audio files that isn't a track yet.
 */
export function ReleaseTracksForm({
  releaseId,
  folder,
  submitLabel = "Save",
  onSaved,
}: {
  /** From the URL */
  releaseId: string
  folder?: string
  submitLabel?: string
  onSaved: () => void
}) {
  const id = idParam(releaseId)
  // Rows start from what's on disk and in the database now (frontend.md,
  // "Commands")
  const { data, error } = useCommand(
    "releaseTracks",
    { id: id ?? 0, folder },
    { fresh: true, enabled: id !== null }
  )

  if (id === null || data === null) return <NotFound />
  if (error) return <QueryError error={error} title="Can't load the tracks" />
  if (!data) return <Skeleton className="h-64 w-full" />
  return (
    <Rows
      key={data.id}
      releaseId={data.id}
      variousArtists={data.variousArtists}
      offerSplit={folder !== undefined}
      offers={offeredNames(
        (data.trackSuggestions ?? []).flatMap((row) => row.artists)
      )}
      initial={initialRows(data)}
      initialTitles={initialTitles(data)}
      submitLabel={submitLabel}
      onSaved={onSaved}
    />
  )
}

function Rows({
  releaseId,
  variousArtists,
  offerSplit,
  offers,
  initial,
  initialTitles,
  submitLabel,
  onSaved,
}: {
  releaseId: number
  variousArtists: boolean
  /** The import's split of titles with " = " */
  offerSplit: boolean
  offers: OfferedName[]
  initial: TrackRow[]
  initialTitles: Record<number, DiscTitleValues>
  submitLabel: string
  onSaved: () => void
}) {
  const [rows, setRows] = useState(initial)
  const [titles, setTitles] = useState(initialTitles)
  const [pickerKey, setPickerKey] = useState(0)
  const edit = (key: string, changes: Partial<TrackRow>) =>
    setRows((current) =>
      current.map((row) => (row.key === key ? { ...row, ...changes } : row))
    )
  const included = rows.filter((row) => row.include)
  const titlesOf = (row: TrackRow): TitleValues => ({
    title: row.title,
    language: row.titleLanguage,
    alternateTitles: row.alternateTitles,
  })
  const withTitles = (row: TrackRow, values: TitleValues) => ({
    ...row,
    title: values.title,
    titleLanguage: values.language,
    alternateTitles: values.alternateTitles,
    split: true,
  })
  const splittable = offerSplit
    ? included.filter((row) => splitTitle(titlesOf(row)))
    : []
  const swappable = offerSplit
    ? included.filter(
        (row) =>
          row.split &&
          row.alternateTitles.length > 0 &&
          !splitTitle(titlesOf(row))
      )
    : []
  /** Splits or swaps the included rows that can be */
  const changeAll = (change: (row: TrackRow) => TitleValues | null) =>
    setRows((current) =>
      current.map((row) => {
        const values = row.include ? change(row) : null
        return values ? withTitles(row, values) : row
      })
    )
  // A title field for each disc the saved tracks will be on
  const discs = [...new Set(included.map((row) => row.cdNumber))]
    .filter((disc) => Number.isInteger(disc) && disc > 0)
    .sort((a, b) => a - b)
  const save = useMutation({
    mutationFn: () =>
      call("setReleaseTracks", {
        id: releaseId,
        tracks: included.map((row) => ({
          id: row.id,
          fileId: row.id ? undefined : row.fileId,
          path: row.id || row.fileId ? undefined : row.path,
          cdNumber: row.cdNumber,
          trackNumber: Number(row.trackNumber),
          title: row.title,
          titleLanguage: row.titleLanguage,
          alternateTitles: alternateTitlesInput(row.alternateTitles),
          artists: creditedInput(row.artists),
          features: creditedInput(row.features),
        })),
        discTitles: discs.map((disc) => {
          const { title, language, alternateTitles } =
            titles[disc] ?? NO_DISC_TITLE
          return {
            cdNumber: disc,
            title,
            language,
            alternateTitles: alternateTitlesInput(alternateTitles),
          }
        }),
      }),
  })

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          await save.mutateAsync()
          onSaved()
        } catch {
          // The form shows the error
        }
      }}
    >
      <p className="text-xs text-muted-foreground">
        {variousArtists
          ? "A various-artists release: give each track its own artists."
          : "On a regular release, leave artists empty and the release's artists perform the track."}
      </p>
      {save.error && (
        <QueryError error={save.error} title="Can't save the tracks" />
      )}
      <OtherNamesOffer offers={offers} />
      {(splittable.length > 0 || swappable.length > 0) && (
        <div className="flex max-w-2xl items-center gap-3 border px-2 py-1.5 text-xs">
          <span className="flex-1">
            {splittable.length > 0
              ? `${splittable.length} ${splittable.length === 1 ? "title has" : "titles have"} " = " in them, like a title printed in two languages. Splitting makes the first part the main title, and the others alternate titles.`
              : "Swap makes the other part of each split title the main one."}
          </span>
          {splittable.length > 0 && (
            <Button
              type="button"
              size="xs"
              variant="outline"
              onClick={() => changeAll((row) => splitTitle(titlesOf(row)))}
            >
              <SplitIcon />
              Split all
            </Button>
          )}
          {swappable.length > 0 && (
            <Button
              type="button"
              size="xs"
              variant="outline"
              onClick={() =>
                changeAll((row) =>
                  swappable.some((r) => r.key === row.key)
                    ? swapTitle(titlesOf(row))
                    : null
                )
              }
            >
              <ArrowLeftRightIcon />
              Swap all
            </Button>
          )}
        </div>
      )}
      {discs.length > 0 && (
        <fieldset className="flex max-w-2xl flex-col gap-2">
          <legend className="mb-2 font-medium">Disc titles</legend>
          <div className="grid grid-cols-[auto_1fr_auto] items-center gap-x-3 gap-y-2">
            {discs.map((disc) => {
              const values = titles[disc] ?? NO_DISC_TITLE
              const set = (changes: Partial<DiscTitleValues>) =>
                setTitles((current) => ({
                  ...current,
                  [disc]: { ...(current[disc] ?? NO_DISC_TITLE), ...changes },
                }))
              return (
                <Fragment key={disc}>
                  <Label htmlFor={`disc-title-${disc}`}>Disc {disc}</Label>
                  <Input
                    id={`disc-title-${disc}`}
                    placeholder="No title"
                    value={values.title}
                    {...textProps(values.language)}
                    onChange={(e) => set({ title: e.target.value })}
                  />
                  {/* Alternate titles belong to the disc's title */}
                  <TitlesPopover
                    main={values.title}
                    language={values.language}
                    titles={values.alternateTitles}
                    disabled={!values.title.trim()}
                    onChange={(language, alternateTitles) =>
                      set({ language, alternateTitles })
                    }
                  />
                </Fragment>
              )
            })}
          </div>
        </fieldset>
      )}
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-8" />
            <TableHead>Disc</TableHead>
            <TableHead>No.</TableHead>
            <TableHead>Title</TableHead>
            <TableHead>Artists</TableHead>
            <TableHead>Features</TableHead>
            <TableHead className="text-right">Duration</TableHead>
            <TableHead>File</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.map((row) => {
            const off = !row.include
            return (
              <TableRow
                key={row.key}
                className={off ? "opacity-60" : undefined}
              >
                {/* Top-aligned, so a hint or chips don't push the row's inputs
                    down; non-inputs are centered on the inputs' h-8 line */}
                <TableCell className="align-top">
                  <div className="flex h-8 items-center">
                    {row.fromFolder ? (
                      <Checkbox
                        aria-label={`Include ${row.path}`}
                        checked={row.include}
                        disabled={!!row.locked}
                        onCheckedChange={(include) =>
                          edit(row.key, { include })
                        }
                      />
                    ) : (
                      <Button
                        type="button"
                        size="icon-xs"
                        variant="ghost"
                        aria-label={`Remove ${row.title}`}
                        onClick={() =>
                          setRows((current) =>
                            current.filter((r) => r.key !== row.key)
                          )
                        }
                      >
                        <XIcon />
                      </Button>
                    )}
                  </div>
                </TableCell>
                <TableCell className="align-top">
                  <Input
                    aria-label="Disc"
                    type="number"
                    min={1}
                    className="w-14"
                    disabled={off}
                    value={row.cdNumber}
                    onChange={(e) =>
                      edit(row.key, { cdNumber: Number(e.target.value) })
                    }
                  />
                </TableCell>
                <TableCell className="align-top">
                  <Input
                    aria-label="Number"
                    type="number"
                    min={1}
                    required={row.include}
                    className="w-16"
                    disabled={off}
                    value={row.trackNumber}
                    onChange={(e) =>
                      edit(row.key, {
                        trackNumber:
                          e.target.value === "" ? "" : Number(e.target.value),
                      })
                    }
                  />
                </TableCell>
                <TableCell className="align-top">
                  <div className="flex items-center gap-1">
                    <Input
                      aria-label="Title"
                      required={row.include}
                      className="min-w-48"
                      disabled={off}
                      value={row.title}
                      {...textProps(row.titleLanguage)}
                      onChange={(e) => edit(row.key, { title: e.target.value })}
                    />
                    {offerSplit && (
                      <SplitTitleButton
                        values={titlesOf(row)}
                        split={row.split ?? false}
                        disabled={off}
                        onChange={(values) =>
                          setRows((current) =>
                            current.map((r) =>
                              r.key === row.key ? withTitles(r, values) : r
                            )
                          )
                        }
                      />
                    )}
                    <TitlesPopover
                      main={row.title}
                      language={row.titleLanguage}
                      titles={row.alternateTitles}
                      disabled={off}
                      onChange={(titleLanguage, alternateTitles) =>
                        edit(row.key, { titleLanguage, alternateTitles })
                      }
                    />
                  </div>
                </TableCell>
                <TableCell className="align-top">
                  <ChipList
                    items={row.artists}
                    disabled={off}
                    onChange={(artists) => edit(row.key, { artists })}
                    picker={(p) => <ArtistPicker {...p} />}
                    credited
                    typed={row.typed}
                  />
                  {row.hint && (
                    <p className="mt-1 text-muted-foreground">{row.hint}</p>
                  )}
                </TableCell>
                <TableCell className="align-top">
                  <ChipList
                    items={row.features}
                    disabled={off}
                    onChange={(features) => edit(row.key, { features })}
                    picker={(p) => <ArtistPicker {...p} />}
                    credited
                  />
                </TableCell>
                <TableCell className="text-right align-top leading-8 tabular-nums">
                  {formatDuration(row.durationMs)}
                </TableCell>
                <TableCell className="align-top">
                  <div className="font-mono leading-8">{row.path}</div>
                  {row.locked && (
                    <p className="text-muted-foreground">{row.locked}</p>
                  )}
                </TableCell>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
      <div className="flex items-end gap-4">
        <div key={pickerKey} className="w-96">
          <FilePicker
            type="music"
            withoutTrack
            placeholder="Add track: a registered audio file"
            value={null}
            exclude={rows.flatMap((row) =>
              row.fileId ? [String(row.fileId)] : []
            )}
            onChange={(file) => {
              if (!file) return
              const last = Math.max(
                0,
                ...rows
                  .filter((r) => r.cdNumber === 1)
                  .map((r) => Number(r.trackNumber) || 0)
              )
              const name = file.name.split("/").pop() ?? file.name
              setRows((current) => [
                ...current,
                {
                  key: `file-${file.id}`,
                  // Options' ids are strings, as the pickers' are
                  fileId: Number(file.id),
                  path: file.name,
                  cdNumber: 1,
                  trackNumber: last + 1,
                  title: name
                    .replace(/\.[^.]+$/, "")
                    .replace(/^\d+[\s._-]+/, ""),
                  titleLanguage: null,
                  alternateTitles: [],
                  artists: [],
                  features: [],
                  durationMs: null,
                  include: true,
                  fromFolder: false,
                },
              ])
              setPickerKey((key) => key + 1)
            }}
          />
        </div>
        <Button
          type="submit"
          disabled={
            save.isPending ||
            !included.every((row) =>
              alternateTitlesValid(row.title, row.alternateTitles)
            ) ||
            !discs.every((disc) =>
              alternateTitlesValid(
                titles[disc]?.title ?? "",
                titles[disc]?.alternateTitles ?? []
              )
            )
          }
          className="ml-auto"
        >
          {submitLabel}
        </Button>
      </div>
    </form>
  )
}
