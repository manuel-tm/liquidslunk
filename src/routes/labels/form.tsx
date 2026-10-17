import { useMutation, useQuery } from "@apollo/client/react"
import { useState } from "react"
import { useNavigate, useParams } from "react-router"

import {
  type AlternateNameRow,
  alternateNameRow,
  AlternateNamesEditor,
  alternateNamesValid,
  labelNamesInput,
} from "@/components/alternate-names-editor"
import { ButtonLink } from "@/components/button-link"
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
import { graphql } from "@/gql"
import { textProps } from "@/lib/languages"
import { NotFound } from "@/routes/not-found"

const LabelFormQuery = graphql(`
  query LabelForm($id: ID!) {
    label(id: $id) {
      id
      name
      nameLanguage
      alternateNames {
        name
        language
        note
      }
      picture {
        id
        path
      }
    }
  }
`)

const CreateLabel = graphql(`
  mutation CreateLabel($input: LabelInput!) {
    createLabel(input: $input) {
      id
    }
  }
`)

const UpdateLabel = graphql(`
  mutation UpdateLabel($id: ID!, $input: LabelInput!) {
    updateLabel(id: $id, input: $input) {
      id
      name
      nameLanguage
      alternateNames {
        name
        language
        note
      }
      picture {
        id
      }
    }
  }
`)

/** `/labels/create` and `/labels/:id/update`: name and logo. */
export function LabelForm() {
  const { id } = useParams()
  const { data, error } = useQuery(LabelFormQuery, {
    variables: { id: id ?? "" },
    skip: id === undefined,
  })

  if (data && !data.label) return <NotFound />
  const label = data?.label

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Labels", to: "/labels" },
          ...(id
            ? [
                {
                  label: label ? `${label.name} (${label.id})` : id,
                  lang: label?.nameLanguage,
                  to: `/labels/${id}`,
                },
                { label: "Update" },
              ]
            : [{ label: "New" }]),
        ]}
      />
      <div className="max-w-3xl p-4">
        {error ? (
          <QueryError error={error} title="Can't load the label" />
        ) : id && !label ? (
          <Skeleton className="h-40 w-full" />
        ) : (
          <Form
            id={id}
            initialName={label?.name ?? ""}
            initialLanguage={label?.nameLanguage ?? null}
            initialNames={(label?.alternateNames ?? []).map(alternateNameRow)}
            initialPicture={
              label?.picture
                ? { id: label.picture.id, name: label.picture.path }
                : null
            }
          />
        )}
      </div>
    </>
  )
}

function Form({
  id,
  initialName,
  initialLanguage,
  initialNames,
  initialPicture,
}: {
  id?: string
  initialName: string
  initialLanguage: string | null
  initialNames: AlternateNameRow[]
  initialPicture: Option | null
}) {
  const navigate = useNavigate()
  const [name, setName] = useState(initialName)
  const [nameLanguage, setNameLanguage] = useState(initialLanguage)
  const [names, setNames] = useState(initialNames)
  const [picture, setPicture] = useState(initialPicture)
  const [create, created] = useMutation(CreateLabel)
  const [update, updated] = useMutation(UpdateLabel)
  const error = created.error ?? updated.error

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        const input = {
          name,
          nameLanguage,
          alternateNames: labelNamesInput(names),
          pictureId: picture?.id ?? null,
        }
        try {
          const saved = id
            ? (await update({ variables: { id, input } })).data!.updateLabel
            : (await create({ variables: { input } })).data!.createLabel
          navigate(`/labels/${saved.id}`)
        } catch {
          // The form shows the error
        }
      }}
    >
      <FieldGroup>
        {error && <QueryError error={error} title="Can't save" />}
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
          <FieldDescription>
            As printed on the sleeve. Two labels can share a name.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel>Alternate names</FieldLabel>
          <AlternateNamesEditor
            main={name}
            names={names}
            onChange={setNames}
            realNames={false}
          />
          <FieldDescription>
            Names in other scripts (トリオレコード) and former names. Searching
            finds the label by any of them.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel htmlFor="logo">Logo</FieldLabel>
          <FilePicker
            id="logo"
            type="image"
            value={picture}
            onChange={setPicture}
          />
        </Field>
        <div className="flex gap-2">
          <Button
            type="submit"
            disabled={
              created.loading ||
              updated.loading ||
              !alternateNamesValid(name, names, false)
            }
          >
            Save
          </Button>
          <ButtonLink variant="outline" to={id ? `/labels/${id}` : "/labels"}>
            Cancel
          </ButtonLink>
        </div>
      </FieldGroup>
    </form>
  )
}
