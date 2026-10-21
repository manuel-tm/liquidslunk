import { useMutation } from "@tanstack/react-query"
import {
  FolderSearchIcon,
  ListChecksIcon,
  ListFilterIcon,
  RefreshCwIcon,
  SearchIcon,
  UploadIcon,
  XIcon,
} from "lucide-react"
import { type ReactNode, useEffect, useState } from "react"
import { Link, useParams } from "react-router"

import { DeleteButton } from "@/components/delete-button"
import { ExternalKindIcon } from "@/components/external-libraries"
import { Pager } from "@/components/list/pager"
import { PER_PAGE } from "@/components/list/list-layout"
import { artistUrl, Names } from "@/components/names"
import { PageHeader } from "@/components/page-header"
import {
  ArtistPicker,
  ReleasePicker,
  type TagOption,
  TagPicker,
  TrackPicker,
} from "@/components/pickers"
import { QueryError } from "@/components/query-error"
import { TabLinks } from "@/components/tab-links"
import { TagValueInput } from "@/components/tag-value-input"
import { Title } from "@/components/titles"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import type {
  ExternalLibraryDetails,
  ExternalRule,
  RuleInput,
} from "@/bindings"
import { useListParams } from "@/hooks/use-list-params"
import {
  describeRule,
  FORMAT_LABELS,
  KIND_LABELS,
  ruleInput,
  sameTarget,
} from "@/lib/external"
import { formatDateTime, formatDuration, formatSize } from "@/lib/format"
import { textProps } from "@/lib/languages"
import { call, idParam, queryClient, useCommand } from "@/lib/query"
import { ejectLibrary, locateLibrary, useSyncRun } from "@/lib/sync"
import { ProfileForm } from "@/routes/external-libraries/form"
import { SyncSheet } from "@/routes/external-libraries/sync-sheet"
import { NotFound } from "@/routes/not-found"

/** The queries of one external library, which a delete removes. */
const LIBRARY_QUERIES = [
  "externalLibrary",
  "externalLibraryRules",
  "externalLibraryTracks",
]

/**
 * The frame of every tab of an external library: the header with Delete, the
 * tabs, then the tab, once the library is there (frontend.md, "External
 * libraries").
 */
function ExternalLibraryFrame({
  children,
}: {
  children: (library: ExternalLibraryDetails) => ReactNode
}) {
  const { id = "" } = useParams()
  const libraryId = idParam(id)
  const { data: library, error } = useCommand(
    "externalLibrary",
    { id: libraryId ?? 0 },
    { enabled: libraryId !== null }
  )
  const [removeFiles, setRemoveFiles] = useState(false)
  const remove = useMutation({
    mutationFn: (id: number) =>
      call("deleteExternalLibrary", { id, removeFiles }),
  })
  if (libraryId === null || library === null) return <NotFound />
  const base = `/external-libraries/${id}`

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "External libraries", to: "/external-libraries" },
          { label: library ? library.name : id },
        ]}
      >
        {library && (
          <DeleteButton
            name={library.name}
            blockers={
              library.syncing ? ["It's syncing. Cancel the sync first."] : []
            }
            detail={
              library.kind === "ipod"
                ? removeFiles
                  ? "Its rules go with it, and so do the tracks liquidslunk put on the iPod. Nothing else on it is touched."
                  : "Its rules go with it. What's on the iPod stays there."
                : removeFiles
                  ? "Its rules go with it, and so does what liquidslunk wrote in its folder. Nothing else there is touched."
                  : "Its rules go with it. What's in its folder stays there."
            }
            onDelete={() => remove.mutateAsync(library.id)}
            then="/external-libraries"
            afterDelete={() => {
              for (const name of LIBRARY_QUERIES) {
                queryClient.removeQueries({
                  queryKey: [name, { id: library.id }],
                })
              }
            }}
          >
            {library.destination.type === "connected" &&
              (library.folder || library.ipod) && (
                <label className="flex items-start gap-2 text-xs">
                  <Checkbox
                    checked={removeFiles}
                    onCheckedChange={(checked) => setRemoveFiles(checked)}
                  />
                  {library.folder ? (
                    <span>
                      Also remove the files liquidslunk wrote in{" "}
                      <span className="font-mono break-all">
                        {library.folder}
                      </span>
                    </span>
                  ) : (
                    <span>
                      Also remove the tracks liquidslunk put on{" "}
                      {library.ipod?.name}
                    </span>
                  )}
                </label>
              )}
          </DeleteButton>
        )}
      </PageHeader>
      <div className="flex flex-col gap-4 p-4 text-xs">
        <TabLinks
          tabs={[
            { label: "Overview", to: base },
            {
              label: "Contents",
              to: `${base}/contents`,
              count: library?.contents.tracks,
            },
            { label: "Settings", to: `${base}/settings` },
          ]}
        />
        {error ? (
          <QueryError error={error} title="Can't load the external library" />
        ) : !library ? (
          <Skeleton className="h-64 w-full" />
        ) : (
          children(library)
        )}
      </div>
    </>
  )
}

