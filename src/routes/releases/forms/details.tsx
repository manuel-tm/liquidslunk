import { useMutation } from "@apollo/client/react"
import { useState } from "react"

import {
  type AlternateTitleRow,
  AlternateTitlesEditor,
  alternateTitlesInput,
  alternateTitlesValid,
} from "@/components/alternate-titles-editor"
import { LanguagePicker } from "@/components/language-picker"
import { OrderedList } from "@/components/ordered-list"
import type { Option } from "@/components/picker"
import { ArtistPicker, LabelPicker, ListItemPicker } from "@/components/pickers"
import { QueryError } from "@/components/query-error"
import { SplitTitleOffer } from "@/components/title-split"
import { Button } from "@/components/ui/button"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { graphql } from "@/gql"
import { creditedInput } from "@/lib/credited"
import { textProps } from "@/lib/languages"

export type DetailsValues = {
  title: string
  titleLanguage: string | null
  alternateTitles: AlternateTitleRow[]
  released: string
  variousArtists: boolean
  catalogNumber: string
  artists: Option[]
  labels: Option[]
  formats: Option[]
  media: Option[]
}

export const EMPTY_DETAILS: DetailsValues = {
  title: "",
  titleLanguage: null,
  alternateTitles: [],
  released: "",
  variousArtists: false,
  catalogNumber: "",
  artists: [],
  labels: [],
  formats: [],
  media: [],
}

const CreateRelease = graphql(`
  mutation CreateRelease($input: ReleaseInput!) {
    createRelease(input: $input) {
      id
    }
  }
`)

const UpdateRelease = graphql(`
  mutation UpdateRelease($id: ID!, $input: ReleaseInput!) {
    updateRelease(id: $id, input: $input) {
      id
      title
      released
      catalogNumber
      variousArtists
    }
  }
`)

/**
 * A release's details (slunk's design §7.6): title, artists, various artists,
 * date, formats, media, labels and catalog number. Without `releaseId`, it
 * creates the release. The import also offers to split a title with " = ",
 * and types the tag names that picked no artist into Artists, in turn
 * (slunk's 3_alternate_names.md §7.7).
 */
export function ReleaseDetailsForm({
  releaseId,
  initial,
  submitLabel = "Save",
  offerSplit = false,
  typedArtists,
  onSaved,
}: {
  releaseId?: string
  initial: DetailsValues
  submitLabel?: string
  offerSplit?: boolean
  typedArtists?: string[]
  onSaved: (id: string) => void
}) {
  const [values, setValues] = useState(initial)
  const set = (changes: Partial<DetailsValues>) =>
    setValues((v) => ({ ...v, ...changes }))
  const [create, created] = useMutation(CreateRelease)
  const [update, updated] = useMutation(UpdateRelease)
  const error = created.error ?? updated.error

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        const input = {
          title: values.title,
          titleLanguage: values.titleLanguage,
          alternateTitles: alternateTitlesInput(values.alternateTitles),
          released: values.released.trim() || null,
          variousArtists: values.variousArtists,
          catalogNumber: values.catalogNumber.trim() || null,
          artists: creditedInput(values.artists),
          labelIds: values.labels.map((l) => l.id),
          formatIds: values.formats.map((f) => f.id),
          mediumIds: values.media.map((m) => m.id),
        }
        try {
          const saved = releaseId
            ? (await update({ variables: { id: releaseId, input } })).data!
                .updateRelease
            : (await create({ variables: { input } })).data!.createRelease
          onSaved(saved.id)
        } catch {
          // The form shows the error
        }
      }}
    >
      <FieldGroup className="max-w-2xl">
        {error && <QueryError error={error} title="Can't save" />}
        <Field>
          <FieldLabel htmlFor="title">Title</FieldLabel>
          <div className="grid grid-cols-[1fr_14rem] gap-2">
            <Input
              id="title"
              required
              value={values.title}
              {...textProps(values.titleLanguage)}
              onChange={(e) => set({ title: e.target.value })}
            />
            <LanguagePicker
              aria-label="Language of the title"
              value={values.titleLanguage}
              onChange={(titleLanguage) => set({ titleLanguage })}
            />
          </div>
          {offerSplit && (
            <SplitTitleOffer
              values={{
                title: values.title,
                language: values.titleLanguage,
                alternateTitles: values.alternateTitles,
              }}
              onChange={({ title, language, alternateTitles }) =>
                set({ title, titleLanguage: language, alternateTitles })
              }
            />
          )}
          <FieldDescription>
            The language is also the tracks' and discs', unless they have their
            own.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel>Alternate titles</FieldLabel>
          <AlternateTitlesEditor
            main={values.title}
            titles={values.alternateTitles}
            onChange={(alternateTitles) => set({ alternateTitles })}
          />
          <FieldDescription>
            The title printed in another script or language (シーナリィ), or a
            translation added here, which isn't printed. Searching finds the
            release by any of them.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel>Artists</FieldLabel>
          <OrderedList
            items={values.artists}
            onChange={(artists) => set({ artists })}
            picker={(p) => <ArtistPicker {...p} />}
            credited
            typed={typedArtists}
          />
          <FieldDescription>
            Who the release is by, each as printed: blank for the main name. A
            compilation uses a "Various Artists" artist.
          </FieldDescription>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="various-artists"
            checked={values.variousArtists}
            onCheckedChange={(variousArtists) => set({ variousArtists })}
          />
          <FieldContent>
            <FieldLabel htmlFor="various-artists">Various artists</FieldLabel>
            <FieldDescription>
              Each track names its own performers: compilations, soundtracks,
              cast recordings.
            </FieldDescription>
          </FieldContent>
        </Field>
        <Field>
          <FieldLabel htmlFor="released">Released</FieldLabel>
          <Input
            id="released"
            className="w-40"
            placeholder="2006-01-10"
            value={values.released}
            onChange={(e) => set({ released: e.target.value })}
          />
          <FieldDescription>
            2006, 2006-01 or 2006-01-10, with only the parts you know.
          </FieldDescription>
        </Field>
        <div className="grid gap-4 md:grid-cols-2">
          <Field>
            <FieldLabel>Formats</FieldLabel>
            <OrderedList
              items={values.formats}
              onChange={(formats) => set({ formats })}
              picker={(p) => <ListItemPicker kind="release_formats" {...p} />}
            />
          </Field>
          <Field>
            <FieldLabel>Media</FieldLabel>
            <OrderedList
              items={values.media}
              onChange={(media) => set({ media })}
              picker={(p) => <ListItemPicker kind="release_media" {...p} />}
            />
          </Field>
        </div>
        <Field>
          <FieldLabel>Labels</FieldLabel>
          <OrderedList
            items={values.labels}
            onChange={(labels) => set({ labels })}
            picker={(p) => <LabelPicker {...p} />}
          />
        </Field>
        <Field>
          <FieldLabel htmlFor="catalog-number">Catalog number</FieldLabel>
          <Input
            id="catalog-number"
            className="w-60"
            placeholder="B0019141-02"
            value={values.catalogNumber}
            onChange={(e) => set({ catalogNumber: e.target.value })}
          />
          <FieldDescription>
            As printed on the spine or back cover.
          </FieldDescription>
        </Field>
        <div>
          <Button
            type="submit"
            disabled={
              created.loading ||
              updated.loading ||
              !alternateTitlesValid(values.title, values.alternateTitles)
            }
          >
            {submitLabel}
          </Button>
        </div>
      </FieldGroup>
    </form>
  )
}
