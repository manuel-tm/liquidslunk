import { useMutation } from "@tanstack/react-query"
import { useState } from "react"
import { useNavigate, useParams } from "react-router"

import {
  type AlternateNameRow,
  alternateNameRow,
  AlternateNamesEditor,
  alternateNamesValid,
  artistNamesInput,
  cleanName,
} from "@/components/alternate-names-editor"
import { ButtonLink } from "@/components/button-link"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import {
  type FileLinkRow,
  FileLinksEditor,
  fileLinksInput,
} from "@/components/file-links-editor"
import { LanguagePicker } from "@/components/language-picker"
import { PageHeader } from "@/components/page-header"
import type { Option } from "@/components/picker"
import { FilePicker } from "@/components/pickers"
import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { textProps } from "@/lib/languages"
import { call, idParam, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

/** `/artists/create` and `/artists/:id/update`. */
export function ArtistForm() {
  const { id } = useParams()
  const artistId = idParam(id)
  // A form starts from fresh data (frontend.md, "Commands")
  const { data: artist, error } = useCommand(
    "artist",
    { id: artistId ?? 0 },
    { fresh: true, enabled: artistId !== null }
  )

  if ((id !== undefined && artistId === null) || artist === null) {
    return <NotFound />
  }

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Artists", to: "/artists" },
          ...(id
            ? [
                {
                  label: artist ? `${artist.name} (${artist.id})` : id,
                  lang: artist?.nameLanguage,
                  to: `/artists/${id}`,
                },
                { label: "Update" },
              ]
            : [{ label: "New" }]),
        ]}
      />
      <div className="max-w-4xl p-4">
        {error ? (
          <QueryError error={error} title="Can't load the artist" />
        ) : id && !artist ? (
          <Skeleton className="h-40 w-full" />
        ) : (
          // Keyed, so a form for another artist, or a new one, starts afresh
          <Form
            key={artist?.id ?? "new"}
            id={artist?.id}
            initialName={artist?.name ?? ""}
            initialLanguage={artist?.nameLanguage ?? null}
            initialNames={(artist?.alternateNames ?? []).map(alternateNameRow)}
            mainNameUses={artist?.mainNameUses ?? 0}
            initialPicture={
              artist?.picture
                ? { id: String(artist.picture.id), name: artist.picture.path }
                : null
            }
            initialLinks={(artist?.files ?? []).map((link) => ({
              file: { id: String(link.file.id), name: link.file.path },
              note: link.note ?? "",
            }))}
          />
        )}
      </div>
    </>
  )
}

/** What saving changes on the artist's links, which the form asks about. */
type Consequences = {
  /** Names in use that the save removes: their links show the main name */
  removed: AlternateNameRow[]
  /** A rename while links show the old name: keep it on them? */
  renamed: boolean
}

