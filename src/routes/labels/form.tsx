import { useMutation } from "@tanstack/react-query"
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
import { textProps } from "@/lib/languages"
import { call, idParam, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

/** `/labels/create` and `/labels/:id/update`: name and logo. */
export function LabelForm() {
  const { id } = useParams()
  const labelId = idParam(id)
  // A form starts from fresh data (design §7.2)
  const { data: label, error } = useCommand(
    "label",
    { id: labelId ?? 0 },
    { fresh: true, enabled: labelId !== null }
  )

  if ((id !== undefined && labelId === null) || label === null) {
    return <NotFound />
  }

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
          // Keyed, so a form for another label, or a new one, starts afresh
          <Form
            key={label?.id ?? "new"}
            id={label?.id}
            initialName={label?.name ?? ""}
            initialLanguage={label?.nameLanguage ?? null}
            initialNames={(label?.alternateNames ?? []).map(alternateNameRow)}
            initialPicture={
              label?.picture
                ? { id: String(label.picture.id), name: label.picture.path }
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
  id?: number
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
  const save = useMutation({
    mutationFn: () => {
      const input = {
        name,
        nameLanguage,
        alternateNames: labelNamesInput(names),
        // Pickers' ids are strings, for the forms that still send GraphQL
        pictureId: picture ? Number(picture.id) : null,
      }
      return id === undefined
        ? call("createLabel", input)
        : call("updateLabel", { id, ...input })
    },
  })

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          const saved = await save.mutateAsync()
          navigate(`/labels/${saved.id}`)
        } catch {
          // The form shows the error
        }
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
              save.isPending || !alternateNamesValid(name, names, false)
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
