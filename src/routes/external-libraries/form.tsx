import { useMutation } from "@tanstack/react-query"
import { open } from "@tauri-apps/plugin-dialog"
import { FolderOpenIcon, SearchIcon } from "lucide-react"
import { useState } from "react"
import { useNavigate, useSearchParams } from "react-router"
import { toast } from "sonner"

import { ButtonLink } from "@/components/button-link"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import type {
  ExternalKind,
  ExternalLibrary,
  ExternalLibraryInput,
  OutputFormat,
} from "@/bindings"
import { useDebounced } from "@/hooks/use-debounced"
import { FORMAT_LABELS, FORMATS, KIND_LABELS } from "@/lib/external"
import { formatSize } from "@/lib/format"
import { call, useCommand } from "@/lib/query"

/** The backend's `DEFAULT_TEMPLATE`, which a blank template is */
const DEFAULT_TEMPLATE =
  "${album_artist}/[${year} - ]${album}/[${disc}-]${track_number:2} ${track_title}.${format}"

/** AAC's bitrates the form offers, in kbps */
const BITRATES = [128, 160, 192, 224, 256, 320]

/**
 * `/external-libraries/create?kind=folder` or `?kind=ipod`: a new external
 * library's name, destination and profile. Its rules come after, in its
 * Contents tab.
 */
export function ExternalLibraryCreate() {
  const [params] = useSearchParams()
  const kind: ExternalKind = params.get("kind") === "ipod" ? "ipod" : "folder"
  // The newest track shows what the template gives
  const { data: newest } = useCommand("tracks", { perPage: 1 })
  return (
    <>
      <PageHeader
        crumbs={[
          { label: "External libraries", to: "/external-libraries" },
          // "New folder library", "New iPod library"
          {
            label: `New ${kind === "ipod" ? KIND_LABELS[kind] : KIND_LABELS[kind].toLowerCase()}`,
          },
        ]}
      />
      <div className="max-w-3xl p-4">
        <ProfileForm
          key={kind}
          kind={kind}
          sampleTrackId={newest?.items[0]?.id ?? null}
        />
      </div>
    </>
  )
}

/**
 * An external library's name, destination and profile (frontend.md, "External
 * libraries"): a new one's, or `library`'s, for its Settings tab. An iPod's
 * profile is what it plays.
 */