/** `/external-libraries/:id`: where it goes, and what's in it. */
export function ExternalLibraryOverview() {
  return (
    <ExternalLibraryFrame>
      {(library) => <Overview library={library} />}
    </ExternalLibraryFrame>
  )
}

function Overview({ library }: { library: ExternalLibraryDetails }) {
  const { contents, destination, ipod } = library
  const onIpod = library.kind === "ipod"
  // An iPod is looked for again when the window comes to the front
  // (frontend.md, "External libraries")
  useEffect(() => {
    if (!onIpod) return
    const lookAgain = () =>
      void queryClient.invalidateQueries({
        queryKey: ["externalLibrary", { id: library.id }],
      })
    window.addEventListener("focus", lookAgain)
    return () => window.removeEventListener("focus", lookAgain)
  }, [onIpod, library.id])
  const [syncing, setSyncing] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<Error | null>(null)
  const run = useSyncRun()
  const running = run?.running ?? false
  const mine = run?.libraryId === library.id ? run : null
  const format =
    library.format === "original"
      ? "Files are kept as they are."
      : `Lossless files become ${FORMAT_LABELS[library.format]}${
          library.format === "aac" ? ` at ${library.aacKbps} kbps` : ""
        }${library.cdResolution ? ", at CD resolution" : ""}. Lossy ones are copied.`
  const free =
    library.freeBytes !== null
      ? `, with ${formatSize(library.freeBytes)} free`
      : ""
  const connected =
    destination.type === "connected"
      ? ipod
        ? `Yes: ${ipod.name}${free}`
        : `Yes${free}`
      : destination.type === "elsewhere"
        ? `No: the ${onIpod ? "iPod connected" : "folder"} belongs to another external library, ${destination.name}`
        : destination.type === "several"
          ? "No: several iPods are connected, and none is this one's yet. Eject all but the one to sync."
          : onIpod
            ? "No: no iPod is connected. Connect it with its disk use on, so it shows in Finder."
            : "No: the folder isn't there"
  const facts: [string, ReactNode][] = [
    [
      "Kind",
      <span className="inline-flex items-center gap-1.5">
        <ExternalKindIcon kind={library.kind} className="size-3.5" />
        {KIND_LABELS[library.kind]}
      </span>,
    ],
    [
      "Destination",
      library.folder ? (
        <span className="font-mono break-all">{library.folder}</span>
      ) : ipod ? (
        <>
          {ipod.description}
          {(ipod.refusal ?? ipod.warning) && (
            <span className="text-muted-foreground">
              . {ipod.refusal ?? ipod.warning}
            </span>
          )}
        </>
      ) : (
        "An iPod with video, found when it's connected"
      ),
    ],
    ["Connected", connected],
    [
      "In it",
      contents.tracks === 0 ? (
        <>
          Nothing yet.{" "}
          <Link
            to={`/external-libraries/${library.id}/contents`}
            className="underline"
          >
            Choose what goes in it
          </Link>
          , or use Add to external library on a release, artist or track.
        </>
      ) : (
        <>
          {plural(contents.releases, "release")},{" "}
          {plural(contents.tracks, "track")}, about{" "}
          {formatSize(contents.sizeBytes)}
          {contents.leftOut > 0 &&
            `. ${plural(contents.leftOut, "track")} can't be converted or tagged, so a sync leaves them out`}
        </>
      ),
    ],
    ["Format", format],
    [
      "Last synced",
      mine?.running
        ? "Syncing now"
        : library.lastSyncedAt
          ? formatDateTime(library.lastSyncedAt)
          : "Never",
    ],
  ]
  const act = async (work: () => Promise<unknown>) => {
    setBusy(true)
    setError(null)
    try {
      await work()
    } catch (caught) {
      setError(caught as Error)
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="flex max-w-3xl flex-col gap-4">
      <dl className="grid grid-cols-[minmax(6rem,auto)_1fr] gap-x-4 gap-y-2">
        {facts.map(([label, value]) => (
          <div key={label} className="contents">
            <dt className="text-muted-foreground">{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <div className="flex flex-wrap items-center gap-2">
        {mine?.running ? (
          <Button onClick={() => setSyncing(true)}>
            <Spinner data-icon="inline-start" />
            Syncing
            {mine.progress &&
              `: ${mine.progress.done} of ${mine.progress.total}`}
          </Button>
        ) : (
          <Button
            disabled={destination.type !== "connected" || running || busy}
            onClick={() => setSyncing(true)}
          >
            <RefreshCwIcon data-icon="inline-start" />
            Sync…
          </Button>
        )}
        {library.ejectable && (
          <Button
            variant="outline"
            disabled={running || busy}
            onClick={() =>
              act(() =>
                ejectLibrary(
                  library.id,
                  ipod ? { ipod: ipod.name } : { folder: library.folder ?? "" }
                )
              )
            }
          >
            <UploadIcon data-icon="inline-start" />
            Eject
          </Button>
        )}
        {!onIpod && destination.type !== "connected" && (
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => act(() => locateLibrary(library.id, library.folder))}
          >
            <FolderSearchIcon data-icon="inline-start" />
            Locate…
          </Button>
        )}
        {onIpod && (
          <Button
            variant="outline"
            disabled={busy}
            onClick={() =>
              act(() =>
                queryClient.invalidateQueries({
                  queryKey: ["externalLibrary", { id: library.id }],
                })
              )
            }
          >
            <SearchIcon data-icon="inline-start" />
            Look again
          </Button>
        )}
      </div>
      {error && <QueryError error={error} title="Can't do that" />}
      <SyncSheet library={library} open={syncing} onOpenChange={setSyncing} />
    </div>
  )
}

/** `/external-libraries/:id/settings`: its name, destination and profile. */
export function ExternalLibrarySettings() {
  return (
    <ExternalLibraryFrame>
      {(library) => (
        <div className="max-w-3xl">
          <ProfileForm
            // Keyed, so it starts from what's saved
            key={`${library.id}:${library.updatedAt}`}
            library={library}
            kind={library.kind}
            sampleTrackId={library.sampleTrackId}
          />
        </div>
      )}
    </ExternalLibraryFrame>
  )
}

/**
 * `/external-libraries/:id/contents`: its rules, what's in and what's left
 * out, and the tracks they come to.
 */
export function ExternalLibraryContents() {
  return (
    <ExternalLibraryFrame>
      {(library) => <Contents library={library} />}
    </ExternalLibraryFrame>
  )
}

function Contents({ library }: { library: ExternalLibraryDetails }) {
  const { data: rules, error } = useCommand("externalLibraryRules", {
    id: library.id,
  })
  const save = useMutation({
    mutationFn: (rules: RuleInput[]) =>
      call("setExternalLibraryRules", { id: library.id, rules }),
  })
  // The pickers start afresh after each pick
  const [picked, setPicked] = useState(0)
  if (error) return <QueryError error={error} title="Can't load the rules" />
  if (!rules) return <Skeleton className="h-64 w-full" />

  const inputs = rules.map(ruleInput)
  async function change(next: RuleInput[]) {
    try {
      await save.mutateAsync(next)
    } catch {
      // The tab shows the error
    }
  }
  // A thing is named once: a new rule for it replaces the old one
  const add = (rule: RuleInput) => {
    setPicked((count) => count + 1)
    return change([...inputs.filter((other) => !sameTarget(other, rule)), rule])
  }
  const remove = (index: number) =>
    change(inputs.filter((_, other) => other !== index))
  const named = (type: string, exclude: boolean) =>
    rules.flatMap((rule) => {
      const { target } = rule
      if (rule.exclude !== exclude || target.type !== type) return []
      return target.type === "release"
        ? [String(target.release.id)]
        : target.type === "artist"
          ? [String(target.artist.id)]
          : target.type === "track"
            ? [String(target.track.id)]
            : []
    })
  const ruleList = (exclude: boolean) =>
    rules
      .map((rule, index) => ({ rule, index }))
      .filter(({ rule }) => rule.exclude === exclude)
  const everything = rules.some((rule) => rule.target.type === "everything")

  return (
    <div className="flex flex-col gap-6">
      {save.error && (
        <QueryError error={save.error} title="Can't change the rules" />
      )}
      <section className="flex max-w-4xl flex-col gap-2">
        <h2 className="font-heading text-base font-semibold">In it</h2>
        <p className="text-muted-foreground">
          Everything, or releases, the releases of artists, now and later,
          tracks, and the releases and tracks with a tag value. An artist&apos;s
          appearances on other artists&apos; releases need a rule of their own.
        </p>
        <RuleList
          rules={ruleList(false)}
          empty="Nothing yet."
          onRemove={remove}
        />
        <div className="grid grid-cols-2 gap-2 lg:grid-cols-5">
          <Button
            variant="outline"
            disabled={everything}
            onClick={() => add({ target: { type: "everything" } })}
          >
            <ListChecksIcon data-icon="inline-start" />
            Add everything
          </Button>
          <ReleasePicker
            value={null}
            key={`Add a release:${picked}`}
            placeholder="Add a release"
            exclude={named("release", false)}
            onChange={(option) =>
              option &&
              add({ target: { type: "release", id: Number(option.id) } })
            }
          />
          <ArtistPicker
            value={null}
            creatable={false}
            key={`Add an artist:${picked}`}
            placeholder="Add an artist"
            exclude={named("artist", false)}
            onChange={(option) =>
              option &&
              add({ target: { type: "artist", id: Number(option.id) } })
            }
          />
          <TrackPicker
            value={null}
            key={`Add a track:${picked}`}
            placeholder="Add a track"
            exclude={named("track", false)}
            onChange={(option) =>
              option &&
              add({ target: { type: "track", id: Number(option.id) } })
            }
          />
          <AddTagRule onAdd={add} />
        </div>
      </section>
      <section className="flex max-w-4xl flex-col gap-2">
        <h2 className="font-heading text-base font-semibold">Left out</h2>
        <p className="text-muted-foreground">
          Releases and tracks to leave out, whatever brings them in: &quot;Pink
          Floyd, but not The Wall&quot;.
        </p>
        <RuleList rules={ruleList(true)} empty="Nothing." onRemove={remove} />
        <div className="grid grid-cols-2 gap-2 lg:grid-cols-5">
          <ReleasePicker
            value={null}
            key={`Leave out a release:${picked}`}
            placeholder="Leave out a release"
            exclude={named("release", true)}
            onChange={(option) =>
              option &&
              add({
                exclude: true,
                target: { type: "release", id: Number(option.id) },
              })
            }
          />
          <TrackPicker
            value={null}
            key={`Leave out a track:${picked}`}
            placeholder="Leave out a track"
            exclude={named("track", true)}
            onChange={(option) =>
              option &&
              add({
                exclude: true,
                target: { type: "track", id: Number(option.id) },
              })
            }
          />
        </div>
      </section>
      <LibraryTracks library={library} />
    </div>
  )
}

const RULE_TYPES = {
  release: "Release",
  artist: "Artist",
  track: "Track",
  tag: "Tag value",
  everything: "Everything",
}

/** Rules, each with what it names, linking to it, and a button that removes it. */
function RuleList({
  rules,
  empty,
  onRemove,
}: {
  rules: { rule: ExternalRule; index: number }[]
  empty: string
  onRemove: (index: number) => void
}) {
  if (rules.length === 0) {
    return <p className="text-muted-foreground">{empty}</p>
  }
  return (
    <ul className="flex flex-col gap-1">
      {rules.map(({ rule, index }) => {
        const { target } = rule
        const label = describeRule(target)
        return (
          <li key={index} className="flex items-center gap-2">
            <Badge variant="outline" className="w-20 justify-center">
              {RULE_TYPES[target.type]}
            </Badge>
            {target.type === "release" ? (
              <span>
                <Link
                  to={`/releases/${target.release.id}`}
                  className="hover:underline"
                >
                  <Title
                    title={target.release.title}
                    language={target.release.titleLanguage}
                  />
                </Link>
                {target.release.artists.length > 0 && (
                  <span className="text-muted-foreground">
                    {" "}
                    by <Names items={target.release.artists} to={artistUrl} />
                  </span>
                )}
              </span>
            ) : target.type === "artist" ? (
              <Link
                to={`/artists/${target.artist.id}`}
                className="hover:underline"
                {...textProps(target.artist.nameLanguage)}
              >
                {target.artist.name}
              </Link>
            ) : target.type === "track" ? (
              <span>
                <Link
                  to={`/tracks/${target.track.id}`}
                  className="hover:underline"
                >
                  <Title
                    title={target.track.title}
                    language={
                      target.track.titleLanguage ??
                      target.track.release.titleLanguage
                    }
                  />
                </Link>
                <span className="text-muted-foreground">
                  {" "}
                  on{" "}
                  <Title
                    title={target.track.release.title}
                    language={target.track.release.titleLanguage}
                  />
                </span>
              </span>
            ) : target.type === "everything" ? (
              <span>The whole library, now and later</span>
            ) : (
              <Link
                to={`/tags/${target.tag.type}/${target.tag.id}`}
                className="hover:underline"
              >
                {label}
              </Link>
            )}
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={`Remove ${label}`}
              title="Remove"
              onClick={() => onRemove(index)}
            >
              <XIcon />
            </Button>
          </li>
        )
      })}
    </ul>
  )
}

/**
 * Adds a tag value rule: a text tag's value, or a range of a number or date
 * tag's, either end left empty, both included.
 */
function AddTagRule({ onAdd }: { onAdd: (rule: RuleInput) => void }) {
  const [open, setOpen] = useState(false)
  const [tag, setTag] = useState<TagOption | null>(null)
  const [value, setValue] = useState("")
  const [min, setMin] = useState("")
  const [max, setMax] = useState("")
  const ready =
    tag !== null &&
    (tag.type === "text" ? value.trim() !== "" : (min + max).trim() !== "")

  function reset() {
    setTag(null)
    setValue("")
    setMin("")
    setMax("")
  }

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (!next) reset()
      }}
    >
      <PopoverTrigger render={<Button variant="outline" />}>
        <ListFilterIcon data-icon="inline-start" />
        Add a tag value
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80">
        <form
          className="flex flex-col gap-2"
          onSubmit={(event) => {
            event.preventDefault()
            if (!tag || !ready) return
            const given = (text: string) => text.trim() || null
            onAdd({
              target: {
                type: "tag",
                tag: { type: tag.type, id: Number(tag.tagId) },
                equals: tag.type === "text" ? given(value) : null,
                min: tag.type === "text" ? null : given(min),
                max: tag.type === "text" ? null : given(max),
              },
            })
            setOpen(false)
            reset()
          }}
        >
          <TagPicker
            id="rule-tag"
            value={tag}
            onChange={setTag}
            placeholder="Search tags"
          />
          {tag?.type === "text" && (
            <TagValueInput
              id="rule-value"
              aria-label="Value"
              type="text"
              tagId={tag.tagId}
              value={value}
              onValueChange={setValue}
            />
          )}
          {tag && tag.type !== "text" && (
            <>
              <div className="grid grid-cols-2 gap-2">
                <Input
                  aria-label="From"
                  inputMode={tag.type === "number" ? "decimal" : undefined}
                  placeholder={
                    tag.type === "date" ? "From: 2024-03" : "From: 8"
                  }
                  value={min}
                  onChange={(e) => setMin(e.target.value)}
                />
                <Input
                  aria-label="To"
                  inputMode={tag.type === "number" ? "decimal" : undefined}
                  placeholder={tag.type === "date" ? "To: 2024" : "To: 10"}
                  value={max}
                  onChange={(e) => setMax(e.target.value)}
                />
              </div>
              <p className="text-muted-foreground">
                Either end can be left empty, and both are included.
              </p>
            </>
          )}
          <Button type="submit" disabled={!ready}>
            Add
          </Button>
        </form>
      </PopoverContent>
    </Popover>
  )
}

/** The tracks the rules come to, a page at a time, by release. */
function LibraryTracks({ library }: { library: ExternalLibraryDetails }) {
  const list = useListParams()
  const { data, error } = useCommand(
    "externalLibraryTracks",
    { id: library.id, page: list.page, perPage: PER_PAGE },
    { keepPrevious: true }
  )
  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-center gap-4">
        <h2 className="font-heading text-base font-semibold">Tracks</h2>
        <div className="ml-auto">
          {data && data.total > 0 && (
            <Pager
              page={list.page}
              perPage={PER_PAGE}
              total={data.total}
              onPage={(page) => list.update({ page })}
            />
          )}
        </div>
      </div>
      {error ? (
        <QueryError error={error} title="Can't load the tracks" />
      ) : !data ? (
        <Skeleton className="h-32 w-full" />
      ) : data.total === 0 ? (
        <p className="text-muted-foreground">No tracks yet.</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Release</TableHead>
              <TableHead className="text-right">Disc</TableHead>
              <TableHead className="text-right">No.</TableHead>
              <TableHead>Title</TableHead>
              <TableHead>Artists</TableHead>
              <TableHead className="text-right">Duration</TableHead>
              <TableHead>File</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {data.items.map((track) => (
              <TableRow key={track.id}>
                <TableCell className="whitespace-normal">
                  <Link
                    to={`/releases/${track.release.id}`}
                    className="hover:underline"
                  >
                    <Title
                      title={track.release.title}
                      language={track.release.titleLanguage}
                    />
                  </Link>
                </TableCell>
                <TableCell className="text-right tabular-nums">
                  {track.cdNumber}
                </TableCell>
                <TableCell className="text-right tabular-nums">
                  {track.trackNumber}
                </TableCell>
                <TableCell className="whitespace-normal">
                  <Link to={`/tracks/${track.id}`} className="hover:underline">
                    <Title
                      title={track.title}
                      language={
                        track.titleLanguage ?? track.release.titleLanguage
                      }
                    />
                  </Link>
                </TableCell>
                <TableCell className="whitespace-normal">
                  <Names items={track.performers} to={artistUrl} />
                </TableCell>
                <TableCell className="text-right tabular-nums">
                  {formatDuration(track.durationMs)}
                </TableCell>
                <TableCell className="font-mono whitespace-normal">
                  {track.file.path}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </section>
  )
}

function plural(n: number, noun: string) {
  return `${n} ${noun}${n === 1 ? "" : "s"}`
}
