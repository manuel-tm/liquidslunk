import { useMutation } from "@tanstack/react-query"
import { useState } from "react"
import { useNavigate, useParams, useSearchParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import {
  type CreditLine,
  CreditsEditor,
  creditsInput,
  creditsValid,
} from "@/components/credits-editor"
import {
  type FileLinkRow,
  FileLinksEditor,
  fileLinksInput,
} from "@/components/file-links-editor"
import { OrderedList } from "@/components/ordered-list"
import { PageHeader } from "@/components/page-header"
import type { Option } from "@/components/picker"
import { ArtistPicker, FilePicker, ReleasePicker } from "@/components/pickers"
import { QueryError } from "@/components/query-error"
import {
  type TagRow,
  TagsEditor,
  tagRows,
  tagValuesInput,
} from "@/components/tags-editor"
import { Button } from "@/components/ui/button"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  type AlternateTitleRow,
  alternateTitleRow,
  AlternateTitlesEditor,
  alternateTitlesInput,
  alternateTitlesValid,
} from "@/components/alternate-titles-editor"
import { LanguagePicker } from "@/components/language-picker"
import { Skeleton } from "@/components/ui/skeleton"
import type { TrackInput } from "@/bindings"
import { creditedInput, creditedOption } from "@/lib/credited"
import { textProps } from "@/lib/languages"
import { call, idParam, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

type Values = {
  release: Option | null
  file: Option | null
  cdNumber: number
  trackNumber: number | ""
  title: string
  /** Null: the release's */
  titleLanguage: string | null
  alternateTitles: AlternateTitleRow[]
  artists: Option[]
  features: Option[]
  credits: CreditLine[]
  links: FileLinkRow[]
  tags: TagRow[]
}

/**
 * `/tracks/create` and `/tracks/:id/update`. `?release=` and `?file=` prefill a
 * new track.
 */
export function TrackForm() {
  const { id } = useParams()
  const [params] = useSearchParams()
  const trackId = id === undefined ? undefined : idParam(id)
  // A form starts from fresh data (frontend.md, "Commands")
  const { data, error } = useCommand(
    "trackForm",
    trackId === undefined
      ? {
          release: idParam(params.get("release") ?? undefined) ?? undefined,
          file: idParam(params.get("file") ?? undefined) ?? undefined,
        }
      : { id: trackId ?? 0 },
    { fresh: true, enabled: trackId !== null }
  )

  if (trackId === null || (trackId !== undefined && data && !data.track)) {
    return <NotFound />
  }
  const track = data?.track
  // Options' ids are strings, as the pickers' are
  const option = (id: number, name: string) => ({ id: String(id), name })

  const initial: Values | null = !data
    ? null
    : track
      ? {
          release: option(track.release.id, track.release.title),
          file: option(track.file.id, track.file.path),
          cdNumber: track.cdNumber,
          trackNumber: track.trackNumber,
          title: track.title,
          titleLanguage: track.titleLanguage,
          alternateTitles: track.alternateTitles.map(alternateTitleRow),
          artists: track.artists.map(creditedOption),
          features: track.features.map(creditedOption),
          credits: track.credits.map(({ role, artists }) => ({
            role,
            artists: artists.map(creditedOption),
          })),
          links: track.files.map((link) => ({
            file: option(link.file.id, link.file.path),
            note: link.note ?? "",
          })),
          tags: tagRows(track.tags),
        }
      : {
          release: data.prefillRelease
            ? option(data.prefillRelease.id, data.prefillRelease.title)
            : null,
          file: data.prefillFile
            ? option(data.prefillFile.id, data.prefillFile.path)
            : null,
          cdNumber: 1,
          trackNumber: "",
          title:
            data.prefillFile?.name
              .replace(/\.[^.]+$/, "")
              .replace(/^\d+[\s._-]+/, "") ?? "",
          titleLanguage: null,
          alternateTitles: [],
          artists: [],
          features: [],
          credits: [],
          links: [],
          tags: [],
        }

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Tracks", to: "/tracks" },
          ...(id
            ? [
                {
                  label: track ? `${track.title} (${track.id})` : id,
                  to: `/tracks/${id}`,
                },
                { label: "Update" },
              ]
            : [{ label: "New" }]),
        ]}
      />
      <div className="max-w-2xl p-4">
        {error ? (
          <QueryError error={error} title="Can't load the track" />
        ) : !initial ? (
          <Skeleton className="h-64 w-full" />
        ) : (
          <Form id={trackId} initial={initial} />
        )}
      </div>
    </>
  )
}