function Form({
  id,
  initialName,
  initialLanguage,
  initialNames,
  mainNameUses = 0,
  initialPicture,
  initialLinks,
}: {
  id?: number
  initialName: string
  initialLanguage: string | null
  initialNames: AlternateNameRow[]
  /** Links that show the main name, which a rename changes */
  mainNameUses?: number
  initialPicture: Option | null
  initialLinks: FileLinkRow[]
}) {
  const navigate = useNavigate()
  const [name, setName] = useState(initialName)
  const [nameLanguage, setNameLanguage] = useState(initialLanguage)
  const [names, setNames] = useState(initialNames)
  // Set by "Make main", which keeps every link showing what it showed
  const [keepOldName, setKeepOldName] = useState(false)
  const [picture, setPicture] = useState(initialPicture)
  const [links, setLinks] = useState(initialLinks)
  const [asking, setAsking] = useState<Consequences | null>(null)
  const save = useMutation({
    mutationFn: (keep: boolean) => {
      const input = {
        name,
        nameLanguage,
        alternateNames: artistNamesInput(names),
        keepOldName: keep,
        // Options' ids are strings, as the Picker's are
        pictureId: picture ? Number(picture.id) : null,
        files: fileLinksInput(links),
      }
      return id === undefined
        ? call("createArtist", input)
        : call("updateArtist", { id, ...input })
    },
  })

  /**
   * Swaps a name with the main name: the name and its language become the main
   * ones, and the main name takes its place. A real name stays a real name too.
   * Saving keeps the old main name on the links that show it.
   */
  function makeMain(row: AlternateNameRow) {
    const old = alternateNameRow({ name, language: nameLanguage })
    setNames(
      names.flatMap((r) =>
        r.key !== row.key ? [r] : row.realName ? [r, old] : [old]
      )
    )
    setName(row.name)
    setNameLanguage(row.language)
    setKeepOldName(true)
  }

  /** What the save would change on links, if anything needs asking. */
  function consequences(): Consequences | null {
    if (!id) return null
    const kept = new Set(names.map((row) => row.id))
    const removed = initialNames.filter(
      (row) =>
        row.id !== null &&
        !kept.has(row.id) &&
        (row.uses ?? 0) > 0 &&
        // Promoted: its links show the same name, now the main one
        cleanName(row.name) !== cleanName(name)
    )
    const renamed =
      !keepOldName &&
      mainNameUses > 0 &&
      cleanName(name) !== cleanName(initialName)
    return removed.length > 0 || renamed ? { removed, renamed } : null
  }

  async function saveArtist(keep: boolean) {
    try {
      const saved = await save.mutateAsync(keep)
      navigate(`/artists/${saved.id}`)
    } catch {
      // The form shows the error
    } finally {
      setAsking(null)
    }
  }

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        const ask = consequences()
        if (ask) setAsking(ask)
        else await saveArtist(keepOldName)
      }}
    >
      <FieldGroup>
        {save.error && <QueryError error={save.error} title="Can't save" />}
        <Field>
          <FieldLabel htmlFor="name">Name</FieldLabel>
          <div className="grid grid-cols-[1fr_14rem] gap-2">
            <Input
              id="name"
              value={name}
              required
              {...textProps(nameLanguage)}
              onChange={(e) => setName(e.target.value)}
            />
            <LanguagePicker
              aria-label="Language of the name"
              value={nameLanguage}
              onChange={setNameLanguage}
            />
          </div>
          {id && mainNameUses > 0 && (
            <FieldDescription>
              Shown on {plural(mainNameUses, "credit")}: every link to the
              artist that isn't credited with another name.
            </FieldDescription>
          )}
        </Field>
        <Field>
          <FieldLabel>Alternate names</FieldLabel>
          <AlternateNamesEditor
            main={name}
            names={names}
            onChange={setNames}
            realNames
            onMakeMain={makeMain}
          />
          <FieldDescription>
            Other spellings (ALIKA), initials (R. Fukui), names in other scripts
            (福居良) and real names. A release, track or credit can be credited
            with any of them, and searching finds the artist by any of them.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel htmlFor="picture">Picture</FieldLabel>
          <FilePicker
            id="picture"
            type="image"
            value={picture}
            onChange={setPicture}
          />
        </Field>
        <Field>
          <FieldLabel>Linked files</FieldLabel>
          <FileLinksEditor links={links} onChange={setLinks} />
        </Field>
        <div className="flex gap-2">
          <Button
            type="submit"
            disabled={save.isPending || !alternateNamesValid(name, names, true)}
          >
            Save
          </Button>
          <ButtonLink variant="outline" to={id ? `/artists/${id}` : "/artists"}>
            Cancel
          </ButtonLink>
        </div>
      </FieldGroup>
      <SaveDialog
        asking={asking}
        oldName={cleanName(initialName)}
        newName={cleanName(name)}
        mainNameUses={mainNameUses}
        busy={save.isPending}
        onCancel={() => setAsking(null)}
        onSave={saveArtist}
      />
    </form>
  )
}

/**
 * Asks before a save changes what links show: removing a name that's in use
 * credits its links with the main name, and a rename can keep the old name on
 * the links that show it.
 */
function SaveDialog({
  asking,
  oldName,
  newName,
  mainNameUses,
  busy,
  onCancel,
  onSave,
}: {
  asking: Consequences | null
  oldName: string
  newName: string
  mainNameUses: number
  busy: boolean
  onCancel: () => void
  onSave: (keepOldName: boolean) => void
}) {
  return (
    <AlertDialog
      open={asking !== null}
      onOpenChange={(open) => !open && onCancel()}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {asking?.renamed
              ? `Keep "${oldName}" where it's printed?`
              : "Remove names that are in use?"}
          </AlertDialogTitle>
          <AlertDialogDescription>
            {asking?.renamed
              ? `${plural(mainNameUses, "credit")} ${mainNameUses === 1 ? "shows" : "show"} "${oldName}". Keeping it credits them with it, as an alternate name. Otherwise they'll show "${newName}".`
              : `Their credits will show "${newName}" instead.`}
          </AlertDialogDescription>
        </AlertDialogHeader>
        {asking && asking.removed.length > 0 && (
          <ul className="flex list-disc flex-col gap-1 pl-4 text-xs">
            {asking.removed.map((row) => (
              <li key={row.key}>
                <span {...textProps(row.language)}>{row.name}</span> is credited{" "}
                {row.uses === 1 ? "once" : `${row.uses} times`}
                {asking.renamed && `: those will show "${newName}"`}
              </li>
            ))}
          </ul>
        )}
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          {asking?.renamed ? (
            <>
              <AlertDialogAction
                variant="outline"
                disabled={busy}
                onClick={() => onSave(false)}
              >
                Show "{newName}"
              </AlertDialogAction>
              <AlertDialogAction disabled={busy} onClick={() => onSave(true)}>
                Keep "{oldName}"
              </AlertDialogAction>
            </>
          ) : (
            <AlertDialogAction disabled={busy} onClick={() => onSave(false)}>
              Remove and save
            </AlertDialogAction>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

function plural(n: number, noun: string): string {
  return `${n} ${noun}${n === 1 ? "" : "s"}`
}
