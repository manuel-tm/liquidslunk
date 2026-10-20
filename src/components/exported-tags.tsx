import { ChevronRightIcon } from "lucide-react"
import { useState, type ReactNode } from "react"
import { Link } from "react-router"

import { QueryError } from "@/components/query-error"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { textProps } from "@/lib/languages"
import { useCommand } from "@/lib/query"

/**
 * "Tags as exported", on the track page (phase 0 of 4_external_libraries.md):
 * the track's tags as every external library writes them, and the path the
 * default template gives it. Closed by default, and read only once opened.
 */
export function ExportedTags({
  trackId,
  titleLanguage,
  releaseLanguage,
}: {
  trackId: number
  /** The track's title's language, or else its release's */
  titleLanguage: string | null
  releaseLanguage: string | null
}) {
  const [open, setOpen] = useState(false)
  const { data: tags, error } = useCommand(
    "exportedTags",
    { id: trackId },
    { enabled: open }
  )

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger
        render={<Button variant="ghost" size="sm" className="-ml-2" />}
      >
        <ChevronRightIcon
          data-icon="inline-start"
          className={open ? "rotate-90" : undefined}
        />
        Tags as exported
      </CollapsibleTrigger>
      <CollapsibleContent className="flex flex-col gap-2 pt-2">
        <p className="text-muted-foreground">
          What an external library will write in this track&apos;s file,
          whatever its format, and where a folder library that keeps the
          original files will put it, by the default template. Fields with no
          value aren&apos;t written.
        </p>
        {error ? (
          <QueryError error={error} title="Can't work out the tags" />
        ) : !tags ? (
          <Skeleton className="h-64 w-full" />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-40">Field</TableHead>
                <TableHead>Value</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <Field label="Title">
                <span {...textProps(titleLanguage)}>{tags.title}</span>
              </Field>
              <Field label="Artist">{tags.artist}</Field>
              <Field label="Artists">
                {tags.artists.length > 0 && <Values values={tags.artists} />}
              </Field>
              <Field label="Album">
                <span {...textProps(releaseLanguage)}>{tags.album}</span>
              </Field>
              <Field label="Album artist">{tags.albumArtist}</Field>
              <Field label="Compilation">{tags.compilation && "Yes"}</Field>
              <Field label="Date">{tags.date}</Field>
              <Field label="Track">
                {tags.trackNumber} of {tags.trackTotal}
              </Field>
              <Field label="Disc">
                {tags.discNumber} of {tags.discTotal}
              </Field>
              <Field label="Disc title">{tags.discTitle}</Field>
              <Field label="Label">
                {tags.labels.length > 0 && <Values values={tags.labels} />}
              </Field>
              <Field label="Catalog number">{tags.catalogNumber}</Field>
              <Field label="Media">
                {tags.media.length > 0 && <Values values={tags.media} />}
              </Field>
              <Field label="Release type">
                {tags.releaseTypes.length > 0 && (
                  <Values values={tags.releaseTypes} />
                )}
              </Field>
              <Field label="Artist sort">{tags.artistSort}</Field>
              <Field label="Album artist sort">{tags.albumArtistSort}</Field>
              <Field label="Album sort">{tags.albumSort}</Field>
              <Field label="Title sort">{tags.titleSort}</Field>
              <Field label="Cover">
                {tags.cover && (
                  <Link
                    to={`/files/${tags.cover.id}`}
                    className="font-mono break-all hover:underline"
                  >
                    {tags.cover.path}
                  </Link>
                )}
              </Field>
              <Field label="File">
                <span className="font-mono break-all" dir="auto">
                  {tags.path}
                </span>
              </Field>
            </TableBody>
          </Table>
        )}
      </CollapsibleContent>
    </Collapsible>
  )
}

/** A field's row, left out when it has no value, since it isn't written. */
function Field({ label, children }: { label: string; children: ReactNode }) {
  if (children === null || children === false || children === "") return null
  return (
    <TableRow>
      <TableCell className="align-top text-muted-foreground">{label}</TableCell>
      <TableCell className="whitespace-normal" dir="auto">
        {children}
      </TableCell>
    </TableRow>
  )
}

/** A field written as several values, one per value. */
function Values({ values }: { values: string[] }) {
  return (
    <span className="flex flex-wrap gap-1">
      {values.map((value, index) => (
        <Badge key={index} variant="secondary" dir="auto">
          {value}
        </Badge>
      ))}
    </span>
  )
}