export function ProfileForm({
  library,
  kind,
  sampleTrackId,
}: {
  library?: ExternalLibrary
  kind: ExternalKind
  /** A track to show the path template with */
  sampleTrackId: number | null
}) {
  const navigate = useNavigate()
  const folderKind = kind === "folder"
  const [values, setValues] = useState<ExternalLibraryInput>(
    library
      ? {
          name: library.name,
          kind,
          folder: library.folder,
          format: library.format,
          aacKbps: library.aacKbps,
          cdResolution: library.cdResolution,
          pathTemplate: library.pathTemplate,
          embedCovers: library.embedCovers,
          coverPx: library.coverPx,
          coverFile: library.coverFile,
        }
      : {
          name: "",
          kind,
          folder: null,
          format: folderKind ? "original" : "aac",
          aacKbps: 256,
          cdResolution: !folderKind,
          pathTemplate: folderKind ? DEFAULT_TEMPLATE : null,
          embedCovers: folderKind,
          coverPx: 600,
          coverFile: false,
        }
  )
  const change = (changes: Partial<ExternalLibraryInput>) =>
    setValues((current) => ({ ...current, ...changes }))
  const save = useMutation({
    mutationFn: () =>
      library
        ? call("updateExternalLibrary", { id: library.id, ...values })
        : call("createExternalLibrary", values),
  })
  const bitrates = BITRATES.includes(values.aacKbps)
    ? BITRATES
    : [...BITRATES, values.aacKbps].sort((a, b) => a - b)

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          const saved = await save.mutateAsync()
          if (library) {
            toast.success(`Saved ${saved.name}`)
          } else {
            navigate(`/external-libraries/${saved.id}/contents`)
          }
        } catch {
          // The form shows the error
        }
      }}
    >
      <FieldGroup>
        {save.error && <QueryError error={save.error} title="Can't save" />}
        <Field>
          <FieldLabel htmlFor="name">Name</FieldLabel>
          <Input
            id="name"
            required
            value={values.name}
            placeholder={folderKind ? "Car" : "iPod"}
            onChange={(e) => change({ name: e.target.value })}
          />
        </Field>
        {folderKind ? (
          <Field>
            <FieldLabel htmlFor="folder">Folder</FieldLabel>
            <div className="flex gap-2">
              <Input
                id="folder"
                readOnly
                className="font-mono"
                value={values.folder ?? ""}
                placeholder="/Volumes/CARD/Music"
              />
              <Button
                type="button"
                variant="outline"
                onClick={async () => {
                  const folder = await open({
                    directory: true,
                    title: "The folder library's folder",
                    defaultPath: values.folder ?? undefined,
                  })
                  if (typeof folder === "string") change({ folder })
                }}
              >
                <FolderOpenIcon data-icon="inline-start" />
                Choose…
              </Button>
            </div>
            <FieldDescription>
              Where its files go: a card, a player in USB mass storage mode, a
              drive, or a folder another app syncs. It can't be in the
              library&apos;s folder, or hold files the library has registered.
            </FieldDescription>
          </Field>
        ) : (
          <Field>
            <FieldLabel>Destination</FieldLabel>
            <FieldDescription>
              An iPod with video, 5th or 5.5th generation, with Apple&apos;s
              firmware, with its disk use on so it shows in Finder. Its first
              sync goes to the iPod connected then, which keeps the
              library&apos;s manifest, so it&apos;s found wherever it mounts.
            </FieldDescription>
            {!library && <MountedIpods />}
          </Field>
        )}
        <Field>
          <FieldLabel htmlFor="format">Format, for lossless files</FieldLabel>
          <Select
            items={Object.fromEntries(
              FORMATS[kind].map((format) => [format, FORMAT_LABELS[format]])
            )}
            value={values.format}
            onValueChange={(format) =>
              format && change({ format: format as OutputFormat })
            }
          >
            <SelectTrigger id="format" className="w-48">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {FORMATS[kind].map((format) => (
                <SelectItem key={format} value={format}>
                  {FORMAT_LABELS[format]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <FieldDescription>
            Lossy files, like MP3 and AAC, are copied as they are
            {folderKind
              ? "."
              : ", and Ogg Vorbis and Opus become AAC, which the iPod plays."}{" "}
            Everything is converted with Apple&apos;s encoders.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel htmlFor="bitrate">AAC&apos;s bitrate</FieldLabel>
          <Select
            items={Object.fromEntries(
              bitrates.map((kbps) => [String(kbps), `${kbps} kbps`])
            )}
            value={String(values.aacKbps)}
            onValueChange={(kbps) => kbps && change({ aacKbps: Number(kbps) })}
          >
            <SelectTrigger id="bitrate" className="w-48">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {bitrates.map((kbps) => (
                <SelectItem key={kbps} value={String(kbps)}>
                  {kbps} kbps
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        {folderKind && (
          <>
            <Field orientation="horizontal">
              <Switch
                id="cd-resolution"
                checked={values.cdResolution}
                onCheckedChange={(cdResolution) => change({ cdResolution })}
              />
              <FieldContent>
                <FieldLabel htmlFor="cd-resolution">
                  Lossless at CD resolution
                </FieldLabel>
                <FieldDescription>
                  At most 16-bit and 48 kHz, for players that play no more.
                </FieldDescription>
              </FieldContent>
            </Field>
            <TemplateField
              template={values.pathTemplate ?? ""}
              onChange={(pathTemplate) => change({ pathTemplate })}
              sampleTrackId={sampleTrackId}
            />
            <Field orientation="horizontal">
              <Switch
                id="embed-covers"
                checked={values.embedCovers}
                onCheckedChange={(embedCovers) => change({ embedCovers })}
              />
              <FieldContent>
                <FieldLabel htmlFor="embed-covers">Embed covers</FieldLabel>
                <FieldDescription>
                  Each file gets its release&apos;s cover, as a JPEG, never
                  enlarged.
                </FieldDescription>
              </FieldContent>
            </Field>
            {values.embedCovers && (
              <Field>
                <FieldLabel htmlFor="cover-px">
                  Embedded covers&apos; size, in pixels
                </FieldLabel>
                <Input
                  id="cover-px"
                  type="number"
                  min={100}
                  max={3000}
                  className="w-32"
                  value={values.coverPx}
                  onChange={(e) => change({ coverPx: Number(e.target.value) })}
                />
                <FieldDescription>
                  Their longest side, from 100 to 3000. 600 is plenty for a
                  player&apos;s screen.
                </FieldDescription>
              </Field>
            )}
            <Field orientation="horizontal">
              <Switch
                id="cover-file"
                checked={values.coverFile}
                onCheckedChange={(coverFile) => change({ coverFile })}
              />
              <FieldContent>
                <FieldLabel htmlFor="cover-file">
                  A cover.jpg in each release&apos;s folder
                </FieldLabel>
                <FieldDescription>
                  For players that look for one beside the music.
                </FieldDescription>
              </FieldContent>
            </Field>
          </>
        )}
        <div className="flex gap-2">
          <Button type="submit" disabled={save.isPending}>
            {library ? "Save" : "Create"}
          </Button>
          <ButtonLink
            variant="outline"
            to={
              library
                ? `/external-libraries/${library.id}`
                : "/external-libraries"
            }
          >
            Cancel
          </ButtonLink>
        </div>
      </FieldGroup>
    </form>
  )
}

/**
 * A folder library's path template (backend.md, "Path templates"), with the
 * path it gives a sample track, or why it can't be used.
 */
function TemplateField({
  template,
  onChange,
  sampleTrackId,
}: {
  template: string
  onChange: (template: string) => void
  sampleTrackId: number | null
}) {
  const typed = useDebounced(template)
  const { data: sample, error } = useCommand(
    "exportedTags",
    { id: sampleTrackId ?? 0, template: typed.trim() || undefined },
    { enabled: sampleTrackId !== null, keepPrevious: true }
  )
  return (
    <Field>
      <FieldLabel htmlFor="template">Path template</FieldLabel>
      <Input
        id="template"
        className="font-mono"
        value={template}
        placeholder={DEFAULT_TEMPLATE}
        onChange={(e) => onChange(e.target.value)}
      />
      <FieldDescription>
        Where each track goes in the folder. <code>{"${name:2}"}</code> pads a
        number to 2 digits, a part in <code>[…]</code> is left out when a
        variable in it is empty, and <code>/</code> makes a folder. Variables:{" "}
        <code>
          {
            "${album_artist} ${artist} ${album} ${year} ${date} ${cd_number} ${disc} ${cd_count} ${disc_title} ${track_number} ${track_count} ${track_title} ${label} ${catalog_number} ${format}"
          }
        </code>
        . Blank is the default.
      </FieldDescription>
      {error ? (
        <p className="text-xs text-destructive">{error.message}</p>
      ) : (
        sample && (
          <p className="text-xs">
            <span className="text-muted-foreground">For example: </span>
            <span className="font-mono break-all" dir="auto">
              {sample.path}
            </span>
          </p>
        )
      )}
    </Field>
  )
}

/**
 * The iPods connected now, for a new iPod library (frontend.md, "External
 * libraries").
 */
function MountedIpods() {
  const {
    data: ipods,
    error,
    isFetching,
    refetch,
  } = useCommand("mountedIpods", undefined, { fresh: true })
  if (error) return <QueryError error={error} title="Can't look for iPods" />
  return (
    <div className="flex flex-col gap-2 text-xs">
      {!ipods ? (
        <Skeleton className="h-8 w-full" />
      ) : ipods.length === 0 ? (
        <p className="text-muted-foreground">No iPod is connected now.</p>
      ) : (
        <ul className="flex flex-col gap-1.5">
          {ipods.map((ipod) => (
            <li key={ipod.mount} className="flex flex-col gap-0.5">
              <span>
                <span className="font-medium">{ipod.name}</span> ·{" "}
                {ipod.description}
                {ipod.freeBytes !== null &&
                  ` · ${formatSize(ipod.freeBytes)} free`}
              </span>
              {(ipod.refusal ?? ipod.warning ?? ipod.externalLibrary) && (
                <span className="text-muted-foreground">
                  {ipod.refusal ??
                    ipod.warning ??
                    `It belongs to ${ipod.externalLibrary}.`}
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
      <div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={isFetching}
          onClick={() => void refetch()}
        >
          <SearchIcon data-icon="inline-start" />
          Look again
        </Button>
      </div>
    </div>
  )
}