function Form({ id, initial }: { id?: number; initial: Values }) {
  const navigate = useNavigate()
  const [values, setValues] = useState(initial)
  const set = (changes: Partial<Values>) =>
    setValues((v) => ({ ...v, ...changes }))
  const save = useMutation({
    mutationFn: (input: TrackInput) =>
      id === undefined
        ? call("createTrack", input)
        : call("updateTrack", { id, ...input }),
  })

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        if (!values.release || !values.file || values.trackNumber === "") return
        const input = {
          // Options' ids are strings, as the pickers' are
          releaseId: Number(values.release.id),
          fileId: Number(values.file.id),
          cdNumber: values.cdNumber,
          trackNumber: values.trackNumber,
          title: values.title,
          titleLanguage: values.titleLanguage,
          alternateTitles: alternateTitlesInput(values.alternateTitles),
          artists: creditedInput(values.artists),
          features: creditedInput(values.features),
          credits: creditsInput(values.credits),
          files: fileLinksInput(values.links),
          tags: tagValuesInput(values.tags),
        }
        try {
          const saved = await save.mutateAsync(input)
          navigate(`/tracks/${saved.id}`)
        } catch {
          // The form shows the error
        }
      }}
    >
      <FieldGroup>
        {save.error && <QueryError error={save.error} title="Can't save" />}
        <Field>
          <FieldLabel htmlFor="release">Release</FieldLabel>
          <ReleasePicker
            id="release"
            value={values.release}
            onChange={(release) => set({ release })}
          />
        </Field>
        <Field>
          <FieldLabel htmlFor="file">Audio file</FieldLabel>
          <FilePicker
            id="file"
            type="music"
            withoutTrack
            value={values.file}
            onChange={(file) => set({ file })}
          />
          <FieldDescription>
            A registered audio file that isn't a track yet. Its duration is read
            from it.
          </FieldDescription>
        </Field>
        <div className="grid grid-cols-[6rem_6rem_1fr] gap-4">
          <Field>
            <FieldLabel htmlFor="disc">Disc</FieldLabel>
            <Input
              id="disc"
              type="number"
              min={1}
              required
              value={values.cdNumber}
              onChange={(e) => set({ cdNumber: Number(e.target.value) })}
            />
          </Field>
          <Field>
            <FieldLabel htmlFor="number">Number</FieldLabel>
            <Input
              id="number"
              type="number"
              min={1}
              required
              value={values.trackNumber}
              onChange={(e) =>
                set({
                  trackNumber:
                    e.target.value === "" ? "" : Number(e.target.value),
                })
              }
            />
          </Field>
          <Field>
            <FieldLabel htmlFor="title">Title</FieldLabel>
            <Input
              id="title"
              required
              value={values.title}
              {...textProps(values.titleLanguage)}
              onChange={(e) => set({ title: e.target.value })}
            />
          </Field>
        </div>
        <Field>
          <FieldLabel htmlFor="title-language">Title language</FieldLabel>
          <div className="w-56">
            <LanguagePicker
              id="title-language"
              placeholder="The release's"
              value={values.titleLanguage}
              onChange={(titleLanguage) => set({ titleLanguage })}
            />
          </div>
          <FieldDescription>
            Blank for the release's. Set it for a track in another language, as
            on a compilation.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel>Alternate titles</FieldLabel>
          <AlternateTitlesEditor
            main={values.title}
            titles={values.alternateTitles}
            onChange={(alternateTitles) => set({ alternateTitles })}
          />
        </Field>
        <Field>
          <FieldLabel>Artists</FieldLabel>
          <OrderedList
            items={values.artists}
            onChange={(artists) => set({ artists })}
            picker={(p) => <ArtistPicker {...p} />}
            credited
          />
          <FieldDescription>
            Who performs it. On a regular release, leave it empty and the
            release's artists perform it.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel>Features</FieldLabel>
          <OrderedList
            items={values.features}
            onChange={(features) => set({ features })}
            picker={(p) => <ArtistPicker {...p} />}
            credited
          />
        </Field>
        <Field>
          <FieldLabel>Credits</FieldLabel>
          <CreditsEditor
            credits={values.credits}
            onChange={(credits) => set({ credits })}
          />
        </Field>
        <Field>
          <FieldLabel>Linked files</FieldLabel>
          <FileLinksEditor
            links={values.links}
            onChange={(links) => set({ links })}
          />
        </Field>
        <FieldSet>
          <FieldLegend variant="label">Tags</FieldLegend>
          <TagsEditor rows={values.tags} onChange={(tags) => set({ tags })} />
        </FieldSet>
        <div className="flex gap-2">
          <Button
            type="submit"
            disabled={
              save.isPending ||
              !values.release ||
              !values.file ||
              !creditsValid(values.credits) ||
              !alternateTitlesValid(values.title, values.alternateTitles)
            }
          >
            Save
          </Button>
          <ButtonLink variant="outline" to={id ? `/tracks/${id}` : "/tracks"}>
            Cancel
          </ButtonLink>
        </div>
      </FieldGroup>
    </form>
  )
}
