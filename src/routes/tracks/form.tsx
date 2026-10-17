import { useMutation, useQuery } from "@apollo/client/react"
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
import { graphql } from "@/gql"
import { creditedInput, creditedOption } from "@/lib/credited"
import { textProps } from "@/lib/languages"
import { NotFound } from "@/routes/not-found"

const TrackFormQuery = graphql(`
  query TrackForm(
    $id: ID!
    $isEdit: Boolean!
    $releaseId: ID!
    $withRelease: Boolean!
    $fileId: ID!
    $withFile: Boolean!
  ) {
    track(id: $id) @include(if: $isEdit) {
      id
      title
      titleLanguage
      alternateTitles {
        title
        language
        printed
      }
      cdNumber
      trackNumber
      release {
        id
        name: title
      }
      file {
        id
        name: path
      }
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
      features {
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
      credits {
        role
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
      }
      files {
        note
        file {
          id
          name: path
        }
      }
      tags {
        tag {
          id
          type
          name
          multiple
        }
        values
      }
    }
    prefillRelease: release(id: $releaseId) @include(if: $withRelease) {
      id
      name: title
    }
    prefillFile: file(id: $fileId) @include(if: $withFile) {
      id
      name: path
      fileName: name
    }
  }
`)

const CreateTrack = graphql(`
  mutation CreateTrack($input: TrackInput!) {
    createTrack(input: $input) {
      id
    }
  }
`)

const UpdateTrack = graphql(`
  mutation UpdateTrack($id: ID!, $input: TrackInput!) {
    updateTrack(id: $id, input: $input) {
      id
      title
    }
  }
`)

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
 * `/tracks/create` and `/tracks/:id/update` (slunk's design §7.8). `?release=` and
 * `?file=` prefill a new track.
 */
export function TrackForm() {
  const { id } = useParams()
  const [params] = useSearchParams()
  const releaseId = params.get("release")
  const fileId = params.get("file")
  const { data, error } = useQuery(TrackFormQuery, {
    variables: {
      id: id ?? "",
      isEdit: id !== undefined,
      releaseId: releaseId ?? "",
      withRelease: releaseId !== null,
      fileId: fileId ?? "",
      withFile: fileId !== null,
    },
  })

  if (id && data && !data.track) return <NotFound />
  const track = data?.track

  const initial: Values | null = !data
    ? null
    : track
      ? {
          release: track.release,
          file: track.file,
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
            file: link.file,
            note: link.note ?? "",
          })),
          tags: tagRows(track.tags),
        }
      : {
          release: data.prefillRelease ?? null,
          file: data.prefillFile ?? null,
          cdNumber: 1,
          trackNumber: "",
          title:
            data.prefillFile?.fileName
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
          <Form id={id} initial={initial} />
        )}
      </div>
    </>
  )
}

function Form({ id, initial }: { id?: string; initial: Values }) {
  const navigate = useNavigate()
  const [values, setValues] = useState(initial)
  const set = (changes: Partial<Values>) =>
    setValues((v) => ({ ...v, ...changes }))
  const [create, created] = useMutation(CreateTrack)
  const [update, updated] = useMutation(UpdateTrack)
  const error = created.error ?? updated.error

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        if (!values.release || !values.file || values.trackNumber === "") return
        const input = {
          releaseId: values.release.id,
          fileId: values.file.id,
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
          const saved = id
            ? (await update({ variables: { id, input } })).data!.updateTrack
            : (await create({ variables: { input } })).data!.createTrack
          navigate(`/tracks/${saved.id}`)
        } catch {
          // The form shows the error
        }
      }}
    >
      <FieldGroup>
        {error && <QueryError error={error} title="Can't save" />}
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
              created.loading ||
              updated.loading ||
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
