import { useMutation, useQuery } from "@apollo/client/react"
import { useState } from "react"
import { useNavigate, useParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { FILE_TYPE_LABELS } from "@/components/file-type"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import {
  Field,
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
import { graphql } from "@/gql"
import type { FileType, FileUpdateQuery } from "@/gql/graphql"
import { NotFound } from "@/routes/not-found"

const FileUpdateQueryDocument = graphql(`
  query FileUpdate($id: ID!) {
    file(id: $id) {
      id
      name
      path
      type
    }
  }
`)

const UpdateFileMutation = graphql(`
  mutation UpdateFile($id: ID!, $input: UpdateFileInput!) {
    updateFile(id: $id, input: $input) {
      id
      path
      name
      type
      sizeBytes
      mtime
      sha256
      updatedAt
      onDisk
    }
  }
`)

/** `/files/:id/update`: relinks a file that moved, or changes its type. */
export function FileUpdate() {
  const { id = "" } = useParams()
  const { data, error } = useQuery(FileUpdateQueryDocument, {
    variables: { id },
  })

  if (data && !data.file) return <NotFound />
  const file = data?.file

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Files", to: "/files" },
          {
            label: file ? `${file.name} (${file.id})` : id,
            to: `/files/${id}`,
          },
          { label: "Update" },
        ]}
      />
      <div className="max-w-xl p-4">
        {error ? (
          <QueryError error={error} title="Can't load the file" />
        ) : !file ? (
          <Skeleton className="h-40 w-full" />
        ) : (
          <FileForm file={file} />
        )}
      </div>
    </>
  )
}

function FileForm({ file }: { file: NonNullable<FileUpdateQuery["file"]> }) {
  const navigate = useNavigate()
  const [path, setPath] = useState(file.path)
  const [type, setType] = useState<FileType>(file.type)
  const [updateFile, { loading, error }] = useMutation(UpdateFileMutation)

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          await updateFile({
            variables: { id: file.id, input: { path, type } },
          })
        } catch {
          return // The form shows the error
        }
        navigate(`/files/${file.id}`)
      }}
    >
      <FieldGroup>
        {error && <QueryError error={error} title="Can't save" />}
        <Field>
          <FieldLabel htmlFor="path">Path</FieldLabel>
          <Input
            id="path"
            value={path}
            required
            className="font-mono"
            onChange={(event) => setPath(event.target.value)}
          />
          <FieldDescription>
            Relative to the folder that holds slunk.db. Change it to relink a
            file that moved: liquidslunk reads its size, modification time and
            hash again.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel htmlFor="type">Type</FieldLabel>
          <Select
            items={FILE_TYPE_LABELS}
            value={type}
            onValueChange={(value) => value && setType(value as FileType)}
          >
            <SelectTrigger id="type" className="w-40">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {Object.entries(FILE_TYPE_LABELS).map(([value, label]) => (
                <SelectItem key={value} value={value}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <FieldDescription>
            A track's audio file has to stay Music, and a cover or picture has
            to stay an Image.
          </FieldDescription>
        </Field>
        <div className="flex gap-2">
          <Button type="submit" disabled={loading}>
            Save
          </Button>
          <ButtonLink variant="outline" to={`/files/${file.id}`}>
            Cancel
          </ButtonLink>
        </div>
      </FieldGroup>
    </form>
  )
}
