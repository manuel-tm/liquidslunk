import { useMutation } from "@tanstack/react-query"
import { CircleAlertIcon, XIcon } from "lucide-react"
import { useState } from "react"

import type { Option } from "@/components/picker"
import { ArtistPicker } from "@/components/pickers"
import { Button } from "@/components/ui/button"
import { creatable, type PendingArtist, pendingReason } from "@/lib/import"
import { call } from "@/lib/query"
import { cn } from "@/lib/utils"

/**
 * A tag's name that picked no artist, in an artists list, marked so it isn't
 * taken for a picked one (frontend.md, "Import"). "Create" makes an artist
 * with the name, unless several have it already, and "Choose" opens the
 * artist picker with the name typed in. Either settles it with that artist,
 * and remove settles it with none. `compact` is for a table row.
 */
export function PendingArtistItem({
  artist,
  exclude,
  compact = false,
  onSettle,
}: {
  artist: PendingArtist
  /** Ids the picker leaves out, like those already in the list */
  exclude: string[]
  compact?: boolean
  onSettle: (artist: Option | null) => void
}) {
  const [choosing, setChoosing] = useState(false)
  const create = useMutation({
    mutationFn: () => call("createArtist", { name: artist.name }),
  })
  const reason = pendingReason(artist)

  return (
    <div
      className={cn(
        "flex flex-wrap items-center gap-x-2 gap-y-1 border border-dashed border-destructive/60 px-2 py-1 text-xs",
        compact && "py-1.5"
      )}
    >
      <CircleAlertIcon className="size-3.5 shrink-0 text-destructive" />
      {choosing ? (
        <div className="min-w-40 flex-1">
          <ArtistPicker
            value={null}
            exclude={exclude}
            defaultQuery={artist.name}
            autoOpen
            aria-label={`Artist for ${artist.name}`}
            onChange={(option) => option && onSettle(option)}
          />
        </div>
      ) : (
        <span className="flex-1">
          <span dir="auto" className="font-medium">
            {artist.name}
          </span>
          {!compact && <span className="text-muted-foreground"> · </span>}
          <span className={cn("text-muted-foreground", compact && "block")}>
            {compact ? reason : `From the tags. ${reason}.`}
          </span>
        </span>
      )}
      <div
        className={cn(
          "flex shrink-0 gap-1",
          compact && !choosing && "basis-full"
        )}
      >
        {!choosing && creatable(artist) && (
          <Button
            type="button"
            size="xs"
            variant="outline"
            disabled={create.isPending}
            onClick={async () => {
              try {
                const created = await create.mutateAsync()
                onSettle({
                  // A string, as pickers' options' ids are
                  id: String(created.id),
                  name: created.name,
                  lang: created.nameLanguage,
                })
              } catch {
                // Shown below
              }
            }}
          >
            {compact ? "Create" : "Create artist"}
          </Button>
        )}
        {!choosing && (
          <Button
            type="button"
            size="xs"
            variant="outline"
            onClick={() => setChoosing(true)}
          >
            Choose…
          </Button>
        )}
        <Button
          type="button"
          size="icon-xs"
          variant="ghost"
          aria-label={`Remove ${artist.name}`}
          className={cn(compact && !choosing && "ml-auto")}
          onClick={() => onSettle(null)}
        >
          <XIcon />
        </Button>
      </div>
      {create.error && (
        <p className="basis-full text-destructive">{create.error.message}</p>
      )}
    </div>
  )
}
